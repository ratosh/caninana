use log::debug;
use rand::prelude::*;
use rust_sc2::bot::Bot;
use rust_sc2::prelude::*;
use rust_sc2::units::Container;
use rust_sc2::Event::UnitDestroyed;
use std::cmp::Ordering;
use std::collections::HashMap;

use crate::command_queue::Command;
use crate::params::*;
use crate::utils::*;
use crate::{AIComponent, BotState};

#[derive(Default)]
pub struct OverlordManager {
    scout_lord: Option<u64>,
    placement_map: Vec<Point2>,
    placement_occupation: HashMap<Point2, u64>,
    assignments: HashMap<u64, OverlordAssignment>,
    cast_time: HashMap<u64, f32>,
}

enum OverlordAssignment {
    Point(Point2),
    Unit(u64),
}

impl OverlordManager {
    const EXPANSION_DISTANCE: f32 = 9f32;
    const RETREAT_ON: [UnitTypeId; 5] = [
        UnitTypeId::Viking,
        UnitTypeId::Battlecruiser,
        UnitTypeId::Phoenix,
        UnitTypeId::Carrier,
        UnitTypeId::Mutalisk,
    ];

    fn queue_overseers(&self, bot: &mut Bot, bot_state: &mut BotState) {
        let workers = bot.units.my.workers.len();
        let invisible_units = bot_state.enemy_cache.units.filter(|u| u.need_detection());
        if workers >= UNLOCK_OVERSEER_WORKERS || !invisible_units.is_empty() {
            let workers = bot.supply_workers as usize;
            bot_state.build_queue.push(
                Command::new_unit(
                    UnitTypeId::Overseer,
                    1 + workers / 30 + invisible_units.len() as usize,
                    true,
                ),
                false,
                500,
            );
        }
    }

    pub fn build_placement_map(&mut self, bot: &Bot) {
        if !self.placement_map.is_empty() {
            self.placement_map
                .retain(|p| bot.free_expansions().any(|e| e.loc == *p));
            return;
        }
        let expansions = bot.expansions.iter().map(|e| e.loc);

        for expansion in expansions {
            self.placement_map.push(expansion);
        }
        self.placement_map.sort_by(|p1, p2| {
            p1.distance(bot.enemy_start)
                .partial_cmp(&p2.distance(bot.enemy_start))
                .unwrap_or(Ordering::Equal)
        });
    }

