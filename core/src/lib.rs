pub mod command_queue;
pub mod managers;

use crate::command_queue::CommandQueue;
use rust_sc2::bot::Bot;
use rust_sc2::Event;

pub trait Opening {
    fn opening(&mut self, bot: &Bot, bot_state: &mut BotState);
}

// With this we enforce all components to implement both (even if they don't need)
pub trait AIComponent {
    fn process(&mut self, bot: &mut Bot, bot_state: &mut BotState);
    fn on_event(&mut self, event: &Event);
}

#[derive(Debug, Default)]
pub struct BotState {
    pub build_queue: CommandQueue,
}
