use log::debug;
use rust_sc2::action::ActionResult;
use rust_sc2::bot::Bot;
use rust_sc2::prelude::*;

use crate::params::*;
use crate::utils::PathingDistance;
use crate::{AIComponent, BotState, UnwrapOrMax};

#[derive(Default)]
pub struct QueenManager {
    spread_map: Vec<Point2>,
}

impl QueenManager {
    fn handle_transfusion(&mut self, bot: &mut Bot) {
        let queens = bot.units.my.units.of_type(UnitTypeId::Queen).filter(|u| {
            !u.is_using(AbilityId::EffectInjectLarva)
                && u.energy().unwrap_or_default() > TRANSFUSION_MIN_ENERGY
                && u.has_ability(AbilityId::TransfusionTransfusion)
        });

        for queen in queens.iter() {
            if let Some(heal) = bot
                .units
                .my
                .all
                .filter(|u| {
                    u.position().distance(queen.position()) < TRANSFUSION_MAX_RANGE
                        && u.health_max().unwrap_or_default() - u.health().unwrap_or_default()
                            > TRANSFUSION_MISSING_HEALTH
                })
                .closest(queen)
            {
                queen.command(
                    AbilityId::TransfusionTransfusion,
                    Target::Tag(heal.tag()),
                    false,
                );
            }
        }
    }

    fn handle_spread(&mut self, bot: &mut Bot) {
        if self.spread_map.is_empty() {
            self.spread_map = bot.create_creep_spread_map();
        }
        let tumors = bot.units.my.all.of_type(UnitTypeId::CreepTumorBurrowed);

        tumors
            .filter(|u| u.has_ability(AbilityId::BuildCreepTumorTumor))
            .iter()
            .for_each(|h| {
                if let Some(closest_spot) = self
                    .spread_map
                    .iter()
                    .filter(|&p| {
                        (!bot.is_visible((p.x as usize, p.y as usize))
                            || !bot.has_creep((p.x as usize, p.y as usize)))
                            && (h.position().distance(p) * 1.5
                                >= bot.pathing_distance(h.position(), *p).unwrap_or_max())
                    })
                    .closest(h.position())
                {
                    if let Some(position) = bot.find_creep_placement(h, *closest_spot) {
                        h.command(
                            AbilityId::BuildCreepTumorTumor,
                            Target::Pos(position),
                            false,
                        );
                    }
                }
            });

        if let Some(queen) = bot
            .units
            .my
            .units
            .filter(|u| {
                !u.is_using(AbilityId::EffectInjectLarva)
                    && !u.is_using(AbilityId::BuildCreepTumorQueen)
                    && u.has_ability(AbilityId::BuildCreepTumorQueen)
                    && u.energy().unwrap_or_default() > CREEP_SPREAD_ENERGY
            })
            .first()
        {
            if let Some(closest_spot) = self
                .spread_map
                .iter()
                .filter(|&p| {
                    (!bot.is_visible((p.x as usize, p.y as usize))
                        || !bot.has_creep((p.x as usize, p.y as usize)))
                        && (queen.position().distance(p)
                            >= bot.pathing_distance(queen.position(), *p).unwrap_or_max())
                })
                .closest(queen.position())
            {
                if let Some(position) =
                    bot.find_placement(UnitTypeId::CreepTumor, *closest_spot, Default::default())
                {
                    queen.command(
                        AbilityId::BuildCreepTumorQueen,
                        Target::Pos(position),
                        false,
                    );
                }
            }
        }
    }