    fn assignment(&mut self, bot: &Bot, bot_state: &BotState) {
        let mut overseers = bot.units.my.units.of_type(UnitTypeId::Overseer);

        if overseers.is_empty() {
            let mut overlords = bot.units.my.all.of_type(UnitTypeId::Overlord);

            if overlords.len() > 2 {
                if self.scout_lord.is_none() {
                    if let Some(unit) = overlords.closest(bot.start_location) {
                        self.clear_assignment_unit(unit.tag());
                        self.scout_lord = Some(unit.tag());
                    }
                }
            } else {
                self.scout_lord = None;
            }
            for overlord in overlords
                .iter()
                .filter(|u| u.hits_percentage().unwrap_or_default() < 0.9f32)
            {
                self.clear_assignment_unit(overlord.tag());
            }
            overlords = overlords.filter(|u| {
                u.hits_percentage().unwrap_or_default() >= 0.9f32
                    && !self.assignments.contains_key(&u.tag())
            });
            for e in bot.expansions.iter() {
                if let Some(enemy_latest_exp) = bot.enemy_expansions().next() {
                    if e.loc == bot.enemy_start
                        || !e.alliance.is_neutral() && e.loc != enemy_latest_exp.loc
                    {
                        self.clear_assignment_point(&e.loc);
                    }
                }
            }
            for point in self.placement_map.iter() {
                if self.placement_occupation.contains(point) {
                    continue;
                }
                let closest_overlord = overlords.closest(point);
                if let Some(closest) = closest_overlord {
                    self.placement_occupation.insert(*point, closest.tag());
                    self.assignments
                        .insert(closest.tag(), OverlordAssignment::Point(*point));
                    overlords = overlords.filter(|u| u.tag() != closest.tag());
                }
            }
        } else {
            let enemy_units = bot_state
                .enemy_cache
                .units
                .filter(|f| !f.is_worker() && f.is_dangerous());

            let mut next_expansion = bot.free_expansions().map(|e| e.loc).next();
            let mut main_targets = enemy_units.filter(|u| u.need_detection());
            let mut assignments = Units::new();
            let closest_main_targets = main_targets
                .sorted(|u| u.distance(bot.start_location))
                .take(overseers.len() - 1);
            let mut secondary_targets = enemy_units.filter(|u| !u.is_cloaked());
            let mut to_remove = vec![];
            for overseer in overseers.iter() {
                debug!("[{:?}] Checking assignment", overseer.tag());
                if let Some(assignment) = self.assignments.get(&overseer.tag()) {
                    match assignment {
                        OverlordAssignment::Unit(tag) => {
                            if closest_main_targets.contains_tag(*tag) {
                                debug!("Found assignment [{:?}]", tag);
                                if main_targets.remove(*tag).is_some() {
                                    debug!("Keep assignment");
                                    assignments.push(overseer.clone());
                                    to_remove.push(overseer.tag());
                                } else {
                                    debug!("Need new assignment");
                                    self.assignments.remove(&overseer.tag());
                                }
                            } else {
                                debug!("Need new assignment");
                                self.assignments.remove(&overseer.tag());
                            }
                        }
                        OverlordAssignment::Point(point) => {
                            if let Some(next_e) = next_expansion {
                                if *point == next_e {
                                    debug!("Already assigned to next expansion");
                                    next_expansion = None;
                                    to_remove.push(overseer.tag());
                                }
                            }
                        }
                    }
                }
            }
            for remove in to_remove {
                overseers.remove(remove);
            }

            for overseer in overseers.iter() {
                debug!("[{:?}] Looking for new assignments", overseer.tag());
                let unit = if let Some(priority_target) = main_targets.closest(bot.start_location) {
                    debug!("Using a main target");
                    Some(priority_target)
                } else if let Some(secondary_target) = secondary_targets.iter().filter(|u| assignments.closest_distance(u.position()).unwrap_or_max() > 5f32).closest(bot.start_location)
                {
                    debug!("Using a secondary target");
                    Some(secondary_target)
                } else {
                    debug!("No target available");
                    None
                };
                let found_target = if next_expansion.is_some() {
                    None
                } else if let Some(possible_target) = unit {
                    if let Some(assignment) = self.assignments.get(&overseer.tag()) {
                        match assignment {
                            OverlordAssignment::Point(_) => {
                                debug!("Assigned to a point, using target");
                                Some(possible_target.tag())
                            }
                            OverlordAssignment::Unit(current_assignment) => {
                                if let Some(unit) = enemy_units.get(*current_assignment) {
                                    if OVERSEER_SWAP_DISTANCE
                                        + possible_target.distance(bot.start_location)
                                        < unit.distance(bot.start_location)
                                    {
                                        debug!(
                                            "Dist [{:?}] Change assignment",
                                            unit.distance(bot.start_location)
                                        );
                                        Some(possible_target.tag())
                                    } else {
                                        debug!("Keep current assignment");
                                        Some(*current_assignment)
                                    }
                                } else {
                                    debug!("Current assignment not found, change to target");
                                    Some(possible_target.tag())
                                }
                            }
                        }
                    } else {
                        Some(possible_target.tag())
                    }
                } else {
                    None
                };
                if let Some(target) = found_target {
                    if let Some(unit) = enemy_units.get(target) {
                        debug!(
                            "Dist [{:?}] to new assignment.",
                            unit.distance(bot.start_location)
                        );
                        assignments.push(unit.clone());
                    }
                    main_targets.remove(target);
                    secondary_targets.remove(target);
                    self.assignments
                        .insert(overseer.tag(), OverlordAssignment::Unit(target));
                } else if let Some(next_e) = next_expansion {
                    debug!("Assign to next expansion.");
                    next_expansion = None;
                    self.assignments
                        .insert(overseer.tag(), OverlordAssignment::Point(next_e));
                } else {
                    debug!("Assign to enemy start.");
                    self.assignments
                        .insert(overseer.tag(), OverlordAssignment::Point(bot.enemy_start));
                }
            }
        }
    }

    fn micro(&mut self, bot: &mut Bot, bot_state: &mut BotState) {
        self.micro_overlord(bot, bot_state);
        self.micro_overseer(bot, bot_state);
        self.micro_changeling(bot, bot_state);
    }

