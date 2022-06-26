use rust_sc2::bot::Bot;
use rust_sc2::prelude::*;

use crate::command_queue::Command;
use crate::params::*;
use crate::utils::*;
use crate::*;

#[derive(Default)]
pub struct ResourceManager {}

impl ResourceManager {
    const DEFENSIVE_UNITS: [UnitTypeId; 1] = [UnitTypeId::Queen];

    const OFFENSIVE_UNITS: [UnitTypeId; 8] = [
        UnitTypeId::Zergling,
        UnitTypeId::Roach,
        UnitTypeId::Ravager,
        UnitTypeId::RavagerCocoon,
        UnitTypeId::Hydralisk,
        UnitTypeId::Corruptor,
        UnitTypeId::BroodLord,
        UnitTypeId::BroodLordCocoon,
    ];

    const ANTI_AIR_UNITS: [UnitTypeId; 3] = [
        UnitTypeId::Queen,
        UnitTypeId::Hydralisk,
        UnitTypeId::Corruptor,
    ];

    const ANTI_GROUND_UNITS: [UnitTypeId; 8] = [
        UnitTypeId::Queen,
        UnitTypeId::Zergling,
        UnitTypeId::Roach,
        UnitTypeId::Ravager,
        UnitTypeId::RavagerCocoon,
        UnitTypeId::Hydralisk,
        UnitTypeId::BroodLord,
        UnitTypeId::BroodLordCocoon,
    ];

    fn spending_decision(&mut self, bot: &mut Bot, bot_state: &mut BotState) {
        let their_expansions = bot.enemy_expansions().count();
        let their_strength = bot_state
            .enemy_cache
            .units
            .filter(|unit| {
                !unit.is_worker() && !unit.is_structure() && unit.type_id() != UnitTypeId::Queen
            })
            .strength(bot);
        let their_ground_strength = bot_state
            .enemy_cache
            .units
            .filter(|unit| {
                !unit.is_flying()
                    && (bot
                        .units
                        .enemy
                        .townhalls
                        .closest_distance(unit.position())
                        .unwrap_or_default()
                        > 16f32
                        || (!unit.is_worker() && !unit.is_structure()))
            })
            .strength(bot);
        let their_air_strength = bot_state
            .enemy_cache
            .units
            .filter(|unit| {
                unit.is_flying()
                    && (bot
                        .units
                        .enemy
                        .townhalls
                        .closest_distance(unit.position())
                        .unwrap_or_default()
                        > 16f32
                        || (!unit.is_worker() && !unit.is_structure()))
            })
            .strength(bot);
        let our_anti_ground_strength = bot
            .units
            .my
            .all
            .filter(|unit| Self::ANTI_GROUND_UNITS.contains(&unit.type_id()))
            .strength(bot)
            + Self::ANTI_GROUND_UNITS
                .iter()
                .map(|t| bot.counter().ordered().count(*t) as f32 * t.base_strength(bot))
                .sum::<f32>();
        let our_anti_air_strength = bot
            .units
            .my
            .all
            .filter(|unit| Self::ANTI_AIR_UNITS.contains(&unit.type_id()))
            .strength(bot)
            + Self::ANTI_AIR_UNITS
                .iter()
                .map(|t| bot.counter().ordered().count(*t) as f32 * t.base_strength(bot))
                .sum::<f32>();
        let ordered_offensive_strength = Self::OFFENSIVE_UNITS
            .iter()
            .map(|t| bot.counter().ordered().count(*t) as f32 * t.base_strength(bot))
            .sum::<f32>();
        let our_offensive_strength = bot
            .units
            .my
            .all
            .filter(|unit| {
                !unit.is_worker() && !unit.is_structure() && unit.type_id() != UnitTypeId::Queen
            })
            .strength(bot)
            + ordered_offensive_strength;

        let ordered_defensive_strength = Self::DEFENSIVE_UNITS
            .iter()
            .map(|t| bot.counter().ordered().count(*t) as f32 * t.base_strength(bot))
            .sum::<f32>();
        let our_strength = bot
            .units
            .my
            .all
            .filter(|unit| !unit.is_worker() && !unit.is_structure())
            .strength(bot)
            + ordered_offensive_strength
            + ordered_defensive_strength;
        let mut conditions: u8 = 0;
        if their_strength * 1.1f32 > our_strength {
            conditions += 2;
        }
        if their_ground_strength > our_anti_ground_strength {
            conditions += 1;
        }
        if their_air_strength > our_anti_air_strength {
            conditions += 1;
        }
        if bot_state.minimum_strength > our_offensive_strength {
            conditions += 2;
        }
        if their_expansions < 2 && bot.time > 155f32 && bot.time < 270f32 {
            conditions += 2;
        }
        bot_state.minimum_strength = bot_state.minimum_strength.max(their_strength * 0.7f32);

        let spending_focus = match conditions {
            0 | 1 => SpendingFocus::Economy,
            _ => SpendingFocus::Army,
        };
        if DEBUG_TEXT && bot_state.spending_focus != spending_focus {
            bot.chat_ally(format!("Changing decision to {:?}, [S{:.2}|OF{:.2}|AA{:.2}|AG{:.2}] vs [S{:.2}|A{:.2}|G{:.2}]", spending_focus, our_strength, our_offensive_strength, our_anti_air_strength, our_anti_ground_strength, their_strength, their_air_strength, their_ground_strength).as_str());
            bot_state.spending_focus = spending_focus;
        }
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
        let ideal_diff = if bases.len() < 3 { 12 } else { 2 };
        let halls = if bot_state.spending_focus != SpendingFocus::Army
            && ((ideal_harvesters < 70
                && ideal_harvesters.saturating_sub(current_harvesters) < ideal_diff)
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
        let wanted_extractors = if workers < DOUBLE_GAS_PER_BASE_WORKERS
            && bot.counter().count(UnitTypeId::RoachWarren) == 0
        {
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
