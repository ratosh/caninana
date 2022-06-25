use rust_sc2::bot::Bot;
use rust_sc2::prelude::*;

use crate::command_queue::Command;
use crate::params::PRIORITY_SPORE_CRAWLER;
use crate::*;

#[derive(Default)]
pub struct DefenseManager {}

impl DefenseManager {
    const SPORE_UNITS: [UnitTypeId; 5] = [
        UnitTypeId::Banshee,
        UnitTypeId::Battlecruiser,
        UnitTypeId::Oracle,
        UnitTypeId::Carrier,
        UnitTypeId::Mutalisk,
    ];

    pub fn queue_defense(&self, bot: &mut Bot, bot_state: &mut BotState) {
        // let enemy_strength = bot_state
        //     .enemy_cache
        //     .units
        //     .filter(|unit| !unit.is_worker() && unit.can_attack())
        //     .strength(bot);
        // let our_strength = bot
        //     .units
        //     .my
        //     .units
        //     .filter(|unit| !unit.is_worker() && unit.can_attack())
        //     .strength(bot);
        // if enemy_strength >= our_strength * 0.8f32
        //     && bot_state.spending_focus != SpendingFocus::Army {
        //     let spines = bot.units.my.townhalls.len();
        //     bot_state.build_queue.push(
        //         Command::new_unit(UnitTypeId::SpineCrawler, spines, true),
        //         false,
        //         210,
        //     );
        // }
        if !bot_state
            .enemy_cache
            .units
            .filter(|u| Self::SPORE_UNITS.contains(&u.type_id()))
            .is_empty()
        {
            let spores = bot.units.my.townhalls.len();
            bot_state.build_queue.push(
                Command::new_unit(UnitTypeId::SporeCrawler, spores, true),
                false,
                PRIORITY_SPORE_CRAWLER,
            );
        };
    }
}

impl AIComponent for DefenseManager {
    fn process(&mut self, bot: &mut Bot, bot_state: &mut BotState) {
        self.queue_defense(bot, bot_state);
    }
}