    // TODO: Hide them if enemy is going heavy on anti air.
    fn micro_overlord(&self, bot: &Bot, bot_state: &BotState) {
        let random_scouting = bot_state
            .enemy_cache
            .units
            .filter(|u| Self::RETREAT_ON.contains(&u.type_id()) || u.can_attack_air())
            .is_empty();
        let overlords = bot.units.my.units.of_type(UnitTypeId::Overlord);
        for unit in overlords.iter() {
            if bot
                .units
                .enemy
                .all
                .filter(|u| {
                    u.can_attack_air()
                        && unit.in_range_of(u, unit.sight_range() + u.speed() + unit.speed())
                })
                .iter()
                .closest(unit)
                .is_some()
            {
                unit.move_towards(bot, bot_state, -20f32);
            } else {
                let safe_point = bot
                    .units
                    .my
                    .townhalls
                    .closest(bot.start_location)
                    .map(|u| u.position())
                    .unwrap_or(bot.start_location);
                let position = if Some(unit.tag()) == self.scout_lord {
                    if let Some(closest_enemy) = bot_state
                        .enemy_cache
                        .units
                        .filter(|f| !f.is_worker() && f.is_dangerous())
                        .closest(bot.start_location)
                    {
                        closest_enemy.position()
                    } else {
                        bot.enemy_start
                    }
                } else if let Some(assignment) = self.assignments.get(&unit.tag()) {
                    match assignment {
                        OverlordAssignment::Point(point) => {
                            point.towards(bot.start_center, Self::EXPANSION_DISTANCE)
                        }
                        OverlordAssignment::Unit(tag) => {
                            if let Some(unit) = bot_state.enemy_cache.units.get(*tag) {
                                unit.position()
                            } else {
                                safe_point
                            }
                        }
                    }
                } else if random_scouting && unit.hits_percentage().unwrap_or_default() > 0.9f32 {
                    if unit.is_idle() {
                        let mut rng = thread_rng();
                        let random_x = (rng.next_u64() % bot.game_info.map_size.x as u64) as f32;
                        let random_y = (rng.next_u64() % bot.game_info.map_size.y as u64) as f32;
                        Point2::new(random_x, random_y)
                    } else {
                        unit.target_pos().unwrap()
                    }
                } else {
                    safe_point
                };
                unit.order_move_to(Target::Pos(position), 4.0f32, false);
            }
        }
    }

    fn micro_overseer(&mut self, bot: &Bot, bot_state: &BotState) {
        for overseer in bot.units.my.units.of_type(UnitTypeId::Overseer).iter() {
            if bot_state
                .enemy_cache
                .units
                .filter(|f| {
                    f.can_attack_air() && f.in_real_range(overseer, f.speed() + overseer.speed())
                        || (f.target_tag().unwrap_or_default() == overseer.tag()
                            && f.position().distance(overseer) < 15f32)
                })
                .iter()
                .closest(overseer)
                .is_some()
            {
                overseer.move_towards(bot, bot_state, -20f32);
            } else if overseer.has_ability(AbilityId::SpawnChangelingSpawnChangeling)
                && !overseer.is_using(AbilityId::SpawnChangelingSpawnChangeling)
                && self
                    .cast_time
                    .get(&overseer.tag())
                    .cloned()
                    .unwrap_or_default()
                    + OVERSEER_CHANGELING_DELAY
                    < bot.time
            {
                self.cast_time.insert(overseer.tag(), bot.time);
                overseer.command(
                    AbilityId::SpawnChangelingSpawnChangeling,
                    Target::None,
                    false,
                );
            } else if let Some(assignment) = self.assignments.get(&overseer.tag()) {
                match assignment {
                    OverlordAssignment::Point(position) => {
                        overseer.order_move_to(Target::Pos(*position), Self::EXPANSION_DISTANCE, false);
                    }
                    OverlordAssignment::Unit(unit) => {
                        overseer.order_move_to(Target::Tag(*unit), 1.0f32, false);
                    }
                }
            } else {
                debug!("Missing assignment")
            };
        }
    }

    fn clear_assignment_unit(&mut self, tag: u64) {
        let removed_point = self.assignments.remove(&tag);
        if let Some(assignment) = removed_point {
            match assignment {
                OverlordAssignment::Point(point) => {
                    self.placement_occupation.remove(&point);
                }
                OverlordAssignment::Unit(_) => {}
            }
        }
    }

    fn clear_assignment_point(&mut self, point: &Point2) {
        let removed_tag = self.placement_occupation.remove(point);
        if let Some(tag) = removed_tag {
            self.assignments.remove(&tag);
        }
    }

    fn micro_changeling(&self, bot: &mut Bot, _bot_state: &mut BotState) {
        let mut random = thread_rng();
        for changeling in bot
            .units
            .my
            .all
            .of_types(&vec![
                UnitTypeId::Changeling,
                UnitTypeId::ChangelingMarine,
                UnitTypeId::ChangelingZealot,
                UnitTypeId::ChangelingZergling,
                UnitTypeId::ChangelingMarineShield,
                UnitTypeId::ChangelingZerglingWings,
            ])
            .filter(|u| u.is_idle())
        {
            let target = if let Some(expansion) = bot.expansions.iter().filter(|u| !u.alliance.is_mine()).choose(&mut random) {
                expansion.loc
            } else {
                bot.enemy_start
            };
            changeling.order_move_to(Target::Pos(target), 7f32, false)
        }
    }
}

impl AIComponent for OverlordManager {
    fn process(&mut self, bot: &mut Bot, bot_state: &mut BotState) {
        self.build_placement_map(bot);
        self.assignment(bot, bot_state);
        self.micro(bot, bot_state);
        self.queue_overseers(bot, bot_state);
    }

    fn on_event(&mut self, event: &Event, _: &mut BotState) {
        if let UnitDestroyed(tag, _) = event {
            if self.scout_lord == Some(*tag) {
                self.scout_lord = None;
            }
            self.clear_assignment_unit(*tag);
        }
    }
}
