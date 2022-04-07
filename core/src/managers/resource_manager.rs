use log::debug;
use rust_sc2::bot::Bot;
use rust_sc2::prelude::*;

use crate::command_queue::Command;
use crate::params::*;
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
                !unit.is_worker()
                    && bot
                        .units
                        .enemy
                        .townhalls
                        .closest_distance(unit.position())
                        .unwrap_or_max()
                        > 20f32
            })
            .strength(bot);
        let close_enemy_units = bot_state
            .enemy_cache
            .units
            .filter(|unit| {
                !unit.is_worker()
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
            .filter(|unit| !unit.is_worker())
            .strength(bot);
        let our_strength = bot
            .units
            .my
            .all
            .filter(|unit| !unit.is_worker() && !unit.is_structure())
            .strength(bot);
        let our_offensive_strength = bot
            .units
            .my
            .all
            .filter(|unit| {
                !unit.is_worker() && !unit.is_structure() && unit.type_id() != UnitTypeId::Queen
            })
            .strength(bot);

        let our_expansions = bot.owned_expansions().count();
        let their_expansions = bot.enemy_expansions().count();

        let mut conditions: u8 = 0;
        if close_enemy_units > our_offensive_strength {
            conditions += 1;
        }
        if advanced_enemy_units * 0.8f32 > our_strength {
            conditions += 1;
        }
        if their_strength * 0.8f32 > our_offensive_strength {
            conditions += 1;
        }
        if their_strength > our_strength {
            conditions += 1;
        }
        if their_expansions == 1 && our_expansions > 1 {
            conditions += 1;
        }
        if bot.minerals > 1_000 {
            conditions += 1;
        }
        if their_strength * 5f32 < our_offensive_strength {
            conditions = 0;
        }

        bot_state.spending_focus = match conditions {
            0 => SpendingFocus::Economy,
            1 => SpendingFocus::Balance,
            _ => SpendingFocus::Army,
        };
        bot.debug.draw_text_screen(
            format!(
                "Decision {:?} > A[{:?}] T[{:?}] [{:?}|{:?}]vs{:?}",
                bot_state.spending_focus,
                advanced_enemy_units,
                close_enemy_units,
                our_strength,
                our_offensive_strength,
                their_strength
            )
            .as_str(),
            Some((0f32, 0f32)),
            Some((255, 255, 255)),
            Some(12),
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
        let halls = if bot_state.spending_focus != SpendingFocus::Army
            && ((ideal_harvesters < 70 && ideal_harvesters.saturating_sub(current_harvesters) < 8)
                || bot.minerals > 1_000)
        {
            bot.counter().count(bot.race_values.start_townhall) + 1
        } else {
            bot.counter().all().count(bot.race_values.start_townhall)
        };
        bot_state.build_queue.push(
            Command::new_unit(
                bot.race_values.start_townhall,
                halls,
                bot_state.spending_focus != SpendingFocus::Army,
            ),
            false,
            400,
        );
    }

    fn order_geysers(&self, bot: &mut Bot, bot_state: &mut BotState) {
        let extractor = bot.race_values.gas;
        let workers = bot.units.my.workers.len();
        let wanted_extractors = if workers < DOUBLE_GAS_PER_BASE_WORKERS {
            1.max(workers as usize / 16)
        } else {
            bot.owned_expansions().count().saturating_sub(1) * 2
        };
        bot_state.build_queue.push(
            Command::new_unit(extractor, wanted_extractors, false),
            false,
            260,
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
