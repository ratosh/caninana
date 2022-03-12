use log::debug;
use rust_sc2::bot::Bot;
use rust_sc2::prelude::*;

use crate::command_queue::Command;
use crate::params::DOUBLE_GAS_PER_BASE_WORKERS;
use crate::utils::*;
use crate::*;

#[derive(Default)]
pub struct ResourceManager {}

impl ResourceManager {
    fn spending_decision(&mut self, bot: &mut Bot, bot_state: &mut BotState) {
        let advanced_enemy_units = bot_state
            .enemy_cache
            .units
            .filter(|unit| {
                unit.can_attack()
                    && !unit.is_worker()
                    && bot
                        .units
                        .enemy
                        .townhalls
                        .closest_distance(unit.position())
                        .unwrap_or_max()
                        > 25f32
            })
            .strength(bot);
        let close_enemy_units = bot_state
            .enemy_cache
            .units
            .filter(|unit| {
                unit.can_attack()
                    && !unit.is_worker()
                    && bot
                        .units
                        .my
                        .townhalls
                        .closest_distance(unit.position())
                        .unwrap_or_default()
                        < 30f32
            })
            .strength(bot);
        let their_strength = bot_state
            .enemy_cache
            .units
            .filter(|unit| !unit.is_worker() && unit.can_attack())
            .strength(bot);
        let our_strength = bot
            .units
            .my
            .units
            .filter(|unit| {
                !unit.is_worker() && unit.can_attack() && unit.type_id() != UnitTypeId::Queen
            })
            .strength(bot);
        let mut conditions: u8 = 0;
        if close_enemy_units > our_strength {
            conditions += 1;
        }
        if advanced_enemy_units * 0.6f32 > our_strength {
            conditions += 1;
        }
        if their_strength * 0.4f32 > our_strength {
            conditions += 1;
        }
        if their_strength * 0.8f32 > our_strength {
            conditions += 1;
        }

        bot_state.spending_focus = match conditions {
            0 => SpendingFocus::Economy,
            1 => SpendingFocus::Balance,
            _ => SpendingFocus::Army,
        };
        debug!(
            "Decision {:?} > {:?} {:?} {:?}|{:?}",
            bot_state.spending_focus,
            advanced_enemy_units,
            close_enemy_units,
            our_strength,
            their_strength
        );
    }

    fn order_supply(&self, bot: &mut Bot, bot_state: &mut BotState) {
        if bot.supply_cap >= 200 {
            return;
        }
        const OVERLORD_MAX: usize = 24;
        let wanted_lords = OVERLORD_MAX.min(
            (bot.supply_cap - bot.supply_left) as usize * 9
                / 8
                / bot
                    .game_data
                    .units
                    .get(&UnitTypeId::Overlord)
                    .unwrap()
                    .food_provided as usize,
        );
        let overseers = bot.counter().all().count(UnitTypeId::Overseer);
        bot_state.build_queue.push(
            Command::new_unit(
                UnitTypeId::Overlord,
                wanted_lords.saturating_sub(overseers),
                true,
            ),
            false,
            900,
        );
    }

    fn queue_expansion(&self, bot: &mut Bot, bot_state: &mut BotState) {
        let mut bases = Units::new();

        for unit_type in bot.race_values.townhalls.iter() {
            bases.extend(bot.units.my.structures.of_type(*unit_type));
        }
        let ideal_harvesters = bases.sum(|x| x.ideal_harvesters().unwrap_or(12));
        let current_harvesters = bases.sum(|x| x.assigned_harvesters().unwrap_or_default())
            + bot.units.my.workers.idle().len() as u32;
        if ideal_harvesters < 64
            && (ideal_harvesters.saturating_sub(current_harvesters) <= 4 || bot.minerals > 1_000)
        {
            bot_state.build_queue.push(
                Command::new_unit(
                    bot.race_values.start_townhall,
                    bot.counter().count(bot.race_values.start_townhall) + 1,
                    bot_state.spending_focus != SpendingFocus::Army,
                ),
                false,
                200,
            );
        } else {
            bot_state.build_queue.push(
                Command::new_unit(
                    bot.race_values.start_townhall,
                    bot.counter().all().count(bot.race_values.start_townhall),
                    bot_state.spending_focus != SpendingFocus::Army,
                ),
                false,
                200,
            );
        }
    }

    fn order_geysers(&self, bot: &mut Bot, bot_state: &mut BotState) {
        let extractor = bot.race_values.gas;
        let drones = bot.counter().all().count(UnitTypeId::Drone);
        let wanted_extractors = if drones < DOUBLE_GAS_PER_BASE_WORKERS {
            1.max(bot.counter().all().count(UnitTypeId::Drone) / 16)
        } else {
            bot.owned_expansions().count() * 2
        };
        bot_state.build_queue.push(
            Command::new_unit(extractor, wanted_extractors, false),
            false,
            5,
        );
    }
}

impl AIComponent for ResourceManager {
    fn process(&mut self, bot: &mut Bot, bot_state: &mut BotState) {
        self.spending_decision(bot, bot_state);
        self.order_supply(bot, bot_state);
        self.queue_expansion(bot, bot_state);
        self.order_geysers(bot, bot_state);
    }
}
