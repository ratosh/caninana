use rust_sc2::bot::Bot;
use rust_sc2::geometry::Point3;
use rust_sc2::prelude::*;
use rust_sc2::Event::UnitDestroyed;

use crate::utils::IsDangerous;
use crate::{AIComponent, BotState};

#[derive(Default, Clone)]
pub struct Squad {
    pub squad: Units,
}

#[derive(Default)]
pub struct Squads {
    pub squads: Vec<Squad>,
}

impl Squad {
    fn influence_range(&self) -> f32 {
        3f32 + (self.squad.len() as f32).log10()
    }

    fn center(&self) -> Point2 {
        self.squad.sum(|u| u.position()) / self.squad.len() as f32
    }

    pub fn center3(&self) -> Point3 {
        self.squad.sum(|u| u.position3d()) / self.squad.len() as f32
    }

    fn is_close(&self, unit: &Unit) -> bool {
        unit.is_closer(
            unit.speed()
                + unit.real_ground_range().max(unit.real_air_range())
                + self.influence_range(),
            self.center(),
        )
    }
}

impl Squads {
    fn recalculate_squads(&mut self, bot: &mut Bot) {
        for unit in bot
            .units
            .my
            .units
            .filter(|f| !f.is_worker() && f.is_dangerous())
            .iter()
        {
            self.recalculate_unit_squad(unit);
        }
        // for squad in self.squads.iter() {
        //     bot.debug.draw_sphere(
        //         squad.center3(),
        //         squad.influence_range(),
        //         Some((255, 255, 255)),
        //     );
        // }
    }

    fn recalculate_unit_squad(&mut self, unit: &Unit) {
        for squad in self.squads.iter_mut() {
            squad.squad.remove(unit.tag());
        }
        self.squads.retain(|s| !s.squad.is_empty());
        let mut found_squad = false;
        for squad in self.squads.iter_mut() {
            if squad.is_close(unit) {
                found_squad = true;
                squad.squad.push(unit.clone());
                break;
            }
        }
        if !found_squad {
            let mut squad = Squad::default();
            squad.squad.push(unit.clone());
            self.squads.push(squad);
        }
    }

    fn destroy_unit(&mut self, tag: u64) {
        for squad in self.squads.iter_mut() {
            squad.squad.remove(tag);
        }
    }

    pub fn find_unit_squad(&self, unit: &Unit) -> Option<&Squad> {
        for squad in self.squads.iter() {
            if squad.squad.contains_tag(unit.tag()) {
                return Some(squad);
            }
        }
        None
    }

    pub fn find_squads_close_by(&self, unit: &Unit) -> Vec<Squad> {
        let mut result = vec![];
        for squad in self.squads.iter() {
            if squad.is_close(unit) {
                result.push(squad.clone());
            }
        }
        result
    }
}

#[derive(Default)]
pub struct SquadManager {}

impl AIComponent for SquadManager {
    fn process(&mut self, bot: &mut Bot, bot_state: &mut BotState) {
        bot_state.squads.recalculate_squads(bot);
    }

    fn on_event(&mut self, event: &Event, bot_state: &mut BotState) {
        if let UnitDestroyed(tag, _) = event {
            bot_state.squads.destroy_unit(*tag);
        }
    }
}
