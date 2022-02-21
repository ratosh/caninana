use rust_sc2::bot::Bot;
use rust_sc2::prelude::*;

use caninana_core::managers::army_manager::ArmyManager;
use caninana_core::managers::cache_manager::CacheManager;
use caninana_core::managers::defense_manager::DefenseManager;
use caninana_core::managers::production_manager::ProductionManager;
use caninana_core::managers::resource_manager::ResourceManager;
use caninana_core::managers::worker_manager::WorkerManager;
use caninana_core::units::overlord_manager::OverlordManager;
use caninana_core::units::queen_manager::QueenManager;
use caninana_core::units::ravager_manager::RavagerManager;
use caninana_core::*;
use caninana_openings::zerg::pool14::Pool14;
use caninana_openings::zerg::pool16::Pool16;

#[bot]
pub struct Caninana {
    components: Vec<ProcessLimiter>,
    bot_state: BotState,
}

impl Default for Caninana {
    fn default() -> Self {
        Self {
            _bot: Bot::default(),
            components: vec![
                ProcessLimiter::new(0, Box::new(CacheManager::default())),
                ProcessLimiter::new(5, Box::new(ArmyManager::default())),
                ProcessLimiter::new(15, Box::new(DefenseManager::default())),
                ProcessLimiter::new(10, Box::new(ProductionManager::default())),
                ProcessLimiter::new(15, Box::new(ResourceManager::default())),
                ProcessLimiter::new(0, Box::new(WorkerManager::default())),
                ProcessLimiter::new(15, Box::new(OverlordManager::default())),
                ProcessLimiter::new(15, Box::new(QueenManager::default())),
                ProcessLimiter::new(15, Box::new(RavagerManager::default())),
            ],
            bot_state: Default::default(),
        }
    }
}

impl Player for Caninana {
    fn get_player_settings(&self) -> PlayerSettings {
        PlayerSettings::new(Race::Zerg).with_name("Caninana")
    }

    fn on_start(&mut self) -> SC2Result<()> {
        let mut opening: Box<dyn Opening> = match self._bot.enemy_race {
            Race::Zerg => Box::new(Pool14::default()),
            _ => Box::new(Pool16::default()),
        };
        opening.opening(&self._bot, &mut self.bot_state);
        self._bot
            .chat_ally(format!("Tag:{}v{}", crate_name!(), crate_version!()).as_str());
        Ok(())
    }

    fn on_step(&mut self, _iteration: usize) -> SC2Result<()> {
        for component in self.components.iter_mut() {
            component.process(&mut self._bot, &mut self.bot_state);
        }
        Ok(())
    }

    /// Called once on last step with a result for your bot.
    fn on_end(&self, _result: GameResult) -> SC2Result<()> {
        println!("Result {:?}", _result);
        Ok(())
    }

    fn on_event(&mut self, event: Event) -> SC2Result<()> {
        for component in self.components.iter_mut() {
            component.on_event(&event, &mut self.bot_state);
        }
        Ok(())
    }
}
