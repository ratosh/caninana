use rust_sc2::bot::Bot;
use rust_sc2::prelude::*;

use crate::command_queue::Command;
use crate::{AIComponent, BotState};

#[derive(Default)]
pub struct DefenseManager {}

impl DefenseManager {
    pub fn check_defensive_placement(&self, _: &mut Bot) {
        todo!()
    }

    pub fn queue_defense(&self, bot: &mut Bot, bot_state: &mut BotState) {
        let advanced_enemy = !bot
            .units
            .enemy
            .units
            .filter(|unit| {
                !unit.is_worker()
                    && unit.can_attack()
                    && unit.position().distance(bot.enemy_start) * 2f32
                        > unit.position().distance(bot.start_location)
            })
            .is_empty();
        if advanced_enemy {
            let crawlers = bot.units.my.townhalls.len() - 1;
            bot_state.build_queue.push(
                Command::new_unit(UnitTypeId::SpineCrawler, crawlers, true),
                false,
                210,
            );
        }
    }
}

impl Supply for Units {
    fn supply(&self) -> f32 {
        self.iter().map(|u| u.supply_cost()).sum()
    }
}

trait Supply {
    fn supply(&self) -> f32;
}

impl AIComponent for DefenseManager {
    fn process(&mut self, bot: &mut Bot, bot_state: &mut BotState) {
        // self.check_defensive_placement(bot);
        self.queue_defense(bot, bot_state);
    }
}
