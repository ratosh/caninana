use rust_sc2::bot::Bot;
use rust_sc2::prelude::*;

use caninana_core::command_queue::Command;
use caninana_core::{BotState, Opening};

#[derive(Default)]
pub struct Pool14 {}

impl Pool14 {
    fn push_commands(&mut self, bot_state: &mut BotState) {
        bot_state
            .build_queue
            .push(Command::new_unit(UnitTypeId::Drone, 14, true), true, 1000);
        bot_state.build_queue.push(
            Command::new_unit(UnitTypeId::SpawningPool, 1, true),
            true,
            990,
        );
        bot_state
            .build_queue
            .push(Command::new_unit(UnitTypeId::Overlord, 2, true), true, 980);
        bot_state
            .build_queue
            .push(Command::new_unit(UnitTypeId::Drone, 17, true), true, 960);
        bot_state
            .build_queue
            .push(Command::new_unit(UnitTypeId::Hatchery, 2, true), true, 950);
        bot_state
            .build_queue
            .push(Command::new_unit(UnitTypeId::Extractor, 1, true), true, 930);
        bot_state
            .build_queue
            .push(Command::new_unit(UnitTypeId::Queen, 1, true), true, 920);
        bot_state
            .build_queue
            .push(Command::new_unit(UnitTypeId::Zergling, 3, true), true, 910);
    }
}

impl Opening for Pool14 {
    fn opening(&mut self, _: &Bot, bot_state: &mut BotState) {
        self.push_commands(bot_state);
    }
}
