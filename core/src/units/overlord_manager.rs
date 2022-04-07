use itertools::Itertools;
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
    overlord_assignment: HashMap<u64, Point2>,
}

impl OverlordManager {
    const EXPANSION_DISTANCE: f32 = 8f32;
    const RETREAT_ON: [UnitTypeId; 5] = [
        UnitTypeId::Viking,
        UnitTypeId::Battlecruiser,
        UnitTypeId::Phoenix,
        UnitTypeId::Carrier,
        UnitTypeId::Mutalisk,
    ];

    const IGNORE_INVISIBLE: [UnitTypeId; 3] = [
        UnitTypeId::CreepTumor,
        UnitTypeId::CreepTumorBurrowed,
        UnitTypeId::CreepTumorQueen,
    ];

    fn queue_overseers(&self, bot: &mut Bot, bot_state: &mut BotState) {
        let workers = bot.units.my.workers.len();
        let invisible_units = !bot_state
            .enemy_cache
            .units
            .filter(|u| (u.is_cloaked() && u.can_attack()) || u.is_burrowed())
            .is_empty();
        if workers >= UNLOCK_OVERSEER_WORKERS || invisible_units {
            let workers = bot.supply_workers as usize;
            let enemy_invisible = bot_state
                .enemy_cache
                .units
                .filter(|u| u.is_cloaked() || u.is_burrowed())
                .supply() as usize;
            bot_state.build_queue.push(
                Command::new_unit(
                    UnitTypeId::Overseer,
                    1 + workers / 20 + enemy_invisible / 10,
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

    fn overlord_assignment(&mut self, bot: &Bot) {
        let retreat_lords = bot.counter().count(UnitTypeId::Overseer) > 0;
        let mut overlords = bot.units.my.all.of_type(UnitTypeId::Overlord);
        for overlord in overlords
            .iter()
            .filter(|u| u.hits_percentage().unwrap_or_default() < 0.9f32)
        {
            self.clear_assignment_overlord(overlord.tag());
        }
        overlords = overlords.filter(|u| {
            u.hits_percentage().unwrap_or_default() >= 0.9f32
                && !self.overlord_assignment.contains_key(&u.tag())
        });
        if retreat_lords {
            return;
        }
        if bot.counter().all().count(UnitTypeId::Overlord) > 2
            && bot.counter().all().count(UnitTypeId::Overseer) == 0
        {
            if self.scout_lord.is_none() {
                if let Some(unit) = overlords.pop() {
                    self.scout_lord = Some(unit.tag());
                }
            }
        } else {
            self.scout_lord = None;
        }
        for e in bot.expansions.iter() {
            if e.alliance != Alliance::Neutral {
                self.clear_assignment_point(&e.loc);
            }
        }
        for point in self.placement_map.iter() {
            if self.placement_occupation.contains(point) {
                continue;
            }
            let closest_overlord = overlords.closest(point);
            if let Some(closest) = closest_overlord {
                self.placement_occupation.insert(*point, closest.tag());
                self.overlord_assignment.insert(closest.tag(), *point);
                overlords = overlords.filter(|u| u.tag() != closest.tag());
            }
        }
    }

    fn micro(&self, bot: &mut Bot, bot_state: &mut BotState) {
        self.overlord_micro(bot, bot_state);
        Self::overseer_micro(bot, bot_state);
    }

    // TODO: Hide them if enemy is going heavy on anti air.
    fn overlord_micro(&self, bot: &Bot, bot_state: &BotState) {
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
                .filter(|u| u.can_attack_air() && u.in_real_range(unit, u.speed() + unit.speed()))
                .iter()
                .closest(unit)
                .is_some()
            {
                unit.move_towards(bot, bot_state, -20f32);
            } else {
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
                } else if let Some(assignment) = self.overlord_assignment.get(&unit.tag()) {
                    assignment.towards(bot.start_center, Self::EXPANSION_DISTANCE)
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
                    bot.units
                        .my
                        .townhalls
                        .closest(bot.start_location)
                        .map(|u| u.position())
                        .unwrap_or(bot.start_location)
                };
                unit.order_move_to(Target::Pos(position), 4.0f32, false);
            }
        }
    }

    fn overseer_micro(bot: &Bot, bot_state: &BotState) {
        let mut enemy_units = bot_state
            .enemy_cache
            .units
            .filter(|f| !f.is_worker() && f.is_dangerous() || f.is_cloaked());
        let mut next_expansion = bot.free_expansions().map(|e| e.loc).next();
        for unit in bot
            .units
            .my
            .units
            .of_type(UnitTypeId::Overseer)
            .iter()
            .sorted_by(|a, b| b.hits().cmp(&a.hits()).then(a.tag().cmp(&b.tag())))
        {
            if bot
                .units
                .enemy
                .all
                .filter(|f| f.can_attack_air() && f.in_real_range(unit, f.speed() + unit.speed()))
                .iter()
                .closest(unit)
                .is_some()
            {
                unit.move_towards(bot, bot_state, -20f32);
            } else {
                let position = if let Some(point) = next_expansion {
                    next_expansion = None;
                    point
                } else if let Some(unit) = enemy_units
                    .clone()
                    .filter(|f| f.is_cloaked() && !Self::IGNORE_INVISIBLE.contains(&f.type_id()))
                    .closest(bot.start_location)
                {
                    enemy_units = enemy_units.filter(|f| f.position().distance(unit) > 9f32);
                    unit.position()
                } else if let Some(unit) = enemy_units.clone().closest(bot.start_location) {
                    enemy_units = enemy_units.filter(|f| f.position().distance(unit) > 9f32);
                    unit.position()
                } else {
                    bot.enemy_start
                };
                unit.order_move_to(Target::Pos(position), 1.0f32, false);
            }
        }
    }

    fn clear_assignment_overlord(&mut self, tag: u64) {
        let removed_point = self.overlord_assignment.remove(&tag);
        if let Some(point) = removed_point {
            self.placement_occupation.remove(&point);
        }
    }

    fn clear_assignment_point(&mut self, point: &Point2) {
        let removed_tag = self.placement_occupation.remove(point);
        if let Some(tag) = removed_tag {
            self.overlord_assignment.remove(&tag);
        }
    }
}

impl AIComponent for OverlordManager {
    fn process(&mut self, bot: &mut Bot, bot_state: &mut BotState) {
        self.build_placement_map(bot);
        self.overlord_assignment(bot);
        self.micro(bot, bot_state);
        self.queue_overseers(bot, bot_state);
    }

    fn on_event(&mut self, event: &Event, _: &mut BotState) {
        if let UnitDestroyed(tag, _) = event {
            self.clear_assignment_overlord(*tag);
            if self.scout_lord == Some(*tag) {
                self.scout_lord = None;
            }
        }
    }
}
