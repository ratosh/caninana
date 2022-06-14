pub mod command_queue;
pub mod managers;
mod params;
pub mod units;
pub mod utils;

use crate::command_queue::CommandQueue;
use crate::managers::cache_manager::UnitsCache;
use crate::managers::squad_manager::Squads;
use rust_sc2::bot::Bot;
use rust_sc2::Event;

pub trait Opening {
    fn opening(&mut self, bot: &Bot, bot_state: &mut BotState);
}

pub struct ProcessLimiter {
    delay: u32,
    last_loop: u32,
    component: Box<dyn AIComponent>,
}

impl ProcessLimiter {
    pub fn new(delay: u32, component: Box<dyn AIComponent>) -> Self {
        Self {
            delay,
            last_loop: 0,
            component,
        }
    }
}

impl AIComponent for ProcessLimiter {
    fn process(&mut self, bot: &mut Bot, bot_state: &mut BotState) {
        let last_loop = self.last_loop;
        let game_loop = bot.state.observation.game_loop();
        if last_loop + self.delay > game_loop {
            return;
        }
        self.last_loop = game_loop;
        self.component.process(bot, bot_state);
    }

    fn on_event(&mut self, event: &Event, bot_state: &mut BotState) {
        self.component.on_event(event, bot_state)
    }
}

// With this we enforce all components to implement both (even if they don't need)
pub trait AIComponent {
    fn process(&mut self, bot: &mut Bot, bot_state: &mut BotState);

    fn on_event(&mut self, _: &Event, _: &mut BotState) {}
}

#[derive(Debug, PartialEq, Clone)]
pub enum SpendingFocus {
    Economy,
    Balance,
    Army,
}

impl Default for SpendingFocus {
    fn default() -> Self {
        Self::Economy
    }
}

#[derive(Default)]
pub struct BotState {
    pub squads: Squads,
    pub build_queue: CommandQueue,
    pub enemy_cache: UnitsCache,
    pub spending_focus: SpendingFocus,
    pub minimum_strength: f32,
}
