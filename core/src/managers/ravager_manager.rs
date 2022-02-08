use rust_sc2::bot::Bot;
use rust_sc2::prelude::*;

use crate::{AIComponent, BotState};

#[derive(Default)]
pub struct RavagerManager {
    last_loop: u32,
}

impl RavagerManager {
    const PROCESS_DELAY: u32 = 5;
    const CORROSIVE_POSSIBLE_TARGETS: [UnitTypeId; 10] = [
        UnitTypeId::SiegeTankSieged,
        UnitTypeId::Thor,
        UnitTypeId::Battlecruiser,
        UnitTypeId::LiberatorAG,
        UnitTypeId::HighTemplar,
        UnitTypeId::Colossus,
        UnitTypeId::VoidRay,
        UnitTypeId::Carrier,
        UnitTypeId::Mothership,
        UnitTypeId::BroodLord,
    ];

    fn cast_corrosive_bile(&self, bot: &mut Bot) {
        let ravagers = bot
            .units
            .my
            .units
            .filter(|f| f.has_ability(AbilityId::EffectCorrosiveBile));
        for ravager in ravagers {
            if let Some(target) = bot
                .units
                .enemy
                .all
                .iter()
                .filter(|f| {
                    ravager.in_ability_cast_range(AbilityId::EffectCorrosiveBile, *f, 0.0f32)
                        && (f.is_structure()
                            || Self::CORROSIVE_POSSIBLE_TARGETS.contains(&f.type_id()))
                })
                .min_by_key(|t| t.hits())
            {
                ravager.command(
                    AbilityId::EffectCorrosiveBile,
                    Target::Pos(target.position()),
                    false,
                );
            }
        }
    }
}

impl AIComponent for RavagerManager {
    fn process(&mut self, bot: &mut Bot, _: &mut BotState) {
        let last_loop = self.last_loop;
        let game_loop = bot.state.observation.game_loop();
        if last_loop + Self::PROCESS_DELAY > game_loop {
            return;
        }
        self.last_loop = game_loop;

        self.cast_corrosive_bile(bot);
    }

    fn on_event(&mut self, _: &Event) {}
}
