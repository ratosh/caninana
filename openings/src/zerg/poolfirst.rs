use rust_sc2::bot::Bot;
use rust_sc2::prelude::*;

use caninana_core::command_queue::Command;
use caninana_core::{BotInfo, Opening};

#[derive(Default)]
pub struct PoolFirst {}

impl PoolFirst {
    fn push_commands(&mut self, bot_info: &mut BotInfo) {
        bot_info
            .build_queue
            .push(Command::new_unit(UnitTypeId::Drone, 13, false), true, 1000);
        bot_info
            .build_queue
            .push(Command::new_unit(UnitTypeId::Overlord, 2, false), true, 990);
        bot_info
            .build_queue
            .push(Command::new_unit(UnitTypeId::Drone, 16, false), true, 980);
        bot_info.build_queue.push(
            Command::new_unit(UnitTypeId::SpawningPool, 1, false),
            true,
            970,
        );
        bot_info
            .build_queue
            .push(Command::new_unit(UnitTypeId::Drone, 17, false), true, 960);
        bot_info
            .build_queue
            .push(Command::new_unit(UnitTypeId::Hatchery, 2, false), true, 950);
        bot_info.build_queue.push(
            Command::new_unit(UnitTypeId::Extractor, 1, false),
            true,
            940,
        );
        bot_info
            .build_queue
            .push(Command::new_unit(UnitTypeId::Drone, 18, false), true, 930);
        bot_info
            .build_queue
            .push(Command::new_unit(UnitTypeId::Queen, 1, false), true, 920);
        bot_info
            .build_queue
            .push(Command::new_unit(UnitTypeId::Zergling, 3, false), true, 910);
    }
}

impl Opening for PoolFirst {
    fn opening(&mut self, _: &Bot, bot_info: &mut BotInfo) {
        self.push_commands(bot_info);
    }
}
