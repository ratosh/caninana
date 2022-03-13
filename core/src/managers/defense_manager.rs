use rust_sc2::bot::Bot;
use rust_sc2::prelude::*;

use crate::command_queue::Command;
use crate::{AIComponent, BotState};

#[derive(Default)]
pub struct DefenseManager {}

impl DefenseManager {
    const SPORE_UNITS: [UnitTypeId; 6] = [
        UnitTypeId::Banshee,
        UnitTypeId::Battlecruiser,
        UnitTypeId::Oracle,
        UnitTypeId::VoidRay,
        UnitTypeId::Carrier,
        UnitTypeId::Mutalisk,
    ];

    pub fn queue_defense(&self, bot: &mut Bot, bot_state: &mut BotState) {
        // let enemy_supply = bot
        //     .units
        //     .enemy
        //     .units
        //     .filter(|unit| !unit.is_worker() && unit.can_attack())
        //     .supply();
        // if enemy_supply > bot.supply_army {
        //     let crawlers = bot.units.my.townhalls.len();
        //     bot_state.build_queue.push(
        //         Command::new_unit(UnitTypeId::SpineCrawler, crawlers, true),
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
            let halls = bot.units.my.townhalls.ready().len();
            bot_state.build_queue.push(
                Command::new_unit(UnitTypeId::SporeCrawler, halls, true),
                false,
                210,
            );
        }
    }
}

impl AIComponent for DefenseManager {
    fn process(&mut self, bot: &mut Bot, bot_state: &mut BotState) {
        self.queue_defense(bot, bot_state);
    }
}
