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
use crate::utils::{*};
use crate::{AIComponent, BotState};

#[derive(Default)]
pub struct OverlordManager {
    scout_lord: Option<u64>,
    placement_map: Vec<Point2>,
    placement_occupation: HashMap<Point2, u64>,
    overlord_assignment: HashMap<u64, Point2>,
    random_scouting: bool,
}

impl OverlordManager {
    const RETREAT_ON: [UnitTypeId; 5] = [
        UnitTypeId::Viking,
        UnitTypeId::Battlecruiser,
        UnitTypeId::Phoenix,
        UnitTypeId::Carrier,
        UnitTypeId::Mutalisk,
    ];

    fn queue_overseers(&self, bot: &mut Bot, bot_state: &mut BotState) {
        let workers = bot.counter().all().count(bot.race_values.worker);
        if workers >= UNLOCK_OVERSEER_WORKERS {
            let enemy_invisible = bot_state
                .enemy_cache
                .units
                .filter(|u| u.is_cloaked() || u.is_burrowed())
                .supply() as usize;
            bot_state.build_queue.push(
                Command::new_unit(
                    UnitTypeId::Overseer,
                    workers / 20 + enemy_invisible / 10,
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
                .retain(|p| bot.free_expansions().map(|e| e.loc).contains(p));
            return;
        }
        // let enemy_ramp = bot.ramps.enemy.points.iter().map(|p| Point2::from(*p)).center();
        //
        // if let Some(ramp) = enemy_ramp {
        //     self.placement_map.push(ramp);
        // }
        // let ramps = bot.ramps.all.iter().map(|r| {
        //     r.points
        //         .iter()
        //         .map(|p| Point2::new(p.0 as f32, p.1 as f32))
        //         .center()
        // });
        //
        // for ramp in ramps.flatten() {
        //     self.placement_map.push(ramp);
        // }
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
        if self.scout_lord.is_none() {
            if bot.counter().all().count(UnitTypeId::Overseer) == 0 {
                if let Some(unit) = overlords.pop() {
                    self.scout_lord = Some(unit.tag());
                }
            }
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

    fn check_decision(&mut self, _: &mut Bot, bot_state: &mut BotState) {
        self.random_scouting = bot_state
            .enemy_cache
            .units
            .filter(|u| Self::RETREAT_ON.contains(&u.type_id()))
            .is_empty();
    }

    fn micro(&self, bot: &mut Bot, bot_state: &mut BotState) {
        self.overlord_micro(bot, bot_state);
        Self::overseer_micro(bot, bot_state);
    }

    // TODO: Hide them if enemy is going heavy on anti air.
    fn overlord_micro(&self, bot: &Bot, bot_state: &BotState) {
        let overlords = bot.units.my.units.of_type(UnitTypeId::Overlord);
        for unit in overlords.iter() {
            let position = if let Some(closest_anti_air) = bot
                .units
                .enemy
                .all
                .filter(|u| u.can_attack_air() && u.in_real_range(unit, u.speed() + unit.speed()))
                .iter()
                .closest(unit)
            {
                unit.position().towards(
                    closest_anti_air.position(),
                    -closest_anti_air.real_range_vs(unit),
                )
            } else if Some(unit.tag()) == self.scout_lord {
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
                *assignment
            } else if self.random_scouting && unit.hits_percentage().unwrap_or_default() > 0.9f32 {
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
            unit.order_move_to(Target::Pos(position), 1.0f32, false);
        }
    }

    fn overseer_micro(bot: &Bot, bot_state: &BotState) {
        let overseers = bot
            .units
            .my
            .units
            .of_type(UnitTypeId::Overseer)
            .sorted(|f| f.tag());
        let mut enemy_units = bot_state
            .enemy_cache
            .units
            .filter(|f| !f.is_worker() && f.is_dangerous() || f.is_cloaked());
        for unit in overseers.iter() {
            let position = if let Some(closest_anti_air) = bot
                .units
                .enemy
                .all
                .filter(|f| f.can_attack_air() && f.in_real_range(unit, f.speed() + unit.speed()))
                .iter()
                .closest(unit)
            {
                unit.position().towards(
                    closest_anti_air.position(),
                    -closest_anti_air.real_range_vs(unit),
                )
            } else if let Some(unit) = enemy_units
                .clone()
                .filter(|f| f.is_cloaked())
                .furthest(bot.enemy_start)
            {
                enemy_units = enemy_units.filter(|f| f.position().distance(unit) > 9f32);
                unit.position()
            } else if let Some(unit) = enemy_units.clone().furthest(bot.enemy_start) {
                enemy_units = enemy_units.filter(|f| f.position().distance(unit) > 9f32);
                unit.position()
            } else {
                bot.enemy_start
            };
            unit.order_move_to(Target::Pos(position), 1.0f32, false);
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
        self.check_decision(bot, bot_state);
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
