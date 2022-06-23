use std::cmp::Ordering;
use std::collections::{HashMap, HashSet, VecDeque};

use itertools::Itertools;

use log::debug;
use rust_sc2::bot::Bot;
use rust_sc2::prelude::*;

use crate::command_queue::Command;
use crate::params::*;
use crate::utils::Center;
use crate::utils::*;
use crate::*;

#[derive(PartialEq, Debug, Clone)]
enum WorkerDecision {
    Run,
    Fight,
    Gather,
    Build,
}

#[derive(Default)]
pub struct WorkerManager {
    worker_decision: HashMap<u64, WorkerDecision>,

    // (worker_tag, resource_tag)
    assignment: HashMap<u64, u64>,
    // (resource_tag, worker_tag)
    resources: HashMap<u64, HashSet<u64>>,
    worker_defense: bool,
}

impl WorkerManager {
    fn unit_destroyed(&mut self, tag: u64) {
        debug!("Unit destroyed {:?}", tag);
        self.unassign_worker(tag);
        self.worker_decision.remove(&tag);
    }

    fn unassign_resource(&mut self, tag: u64) {
        debug!("Unassign resource {:?}", tag);
        if let Some(workers) = self.resources.remove(&tag) {
            for w in workers {
                self.assignment.remove(&w);
            }
        }
    }

    fn unassign_worker(&mut self, tag: u64) {
        if let Some(resource) = self.assignment.remove(&tag) {
            self.resources.entry(resource).and_modify(|workers| {
                workers.remove(&tag);
            });
        }
    }

    fn assign_worker(&mut self, worker: u64, resource: u64) {
        self.assignment.insert(worker, resource);
        self.resources.entry(resource).or_default().insert(worker);
    }
}

impl WorkerManager {
    const MINERAL_WORKERS: usize = 2;
    const GEYSERS_WORKERS: usize = 3;

    fn decision(&mut self, bot: &mut Bot) {
        let defense_range = bot
            .start_location
            .distance(bot.ramps.my.points.center_point().unwrap());
        let surroundings_range = 15f32;

        let close_units = bot.units.enemy.units.filter(|f| {
            f.can_attack_ground()
                && bot
                    .units
                    .my
                    .townhalls
                    .closest_distance(f.position())
                    .unwrap_or_max()
                    <= defense_range
        });

        let units_attacking = bot
            .units
            .enemy
            .all
            .filter(|u| {
                u.can_attack_ground()
                    && close_units.closest_distance(u.position()).unwrap_or_max()
                        < surroundings_range
            })
            .len();

        let weak_attackers = bot
            .units
            .enemy
            .all
            .filter(|u| {
                (u.is_worker() || u.type_id() == UnitTypeId::Zergling)
                    && !close_units.in_range(u, surroundings_range).is_empty()
            })
            .len();
        let enemy_buildings_close = bot.units.enemy.all.filter(|f| {
            !f.is_ready()
                && bot
                    .units
                    .my
                    .townhalls
                    .closest_distance(f.position())
                    .unwrap_or_max()
                    <= defense_range * 2f32
        });
        let spines_close = enemy_buildings_close
            .of_type(UnitTypeId::SpineCrawler)
            .len();
        let pylons_close = enemy_buildings_close.of_type(UnitTypeId::Pylon).len();
        let cannons_close = enemy_buildings_close
            .of_type(UnitTypeId::PhotonCannon)
            .len();
        let halls_close = enemy_buildings_close
            .of_types(&vec![
                UnitTypeId::CommandCenter,
                UnitTypeId::PlanetaryFortress,
            ])
            .len();

        let army_supply = bot
            .units
            .my
            .units
            .filter(|f| f.is_ready() && !f.is_worker() && f.type_id() != UnitTypeId::Queen)
            .sum(|f| f.supply_cost()) as usize;

        let mut needed_fighters = spines_close * 5
            + pylons_close * 5
            + cannons_close * 4
            + 8 * halls_close
            + weak_attackers;
        if weak_attackers > 1 {
            needed_fighters += 1;
        }
        debug!(
            "U[{:?}] W[{:?}] S[{:?}] NF[{:?}]",
            units_attacking,
            weak_attackers,
            enemy_buildings_close.len(),
            needed_fighters
        );
        needed_fighters = needed_fighters.saturating_sub(army_supply);
        self.worker_defense = self.worker_defense || weak_attackers > 5;

        for worker in bot
            .units
            .my
            .all
            .of_types(&vec![UnitTypeId::Drone, UnitTypeId::DroneBurrowed])
            .iter()
            .sorted_by(|a, b| b.hits().cmp(&a.hits()).then(a.tag().cmp(&b.tag())))
        {
            let close_attackers = !bot
                .units
                .enemy
                .units
                .filter(|f| f.can_attack_unit(worker) && f.in_range(worker, f.speed()))
                .is_empty();
            let decision = if worker.is_constructing() {
                WorkerDecision::Build
            } else if needed_fighters > 0 {
                needed_fighters -= 1;
                WorkerDecision::Fight
            } else if close_attackers
                && (worker.hits_percentage().unwrap_or_default() <= 0.55f32
                    || units_attacking > weak_attackers.max(1) * 2)
            {
                WorkerDecision::Run
            } else {
                WorkerDecision::Gather
            };
            debug!(
                "W[{:?}] H[{:?}] D[{:?}]",
                worker.tag(),
                worker.hits(),
                decision.clone()
            );
            self.worker_decision.insert(worker.tag(), decision);
        }
    }

