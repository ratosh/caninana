use rust_sc2::bot::Bot;
use rust_sc2::prelude::*;

use crate::{AIComponent, BotState};

#[derive(Default)]
pub struct RavagerManager {}

impl RavagerManager {
    const CORROSIVE_POSSIBLE_TARGETS: [UnitTypeId; 18] = [
        UnitTypeId::WidowMineBurrowed,
        UnitTypeId::SiegeTankSieged,
        UnitTypeId::Thor,
        UnitTypeId::Battlecruiser,
        UnitTypeId::LiberatorAG,
        UnitTypeId::PlanetaryFortress,
        UnitTypeId::Bunker,
        UnitTypeId::HighTemplar,
        UnitTypeId::Colossus,
        UnitTypeId::VoidRay,
        UnitTypeId::Carrier,
        UnitTypeId::Mothership,
        UnitTypeId::PhotonCannon,
        UnitTypeId::LurkerMPBurrowed,
        UnitTypeId::BroodLord,
        UnitTypeId::Infestor,
        UnitTypeId::SpineCrawler,
        UnitTypeId::SporeCrawler,
    ];

    fn cast_corrosive_bile(&self, bot: &mut Bot, bot_state: &BotState) {
        let ravagers = bot
            .units
            .my
            .units
            .filter(|f| f.has_ability(AbilityId::EffectCorrosiveBile));

        for ravager in ravagers {
            if let Some(target) = bot_state
                .enemy_cache
                .units()
                .filter(|u| {
                    ravager.in_ability_cast_range(AbilityId::EffectCorrosiveBile, *u, 0.0f32)
                        && (Self::CORROSIVE_POSSIBLE_TARGETS.contains(&u.type_id()))
                })
                .iter()
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
    fn process(&mut self, bot: &mut Bot, bot_state: &mut BotState) {
        self.cast_corrosive_bile(bot, bot_state);
    }
}
