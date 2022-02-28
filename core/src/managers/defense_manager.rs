use rust_sc2::bot::Bot;

use crate::{AIComponent, BotState};

#[derive(Default)]
pub struct DefenseManager {}

impl DefenseManager {
    pub fn check_defensive_placement(&self, _: &mut Bot) {
        todo!()
    }

    pub fn queue_defense(&self, _bot: &mut Bot, _bot_state: &mut BotState) {
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
    }
}

impl AIComponent for DefenseManager {
    fn process(&mut self, bot: &mut Bot, bot_state: &mut BotState) {
        // self.check_defensive_placement(bot);
        self.queue_defense(bot, bot_state);
    }
}