    // TODO: Remove from long distance mining if needed.
    fn assignment(&mut self, bot: &Bot) {
        {
            let clear_assignment = self
                .worker_decision
                .iter()
                .filter(|(worker_tag, decision)| {
                    if let Some(worker) = bot.units.all.get(**worker_tag) {
                        if worker.is_returning() {
                            if let Some(target) = worker.target_tag() {
                                if let Some(unit) = bot.units.my.townhalls.get(target) {
                                    return worker.distance(unit) > 9f32;
                                }
                            }
                        }
                        worker.is_idle() || **decision != WorkerDecision::Gather
                    } else {
                        **decision != WorkerDecision::Gather
                    }
                })
                .map(|(worker, _)| *worker)
                .collect::<Vec<u64>>();

            for worker in clear_assignment {
                self.unassign_worker(worker);
            }

            let clear_resources = self
                .resources
                .iter()
                .filter(|(resource, _)| {
                    if let Some(unit) = bot.units.all.get(**resource) {
                        unit.mineral_contents().unwrap_or_default()
                            + unit.vespene_contents().unwrap_or_default()
                            == 0
                    } else if let Some(building) = bot.units.my.gas_buildings.get(**resource) {
                        building.mineral_contents().unwrap_or_default()
                            + building.vespene_contents().unwrap_or_default()
                            == 0
                    } else if let Some(resource) = bot.units.resources.get(**resource) {
                        resource.mineral_contents().unwrap_or_default()
                            + resource.vespene_contents().unwrap_or_default()
                            == 0
                    } else {
                        true
                    }
                })
                .map(|(resource, _)| *resource)
                .collect::<Vec<u64>>();

            for resource in clear_resources {
                self.unassign_resource(resource);
            }
        }
        let mut resources = VecDeque::new();
        for townhall in bot
            .units
            .my
            .townhalls
            .sorted(|t| {
                if t.is_ready() {
                    t.ideal_harvesters()
                        .unwrap_or(12)
                        .saturating_sub(t.assigned_harvesters().unwrap_or_default())
                } else {
                    18
                }
            })
            .iter()
        {
            let mut minerals = VecDeque::new();
            for mineral in bot
                .units
                .mineral_fields
                .closer(9f32, townhall.position())
                .iter()
                .sorted_by(|a, b| {
                    a.distance(townhall)
                        .partial_cmp(&b.distance(townhall))
                        .unwrap_or(Ordering::Equal)
                })
                .map(|m| m.tag())
            {
                match self.resources.get(&mineral) {
                    None => {
                        minerals.push_front(mineral);
                        minerals.push_back(mineral);
                    }
                    Some(assigned) => {
                        for i in assigned.len()..Self::MINERAL_WORKERS {
                            match i {
                                0 => minerals.push_front(mineral),
                                _ => minerals.push_back(mineral),
                            }
                        }
                    }
                }
            }
            for mineral in minerals {
                resources.push_back(mineral);
            }
            for geyser in bot
                .units
                .my
                .gas_buildings
                .filter(|u| u.is_almost_ready() && u.vespene_contents().unwrap_or_default() > 0)
                .closer(9f32, townhall.position())
                .iter()
                .map(|g| g.tag())
            {
                let missing = if let Some(workers) = self.resources.get(&geyser) {
                    Self::GEYSERS_WORKERS - workers.len()
                } else {
                    Self::GEYSERS_WORKERS
                };
                for _ in 0..missing {
                    resources.push_back(geyser);
                }
            }
        }
        let reference_expansion = bot
            .free_expansions()
            .map(|e| e.loc)
            .next()
            .unwrap_or(bot.start_location);
        let long_mineral = bot
            .units
            .mineral_fields
            .iter()
            .filter(|m| !self.resources.contains_key(&m.tag()))
            .sorted_by(|a, b| {
                reference_expansion
                    .distance(a.position())
                    .partial_cmp(&reference_expansion.distance(b.position()))
                    .unwrap_or(Ordering::Equal)
            })
            .next();
        let gatherers = self
            .worker_decision
            .iter()
            .filter(|(_, decision)| **decision == WorkerDecision::Gather)
            .map(|(worker, _)| *worker)
            .collect::<Vec<u64>>();
        for worker in gatherers {
            if !self.assignment.contains_key(&worker) {
                if let Some(resource) = resources.pop_front() {
                    self.assign_worker(worker, resource);
                } else if let Some(mineral) = long_mineral {
                    self.assign_worker(worker, mineral.tag());
                }
            }
        }
    }

