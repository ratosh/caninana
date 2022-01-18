pub mod command_queue;
pub mod managers;

use crate::command_queue::CommandQueue;
use rust_sc2::bot::Bot;
use rust_sc2::Event;

pub trait Opening {
    fn opening(&mut self, bot: &Bot, bot_info: &mut BotInfo);
}

pub trait Manager {
    fn process(&mut self, bot: &mut Bot, bot_info: &mut BotInfo);
}

pub trait EventListener {
    fn on_event(&mut self, event: &Event);
}

#[derive(Debug, Default)]
pub struct BotInfo {
    pub build_queue: CommandQueue,
}
