use log::debug;
use rust_sc2::action::ActionResult;
use rust_sc2::bot::Bot;
use rust_sc2::prelude::*;

use crate::{BotInfo, Manager};

#[derive(Default)]
pub struct QueenManager {
    last_loop: u32,
    spread_map: Vec<Point2>,
}

impl QueenManager {
    const PROCESS_DELAY: u32 = 15;

    fn handle_transfusion(&mut self, bot: &mut Bot) {
        let queens = bot.units.my.units.of_type(UnitTypeId::Queen).filter(|u| {
            !u.is_using(AbilityId::EffectInjectLarva)
                && u.energy().unwrap_or(0) > 50
                && u.has_ability(AbilityId::TransfusionTransfusion)
        });

        for queen in queens.iter() {
            if let Some(heal) = bot
                .units
                .my
                .units
                .filter(|u| {
                    u.position().distance(queen.position()) < 15f32
                        && u.health_max().unwrap() - u.health().unwrap() > 60
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
        let tumors = bot
            .units
            .my
            .all
            .of_type(UnitTypeId::CreepTumorBurrowed)
            .filter(|u| u.has_ability(AbilityId::BuildCreepTumorTumor));

        tumors.iter().for_each(|h| {
            if let Some(closest_spot) = self
                .spread_map
                .iter()
                .filter(|&p| {
                    (!bot.is_visible((p.x as usize, p.y as usize))
                        || !bot.has_creep((p.x as usize, p.y as usize)))
                        && (h.position().distance(p)
                            >= bot.pathing_distance(h.position(), p.clone()))
                })
                .closest(bot.start_location.between(h.position()))
            {
                if let Some(position) = bot.find_creep_placement(h, closest_spot.clone()) {
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
                u.type_id() == UnitTypeId::Queen
                    && u.is_idle()
                    && u.has_ability(AbilityId::BuildCreepTumorQueen)
                    && u.energy().unwrap() > 75
            })
            .first()
        {
            if let Some(position) =
                bot.find_placement(UnitTypeId::CreepTumor, queen.position(), Default::default())
            {
                queen.command(
                    AbilityId::BuildCreepTumorQueen,
                    Target::Pos(position),
                    false,
                );
            }
        }
    }

    fn handle_injection(&mut self, bot: &mut Bot) {
        let mut queens = bot.units.my.units.of_type(UnitTypeId::Queen).filter(|u| {
            !u.is_using(AbilityId::EffectInjectLarva) && u.has_ability(AbilityId::EffectInjectLarva)
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
                        .filter(|q| q.target_tag().unwrap() == h.tag())
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

const SPREAD_MAP_DISTANCE: usize = 4;
const CREEP_SPREAD_DISTANCE: usize = 5;

trait CreepMap {
    fn create_creep_spread_map(&self) -> Vec<Point2>;
}

impl CreepMap for Bot {
    fn create_creep_spread_map(&self) -> Vec<Point2> {
        let mut result = vec![];
        for x in (self.game_info.playable_area.x0..self.game_info.playable_area.x1)
            .step_by(SPREAD_MAP_DISTANCE)
        {
            for y in (self.game_info.playable_area.y0..self.game_info.playable_area.y1)
                .step_by(SPREAD_MAP_DISTANCE)
            {
                let point = Point2::new(x as f32, y as f32);
                if self.is_placeable(point)
                    && self
                        .expansions
                        .iter()
                        .map(|e| e.loc)
                        .closest_distance(point)
                        .unwrap_or(0f32)
                        > SPREAD_MAP_DISTANCE as f32
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
                if let Some(_) = self.game_data.abilities.get(&ability) {
                    let placement_step = 1;
                    let range = CREEP_SPREAD_DISTANCE as f32;
                    let near = unit.position().towards(spot, range);
                    for distance in
                        (placement_step..(range as i32)).step_by(placement_step as usize)
                    {
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
                                self.expansions
                                    .iter()
                                    .map(|e| e.loc)
                                    .closest_distance(p)
                                    .unwrap()
                                    > range
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
        }
        None
    }
}

trait Between {
    fn between(self, pos: Point2) -> Point2;
}

impl Between for Point2 {
    fn between(self, other: Self) -> Self {
        (self + other) / 2f32
    }
}

trait PathingDistance {
    fn pathing_distance(&self, p1: Point2, p2: Point2) -> f32;
}

impl PathingDistance for Bot {
    fn pathing_distance(&self, p1: Point2, p2: Point2) -> f32 {
        self.query_pathing(vec![(Target::Pos(p1), p2)])
            .unwrap_or(vec![])
            .iter()
            .map(|d| d.unwrap_or(0f32))
            .sum()
    }
}

impl Manager for QueenManager {
    fn process(&mut self, bot: &mut Bot, _: &mut BotInfo) {
        let last_loop = self.last_loop;
        let game_loop = bot.state.observation.game_loop();
        if last_loop + Self::PROCESS_DELAY > game_loop {
            return;
        }
        self.last_loop = game_loop;
        self.handle_injection(bot);
        self.handle_spread(bot);
        self.handle_transfusion(bot);
    }
}