    fn micro(&mut self, bot: &mut Bot) {
        let retreat_mineral = bot
            .units
            .mineral_fields
            .closest(bot.start_location)
            .unwrap();
        for burrowed_worker in bot.units.my.all.of_type(UnitTypeId::DroneBurrowed) {
            if self
                .worker_decision
                .get(&burrowed_worker.tag())
                .unwrap_or(&WorkerDecision::Gather)
                != &WorkerDecision::Run
            {
                burrowed_worker.use_ability(AbilityId::BurrowUpDrone, false);
            }
        }
        for worker in bot.units.my.workers.iter() {
            let decision = self
                .worker_decision
                .get(&worker.tag())
                .unwrap_or(&WorkerDecision::Gather);
            match decision {
                WorkerDecision::Run => {
                    if bot.has_upgrade(UpgradeId::Burrow) {
                        worker.use_ability(AbilityId::BurrowDownDrone, false);
                    } else {
                        let attackers = bot.units.enemy.units.filter(|f| {
                            f.can_attack_unit(worker)
                                && f.in_range(worker, 1f32 + f.speed() + worker.speed())
                        });
                        if !attackers.filter(|u| u.is_worker()).is_empty() {
                            worker.order_gather(retreat_mineral.tag(), false);
                        } else if let Some(run_from) = attackers.center() {
                            worker.order_move_to(
                                Target::Pos(worker.position().towards(run_from, -worker.speed())),
                                0.1f32,
                                false,
                            );
                        }
                    }
                }
                WorkerDecision::Fight => {
                    let weakest_in_range = bot
                        .units
                        .enemy
                        .all
                        .iter()
                        .filter(|u| worker.can_attack_unit(u) && worker.in_range(u, 0.3f32))
                        .sorted_by(|a, b| a.hits().cmp(&b.hits()))
                        .next();
                    let closest_to_base = bot
                        .units
                        .enemy
                        .all
                        .iter()
                        .filter(|u| worker.can_attack_unit(u))
                        .closest(retreat_mineral.position());
                    if worker.weapon_cooldown().unwrap_or_default() > 5f32 {
                        worker.order_gather(retreat_mineral.tag(), false);
                    } else if worker.is_carrying_resource() {
                        if !worker.is_returning() {
                            worker.return_resource(false);
                        }
                    } else if let Some(target) = weakest_in_range {
                        worker.order_attack(Target::Tag(target.tag()), false);
                    } else if let Some(target) = closest_to_base {
                        worker.order_attack(Target::Pos(target.position()), false);
                    } else {
                        worker.order_gather(retreat_mineral.tag(), false);
                    }
                }
                WorkerDecision::Gather => {
                    let assignment = self.assignment.get(&worker.tag());
                    if let Some(current_assignment) = assignment {
                        if worker.is_carrying_resource()
                            && !worker.is_returning()
                            && !bot.units.my.townhalls.is_empty()
                        {
                            worker.return_resource(false);
                        }
                        if !worker.is_carrying_resource() || worker.is_idle() {
                            if let Some(worker_assignment) = worker.target_tag() {
                                if worker_assignment != *current_assignment {
                                    worker.order_gather(*current_assignment, false);
                                }
                            } else {
                                worker.order_gather(*current_assignment, false);
                            }
                        }
                    }
                }
                WorkerDecision::Build => {
                    debug!(
                        "Worker {:?} is building {:?}",
                        worker.tag(),
                        worker.ordered_ability()
                    );
                }
            }
        }
    }

