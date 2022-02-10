use log::debug;
use rust_sc2::bot::Bot;
use rust_sc2::prelude::*;

use crate::command_queue::Command;
use crate::utils::Supply;
use crate::{AIComponent, BotState, SpendingFocus};

#[derive(Default)]
pub struct ResourceManager {}

impl ResourceManager {
    fn spending_decision(&mut self, bot: &Bot, bot_state: &mut BotState) {
        let advanced_units = !bot
            .units
            .my
            .units
            .filter(|unit| {
                unit.can_attack()
                    && unit
                        .position()
                        .is_closer(unit.distance(bot.start_location) / 2f32, bot.enemy_start)
            })
            .is_empty();
        let advanced_enemy_units = !bot_state
            .enemy_cache
            .units()
            .filter(|unit| {
                unit.can_attack()
                    && unit
                        .position()
                        .is_closer(unit.distance(bot.enemy_start) * 2f32, bot.start_location)
            })
            .is_empty();
        let enemy_supply = bot_state
            .enemy_cache
            .units()
            .filter(|unit| unit.can_attack())
            .supply();
        let our_supply = bot.units.my.all.filter(|unit| unit.can_attack()).supply();

        bot_state.spending_focus =
            if advanced_units && !advanced_enemy_units && our_supply >= enemy_supply {
                SpendingFocus::Economy
            } else {
                SpendingFocus::Army
            };
        debug!("Decision {:?}", bot_state.spending_focus);
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
        bot_state.build_queue.push(
            Command::new_unit(UnitTypeId::Overlord, wanted_lords, false),
            true,
            900,
        );
    }

    fn order_expansion(&self, bot: &mut Bot, bot_state: &mut BotState) {
        let mut bases = Units::new();

        if bot
            .counter()
            .ordered()
            .count(bot.race_values.start_townhall)
            != 0
        {
            return;
        }
        for unit_type in bot.race_values.townhalls.iter() {
            bases.extend(bot.units.my.structures.of_type(*unit_type));
        }
        let ideal_harvesters = bases.sum(|x| x.ideal_harvesters().unwrap());
        let current_harvesters = bases.sum(|x| x.assigned_harvesters().unwrap())
            + bot.units.my.workers.idle().len() as u32;
        if ideal_harvesters < 64
            && (current_harvesters >= ideal_harvesters * 15 / 16 || bot.minerals > 1_000)
        {
            bot_state.build_queue.push(
                Command::new_unit(
                    bot.race_values.start_townhall,
                    bot.counter().all().count(bot.race_values.start_townhall) + 1,
                    true,
                ),
                false,
                200,
            );
        }
    }

    fn order_geysers(&self, bot: &mut Bot, bot_state: &mut BotState) {
        let extractor = bot.race_values.gas;
        let drones = bot.counter().all().count(UnitTypeId::Drone);
        let wanted_extractors = if drones < 35 {
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
        self.order_expansion(bot, bot_state);
        self.order_geysers(bot, bot_state);
    }
}
