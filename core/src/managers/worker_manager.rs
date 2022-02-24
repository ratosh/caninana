use std::collections::{HashMap, HashSet, VecDeque};

use itertools::Itertools;

use log::debug;
use rust_sc2::bot::Bot;
use rust_sc2::prelude::*;

use crate::command_queue::Command;
use crate::params::MAX_WORKERS;
use crate::utils::{PathingDistance, UnitOrderCheck};
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
        let defense_range = 19f32;
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
                u.is_worker()
                    && close_units.closest_distance(u.position()).unwrap_or_max()
                        < surroundings_range
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
                    <= defense_range
        });
        let spines_close = enemy_buildings_close
            .of_type(UnitTypeId::SpineCrawler)
            .len();
        let pylons_close = enemy_buildings_close.of_type(UnitTypeId::Pylon).len();
        let cannons_close = enemy_buildings_close
            .of_type(UnitTypeId::PhotonCannon)
            .len();
        let current_fighters = self
            .worker_decision
            .values()
            .filter(|f| **f == WorkerDecision::Fight)
            .count();

        let army_supply = bot
            .units
            .my
            .units
            .filter(|f| f.is_ready() && !f.is_worker())
            .sum(|f| f.supply_cost()) as usize;

        let mut needed_fighters = 0;
        if spines_close > 0 {
            needed_fighters += spines_close * 5
        }
        if pylons_close > 0 {
            needed_fighters += pylons_close * 5
        }
        if cannons_close > 0 {
            needed_fighters += cannons_close * 4
        }
        if weak_attackers > 0 {
            needed_fighters += weak_attackers + 1
        }
        debug!(
            "U[{:?}] W[{:?}] S[{:?}] F[{:?}] NF[{:?}]",
            units_attacking,
            weak_attackers,
            enemy_buildings_close.len(),
            current_fighters,
            needed_fighters
        );
        needed_fighters = needed_fighters.saturating_sub(army_supply);

        for worker in bot
            .units
            .my
            .workers
            .iter()
            .sorted_by(|a, b| b.hits().cmp(&a.hits()).then(a.tag().cmp(&b.tag())))
        {
            let close_attackers = !bot
                .units
                .enemy
                .units
                .filter(|f| {
                    f.can_attack_unit(worker) && f.in_range(worker, f.speed() + worker.speed())
                })
                .is_empty();
            let decision = if needed_fighters > 0 {
                needed_fighters -= 1;
                WorkerDecision::Fight
            } else if close_attackers
                && (weak_attackers > 1 && weak_attackers == units_attacking
                    || worker.hits_percentage().unwrap_or_default() <= 0.5)
            {
                WorkerDecision::Run
            } else if worker.is_constructing() {
                WorkerDecision::Build
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
            .sorted(|t| t.distance(bot.start_location))
            .iter()
        {
            let mut minerals = VecDeque::new();
            for mineral in bot
                .units
                .mineral_fields
                .closer(9f32, townhall.position())
                .iter()
                .map(|m| m.tag())
            {
                match self.resources.get(&mineral) {
                    None => {
                        minerals.push_front(mineral);
                        minerals.push_back(mineral);
                    }
                    Some(assigned) => {
                        for _ in assigned.len()..Self::MINERAL_WORKERS {
                            minerals.push_back(mineral);
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
                .ready()
                .filter(|u| u.vespene_contents().unwrap_or_default() > 0)
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
        let gatherers = self
            .worker_decision
            .iter()
            .filter(|(_, decision)| **decision == WorkerDecision::Gather)
            .map(|(worker, _)| *worker)
            .collect::<Vec<u64>>();
        for worker in gatherers {
            if !self.assignment.contains_key(&worker) {
                // TODO: Consider distance
                if let Some(resource) = resources.pop_front() {
                    self.assign_worker(worker, resource);
                } else if let Some(unit) = bot.units.my.workers.get(worker) {
                    if let Some(long_mineral) = bot
                        .units
                        .mineral_fields
                        .iter()
                        .filter(|m| !self.resources.contains_key(&m.tag()))
                        .closest(unit.position())
                    {
                        self.assign_worker(worker, long_mineral.tag());
                    }
                }
            }
        }
    }

    fn micro(&mut self, bot: &mut Bot) {
        let advance_mineral = bot.units.mineral_fields.closest(bot.enemy_start).unwrap();
        let retreat_mineral = bot
            .units
            .mineral_fields
            .closest(bot.start_location)
            .unwrap();
        for worker in bot.units.my.workers.iter() {
            let decision = self
                .worker_decision
                .get(&worker.tag())
                .unwrap_or(&WorkerDecision::Gather);
            match decision {
                WorkerDecision::Run => {
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
                WorkerDecision::Fight => {
                    let close_targets = bot.units.enemy.all.filter(|u| {
                        worker.can_attack_unit(u) && worker.in_range(u, worker.speed())
                    });
                    let closest_to_base = bot
                        .units
                        .enemy
                        .all
                        .iter()
                        .filter(|u| worker.can_attack_unit(u))
                        .closest(bot.start_location);
                    let closest_target = bot
                        .units
                        .enemy
                        .all
                        .iter()
                        .filter(|u| worker.can_attack_unit(u) && worker.in_range(u, 3f32))
                        .closest(worker);
                    if worker.on_cooldown() {
                        worker.order_gather(retreat_mineral.tag(), false);
                    } else if let Some(target) = close_targets.closest(worker) {
                        worker.order_attack(Target::Tag(target.tag()), false);
                    } else if let Some(target) = closest_target {
                        if !target.is_worker() {
                            worker.order_attack(Target::Tag(target.tag()), false);
                        } else if bot
                            .pathing_distance(target.position(), retreat_mineral.position())
                            > bot.pathing_distance(worker.position(), retreat_mineral.position())
                        {
                            worker.order_gather(advance_mineral.tag(), false);
                        } else {
                            worker.order_gather(retreat_mineral.tag(), false);
                        }
                    } else if let Some(target) = closest_to_base {
                        worker.order_attack(Target::Tag(target.tag()), false);
                    } else {
                        worker.order_gather(retreat_mineral.tag(), false);
                    }
                }
                WorkerDecision::Gather => {
                    let assignment = self.assignment.get(&worker.tag());
                    if worker.is_carrying_resource() && !worker.is_returning() {
                        worker.return_resource(false);
                    } else if !worker.is_carrying_resource() || worker.is_idle() {
                        if let Some(worker_assignment) = worker.target_tag() {
                            if worker_assignment != *assignment.unwrap() {
                                worker.order_gather(*assignment.unwrap(), false);
                            }
                        } else {
                            worker.order_gather(*assignment.unwrap(), false);
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
            .map(|e| e.ideal_harvesters().unwrap_or_default())
            .sum::<u32>();

        let wanted_workers = (ideal_miners + ideal_geysers).min(MAX_WORKERS as u32);

        bot_state.build_queue.push(
            Command::new_unit(UnitTypeId::Drone, wanted_workers as usize, false),
            false,
            25,
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
