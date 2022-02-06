use rust_sc2::bot::Bot;
use rust_sc2::prelude::*;

use crate::command_queue::Command;
use crate::{BotInfo, Manager};

#[derive(Default)]
pub struct DefenseManager {
    last_loop: u32,
}

impl DefenseManager {
    pub fn check_defensive_placement(&self, _: &mut Bot) {
        todo!()
    }

    pub fn queue_defense(&self, bot: &mut Bot, bot_info: &mut BotInfo) {
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
            let enemy_supply = bot
                .units
                .cached
                .units
                .filter(|unit| !unit.is_worker() && unit.is_ready())
                .supply();
            let crawlers = enemy_supply as usize / 8;
            println!("E{enemy_supply:?} C{crawlers:?}");
            bot_info.build_queue.push(
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

impl DefenseManager {
    const PROCESS_DELAY: u32 = 15;
}

impl Manager for DefenseManager {
    fn process(&mut self, bot: &mut Bot, bot_info: &mut BotInfo) {
        let last_loop = self.last_loop;
        let game_loop = bot.state.observation.game_loop();
        if last_loop + Self::PROCESS_DELAY > game_loop {
            return;
        }
        self.last_loop = game_loop;
        // self.check_defensive_placement(bot);
        self.queue_defense(bot, bot_info);
    }
}
