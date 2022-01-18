use caninana_core::command_queue::Command;
use caninana_core::{BotInfo, Opening};
use rust_sc2::bot::Bot;
use rust_sc2::prelude::*;

#[derive(Default)]
pub struct PoolFirst {}

impl PoolFirst {
    fn push_commands(&mut self, bot_info: &mut BotInfo) {
        bot_info
            .build_queue
            .push(Command::new_unit(UnitTypeId::Drone, 13), true, 1000);
        bot_info
            .build_queue
            .push(Command::new_unit(UnitTypeId::Overlord, 2), true, 990);
        bot_info
            .build_queue
            .push(Command::new_unit(UnitTypeId::Drone, 16), true, 980);
        bot_info
            .build_queue
            .push(Command::new_unit(UnitTypeId::SpawningPool, 1), true, 970);
        bot_info
            .build_queue
            .push(Command::new_unit(UnitTypeId::Drone, 17), true, 960);
        bot_info
            .build_queue
            .push(Command::new_unit(UnitTypeId::Hatchery, 2), true, 950);
        bot_info
            .build_queue
            .push(Command::new_unit(UnitTypeId::Drone, 18), true, 940);
        bot_info
            .build_queue
            .push(Command::new_unit(UnitTypeId::Overlord, 3), true, 940);
        bot_info
            .build_queue
            .push(Command::new_unit(UnitTypeId::Queen, 1), true, 930);
        bot_info
            .build_queue
            .push(Command::new_unit(UnitTypeId::Drone, 19), true, 920);
    }
}

impl Opening for PoolFirst {
    fn opening(&mut self, _: &Bot, bot_info: &mut BotInfo) {
        self.push_commands(bot_info);
    }
}