    fn queue_worker(&self, bot: &mut Bot, bot_state: &mut BotState) {
        let ideal_miners = bot
            .units
            .my
            .townhalls
            .iter()
            .map(|e| {
                if e.is_ready() {
                    e.ideal_harvesters().unwrap_or_default()
                } else {
                    12
                }
            })
            .sum::<u32>();

        let ideal_geysers = bot
            .units
            .my
            .gas_buildings
            .iter()
            .map(|e| e.ideal_harvesters().unwrap_or_default().saturating_sub(1))
            .sum::<u32>();

        let ideal_workers = MAX_WORKERS.min((ideal_miners + ideal_geysers) as usize);
        let min_extra_workers = match bot_state.spending_focus {
            SpendingFocus::Economy => bot.owned_expansions().count() * 2,
            SpendingFocus::Balance => bot.owned_expansions().count() + 1,
            SpendingFocus::Army => 1,
        };
        let drones = bot
            .counter()
            .all()
            .count(bot.race_values.worker)
            .saturating_sub(bot.counter().count(UnitTypeId::DroneBurrowed));
        let min_workers =
            if bot.counter().ordered().count(bot.race_values.worker) < min_extra_workers {
                ideal_workers.min(drones + min_extra_workers)
            } else {
                ideal_workers.min(drones)
            };
        if self.worker_defense {
            bot_state.build_queue.push(
                Command::new_unit(bot.race_values.worker, 16, false),
                true,
                9999,
            );
        }
        bot_state.build_queue.push(
            Command::new_unit(bot.race_values.worker, min_workers, false),
            true,
            PRIORITY_DRONE_ECONOMY,
        );
        let ideal_priority = if bot_state.spending_focus == SpendingFocus::Economy {
            100
        } else {
            15
        };

        bot_state.build_queue.push(
            Command::new_unit(bot.race_values.worker, ideal_workers, false),
            false,
            ideal_priority,
        );
    }
}

impl AIComponent for WorkerManager {
    fn process(&mut self, bot: &mut Bot, bot_state: &mut BotState) {
        self.decision(bot);
        self.assignment(bot);
        self.micro(bot);
        self.queue_worker(bot, bot_state);
    }

    fn on_event(&mut self, event: &Event, _: &mut BotState) {
        if let Event::UnitDestroyed(tag, alliance) = event {
            match alliance {
                Some(Alliance::Own) => {
                    self.unit_destroyed(*tag);
                }
                // mineral mined out
                Some(Alliance::Neutral) => self.unassign_resource(*tag),
                _ => {}
            }
        }
    }
}
