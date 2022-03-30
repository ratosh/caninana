use rand::prelude::*;
use rust_sc2::bot::Bot;
use rust_sc2::prelude::*;

use crate::utils::{Supply, UnitOrderCheck};
use crate::{AIComponent, BotState};
use crate::command_queue::Command;
use crate::params::*;

#[derive(Default)]
pub struct OverlordManager {}

impl OverlordManager {

    fn queue_overseers(&self, bot: &mut Bot, bot_state: &mut BotState) {
        let workers = bot.counter().all().count(bot.race_values.worker);
        if workers >= UNLOCK_OVERSEER_WORKERS {
            let enemy_invisible = bot_state.enemy_cache.units.filter(|u| u.is_cloaked() || u.is_burrowed()).supply() as usize;
            bot_state.build_queue.push(
                Command::new_unit(UnitTypeId::Overseer, workers/ 20 + enemy_invisible / 10, true),
                false,
                500,
            );
        }
    }

    fn micro(&self, bot: &mut Bot, bot_state: &mut BotState) {
        Self::overlord_micro(bot);
        Self::overseer_micro(bot, bot_state);
    }

    // TODO: Hide them if enemy is going heavy on anti air.
    fn overlord_micro(bot: &Bot) {
        let overlords = bot
            .units
            .my
            .units
            .of_type(UnitTypeId::Overlord)
            .sorted(|u| u.tag());
        let enemy_ramps = bot
            .ramps
            .enemy
            .points
            .iter()
            .map(|p| Point2::new(p.0 as f32, p.1 as f32).towards(bot.start_center, 7f32))
            .collect::<Vec<Point2>>();
        let mut scout_points = vec![];
        for ramp in enemy_ramps {
            if let Some(distance) = scout_points.iter().closest_distance(ramp) {
                if distance > 8f32 {
                    scout_points.push(ramp);
                }
            } else {
                scout_points.push(ramp);
            }
        }
        if let Some(natural) = bot.expansions.iter().take(2).last() {
            scout_points.push(natural.loc);
        }

        for overlord in overlords.iter() {
            let scout_point = scout_points.iter().closest(overlord).cloned();
            if let Some(interest_point) = scout_point {
                scout_points.retain(|p| *p != interest_point);
            }
            if let Some(closest_anti_air) = bot
                .units
                .enemy
                .all
                .filter(|f| {
                    f.can_attack_air() && f.in_real_range(overlord, f.speed() + overlord.speed())
                })
                .iter()
                .closest(overlord)
            {
                let position = overlord.position().towards(
                    closest_anti_air.position(),
                    -closest_anti_air.real_range_vs(overlord),
                );
                overlord.order_move_to(Target::Pos(position), 0.5f32, false);
            } else if overlord.hits_percentage().unwrap_or_default() > 0.9f32 && overlord.is_idle()
            {
                if let Some(ramp) = scout_point {
                    overlord.order_move_to(Target::Pos(ramp), 0.5f32, false);
                } else {
                    let mut rng = thread_rng();
                    let random_x = (rng.next_u64() % bot.game_info.map_size.x as u64) as f32;
                    let random_y = (rng.next_u64() % bot.game_info.map_size.y as u64) as f32;
                    let position = Point2::new(random_x, random_y);
                    overlord.order_move_to(Target::Pos(position), 0.5f32, false);
                }
            }
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
            .filter(|f| !f.is_worker() && f.can_attack() || f.is_cloaked());
        for overseer in overseers.iter() {
            let position = if let Some(closest_anti_air) = bot
                .units
                .enemy
                .all
                .filter(|f| {
                    f.can_attack_air() && f.in_real_range(overseer, f.speed() + overseer.speed())
                })
                .iter()
                .closest(overseer)
            {
                overseer
                    .position()
                    .towards(closest_anti_air.position(), -overseer.speed())
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
            overseer.order_move_to(Target::Pos(position), 1.0f32, false);
        }
    }
}

impl AIComponent for OverlordManager {
    fn process(&mut self, bot: &mut Bot, bot_state: &mut BotState) {
        self.queue_overseers(bot, bot_state);
        self.micro(bot, bot_state);
    }
}