    fn handle_injection(&mut self, bot: &mut Bot) {
        if bot.units.my.larvas.len() > INJECTION_MAX_LARVA {
            return;
        }
        let mut queens = bot.units.my.units.of_type(UnitTypeId::Queen).filter(|u| {
            !u.is_using(AbilityId::EffectInjectLarva)
                && u.has_ability(AbilityId::EffectInjectLarva)
                && !u.is_attacked()
        });
        let injecting_queens = bot
            .units
            .my
            .units
            .of_type(UnitTypeId::Queen)
            .filter(|u| u.is_using(AbilityId::EffectInjectLarva));
        if !queens.is_empty() {
            for base in bot.units.my.townhalls.iter().filter(|h| {
                !h.has_buff(BuffId::QueenSpawnLarvaTimer)
                    && injecting_queens
                        .filter(|q| q.target_tag() == Some(h.tag()))
                        .is_empty()
            }) {
                debug!("Need to inject in base {}", base.tag());
                if let Some(queen) = queens.closest(base) {
                    queen.command(AbilityId::EffectInjectLarva, Target::Tag(base.tag()), false);
                    let queen_tag = queen.tag();
                    queens.remove(queen_tag);
                } else {
                    debug!("Unable to find a queen to inject");
                }
            }
        }
    }
}

trait CreepMap {
    fn create_creep_spread_map(&self) -> Vec<Point2>;
}

impl CreepMap for Bot {
    fn create_creep_spread_map(&self) -> Vec<Point2> {
        let mut result = vec![];
        for x in (self.game_info.playable_area.x0..self.game_info.playable_area.x1)
            .step_by(CREEP_SPREAD_MAP_DISTANCE)
        {
            for y in (self.game_info.playable_area.y0..self.game_info.playable_area.y1)
                .step_by(CREEP_SPREAD_MAP_DISTANCE)
            {
                let point = Point2::new(x as f32, y as f32);
                if self.is_placeable(point)
                    && self
                        .expansions
                        .iter()
                        .map(|e| e.loc)
                        .closest_distance(point)
                        .unwrap_or_default()
                        > CREEP_SPREAD_MAP_DISTANCE as f32
                {
                    result.push(point);
                }
            }
        }
        result
    }
}

trait CreepPlacement {
    fn find_creep_placement(&self, unit: &Unit, pos: Point2) -> Option<Point2>;
}

impl CreepPlacement for Bot {
    fn find_creep_placement(&self, unit: &Unit, spot: Point2) -> Option<Point2> {
        if let Some(data) = self.game_data.units.get(&UnitTypeId::CreepTumor) {
            if let Some(ability) = data.ability {
                let placement_step = 1;
                let range = CREEP_SPREAD_DISTANCE as f32;
                let near = unit.position().towards(spot, range);
                for distance in (placement_step..(range as i32)).step_by(placement_step as usize) {
                    let positions = (-distance..=distance)
                        .step_by(placement_step as usize)
                        .flat_map(|offset| {
                            vec![
                                near.offset(offset as f32, (-distance) as f32),
                                near.offset(offset as f32, distance as f32),
                                near.offset((-distance) as f32, offset as f32),
                                near.offset(distance as f32, offset as f32),
                            ]
                        })
                        .filter(|p| {
                            if let Some(exp) = self.expansions.iter().map(|e| e.loc).closest(p) {
                                (exp.x - p.x).abs() > CREEP_DISTANCE_TO_HALL
                                    && (exp.y - p.y).abs() > CREEP_DISTANCE_TO_HALL
                            } else {
                                false
                            }
                        })
                        .collect::<Vec<Point2>>();
                    if let Ok(results) = self.query_placement(
                        positions
                            .iter()
                            .map(|pos| (ability, *pos, Some(unit.tag())))
                            .collect(),
                        false,
                    ) {
                        let valid_positions = positions
                            .iter()
                            .zip(results.iter())
                            .filter_map(|(pos, res)| {
                                if matches!(res, ActionResult::Success) {
                                    Some(*pos)
                                } else {
                                    None
                                }
                            })
                            .collect::<Vec<Point2>>();

                        if !valid_positions.is_empty() {
                            return valid_positions.iter().closest(spot).cloned();
                        }
                    }
                }
            }
        }
        None
    }
}

impl AIComponent for QueenManager {
    fn process(&mut self, bot: &mut Bot, _: &mut BotState) {
        self.handle_injection(bot);
        self.handle_spread(bot);
        self.handle_transfusion(bot);
    }
}
