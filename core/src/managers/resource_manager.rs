use crate::command_queue::Command;
use crate::{BotInfo, Manager};
use rust_sc2::bot::Bot;
use rust_sc2::prelude::*;

#[derive(Default)]
pub struct ResourceManager {
    last_loop: u32,
}

impl ResourceManager {
    const PROCESS_DELAY: u32 = 10;

    fn order_supply(&self, bot: &mut Bot, bot_info: &mut BotInfo) {
        if bot.supply_cap >= 200 {
            return;
        }
        let current_overlords = bot.counter().all().count(UnitTypeId::Overlord);
        let ordered_overlords = bot.counter().ordered().count(UnitTypeId::Overlord);
        let more_lords = 3.min(bot.supply_cap / ((bot.supply_left + 1) * 8)) as usize;
        if more_lords > 0 && ordered_overlords < 5 {
            bot_info.build_queue.push(
                Command::new_unit(UnitTypeId::Overlord, current_overlords + more_lords),
                false,
                50,
            );
        }
    }

    fn order_expansion(&self, bot: &mut Bot, bot_info: &mut BotInfo) {
        let mut bases = Units::new();

        if !bot
            .units
            .my
            .structures
            .not_ready()
            .of_type(bot.race_values.start_townhall.clone())
            .is_empty()
        {
            return;
        }
        for unit_type in bot.race_values.townhalls.iter() {
            bases.extend(bot.units.my.structures.of_type(unit_type.clone()));
        }
        let ideal_harversters = bases.sum(|x| x.ideal_harvesters().unwrap());
        let current_harversters = bases.sum(|x| x.assigned_harvesters().unwrap()) + bot.units.my.workers.idle().len() as u32;
        if current_harversters > 22 &&
            bot.can_afford(bot.race_values.start_townhall, false)
            && ideal_harversters < 64
            && current_harversters >= ideal_harversters
        {
            bot_info.build_queue.push(
                Command::new_unit(UnitTypeId::Hatchery, bases.len() + 1),
                current_harversters > ideal_harversters,
                80,
            );
        }
    }

    fn order_geysers(&self, bot: &mut Bot, bot_info: &mut BotInfo) {
        let extractor = bot.race_values.gas;
        let drones = bot.counter().all().count(UnitTypeId::Drone);
        let wanted_extractors = if drones < 70 {
            1.max(bot.counter().all().count(UnitTypeId::Drone) / 16)
        } else {
            bot.owned_expansions().count() * 2
        };
        bot_info
            .build_queue
            .push(Command::new_unit(extractor, wanted_extractors), false, 5);
    }
}

impl Manager for ResourceManager {
    fn process(&mut self, bot: &mut Bot, bot_info: &mut BotInfo) {
        let last_loop = self.last_loop;
        let game_loop = bot.state.observation.game_loop();
        if last_loop + Self::PROCESS_DELAY > game_loop {
            return;
        }
        self.last_loop = game_loop;
        self.order_supply(bot, bot_info);
        self.order_expansion(bot, bot_info);
        self.order_geysers(bot, bot_info);
    }
}
