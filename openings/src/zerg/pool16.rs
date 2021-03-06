use rust_sc2::bot::Bot;
use rust_sc2::prelude::*;

use caninana_core::command_queue::Command;
use caninana_core::{BotState, Opening};

#[derive(Default)]
pub struct Pool16 {}

impl Pool16 {
    fn push_commands(&mut self, bot_state: &mut BotState) {
        bot_state
            .build_queue
            .push(Command::new_unit(UnitTypeId::Drone, 13, true), true, 1000);
        bot_state
            .build_queue
            .push(Command::new_unit(UnitTypeId::Overlord, 2, true), true, 990);
        bot_state
            .build_queue
            .push(Command::new_unit(UnitTypeId::Drone, 16, true), true, 980);
        bot_state.build_queue.push(
            Command::new_unit(UnitTypeId::SpawningPool, 1, true),
            true,
            970,
        );
        bot_state
            .build_queue
            .push(Command::new_unit(UnitTypeId::Drone, 17, true), true, 960);
        bot_state
            .build_queue
            .push(Command::new_unit(UnitTypeId::Hatchery, 2, true), true, 950);
        bot_state
            .build_queue
            .push(Command::new_unit(UnitTypeId::Drone, 18, true), true, 940);
        bot_state
            .build_queue
            .push(Command::new_unit(UnitTypeId::Extractor, 1, true), true, 930);
        bot_state
            .build_queue
            .push(Command::new_unit(UnitTypeId::Overlord, 3, true), true, 920);
        bot_state
            .build_queue
            .push(Command::new_unit(UnitTypeId::Queen, 1, true), true, 910);
        bot_state
            .build_queue
            .push(Command::new_unit(UnitTypeId::Zergling, 2, true), true, 900);
    }
}

impl Opening for Pool16 {
    fn opening(&mut self, _: &Bot, bot_state: &mut BotState) {
        self.push_commands(bot_state);
    }
}
