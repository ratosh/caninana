use std::collections::{HashMap, HashSet, VecDeque};

use log::debug;
use rust_sc2::bot::Bot;
use rust_sc2::prelude::*;

use crate::command_queue::Command;
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
    last_loop: u32,
    worker_decision: HashMap<u64, WorkerDecision>,

    // (worker_tag, resource_tag)
    assignment: HashMap<u64, u64>,
    // (resource_tag, worker_tag)
    resources: HashMap<u64, HashSet<u64>>,
}

impl WorkerManager {
    fn unit_destroyed(&mut self, tag: u64) {
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
    const PROCESS_DELAY: u32 = 10;
    const MINERAL_WORKERS: usize = 2;
    const GEYSERS_WORKERS: usize = 3;

    fn decision(&mut self, bot: &mut Bot) {
        let units_attacking = bot
            .units
            .enemy
            .units
            .filter(|f| f.can_attack_ground() && f.is_closer(15f32, bot.start_location))
            .len();
        let workers_attacking = bot
            .units
            .enemy
            .workers
            .filter(|f| f.is_closer(15f32, bot.start_location))
            .len();
        let spines_close = bot
            .units
            .enemy
            .all
            .filter(|f| {
                f.is_closer(15f32, bot.start_location)
                    && f.type_id() == UnitTypeId::SpineCrawler
                    && !f.is_ready()
            })
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
        println!(
            "U[{:?}] W[{:?}] S[{:?}] F[{:?}]",
            units_attacking, workers_attacking, spines_close, current_fighters
        );

        let mut needed_fighters = 0;
        if spines_close > 0 {
            needed_fighters += (spines_close * 5).max(current_fighters)
        }
        if workers_attacking > 0 {
            needed_fighters += (workers_attacking * 12 / 10).max(current_fighters)
        }
        needed_fighters = needed_fighters.saturating_sub(army_supply);
        let back_threshold = if units_attacking > workers_attacking {
            0.5f32
        } else {
            0.25f32
        };

        for worker in bot.units.my.workers.sorted(|f| f.tag()).iter() {
            let attackers_in_range = !bot
                .units
                .enemy
                .units
                .filter(|f| {
                    f.can_attack_ground()
                        && f.in_range(worker, 2f32 + f.speed() + worker.speed())
                })
                .is_empty();
            let decision = if worker.health_percentage().unwrap_or_default() < back_threshold
                && attackers_in_range
            {
                WorkerDecision::Run
            } else if needed_fighters > 0 {
                needed_fighters -= 1;
                WorkerDecision::Fight
            } else if worker.is_attacked() {
                WorkerDecision::Fight
            } else if worker.is_constructing() {
                WorkerDecision::Build
            } else {
                WorkerDecision::Gather
            };
            self.worker_decision.insert(worker.tag(), decision);
        }
    }

    // TODO: Remove from long distance mining if needed.
    fn assignment(&mut self, bot: &Bot) {
        {
            let clear_assignement = self
                .worker_decision
                .iter()
                .filter(|(worker_tag, decision)| {
                    if let Some(unit) = bot.units.all.get(**worker_tag) {
                        unit.is_idle() || **decision != WorkerDecision::Gather
                    } else {
                        **decision != WorkerDecision::Gather
                    }
                })
                .map(|(worker, _)| *worker)
                .collect::<Vec<u64>>();

            for worker in clear_assignement {
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
                    } else {
                        bot.units.resources.get(**resource).is_none()
                            && bot.units.my.gas_buildings.get(**resource).is_none()
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
                        if assigned.len() < Self::MINERAL_WORKERS {
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
        for worker in bot.units.my.workers.iter() {
            let decision = self
                .worker_decision
                .get(&worker.tag())
                .unwrap_or(&WorkerDecision::Gather);
            match decision {
                WorkerDecision::Run => {
                    if let Some(run_from) = bot.units.enemy.units.closest(worker) {
                        worker.move_to(
                            Target::Pos(
                                worker
                                    .position()
                                    .towards(run_from.position(), -worker.speed()),
                            ),
                            false,
                        );
                    }
                }
                WorkerDecision::Fight => {
                    if let Some(target) = bot.units.enemy.all.closest(worker) {
                        worker.attack(Target::Pos(target.position()), false);
                    }
                }
                WorkerDecision::Gather => {
                    let assignment = self.assignment.get(&worker.tag());
                    if !worker.is_carrying_resource() || worker.is_idle() {
                        if let Some(worker_assignment) = worker.target_tag() {
                            if worker_assignment != *assignment.unwrap() {
                                worker.gather(*assignment.unwrap(), false);
                            }
                        } else {
                            worker.gather(*assignment.unwrap(), false);
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

    fn queue_worker(&self, bot: &mut Bot, bot_info: &mut BotInfo) {
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

        let wanted_workers = 80.min(ideal_miners + ideal_geysers);

        bot_info.build_queue.push(
            Command::new_unit(UnitTypeId::Drone, wanted_workers as usize, false),
            false,
            25,
        );
    }
}

impl Manager for WorkerManager {
    fn process(&mut self, bot: &mut Bot, bot_info: &mut BotInfo) {
        let last_loop = self.last_loop;
        let game_loop = bot.state.observation.game_loop();
        if last_loop + Self::PROCESS_DELAY > game_loop {
            return;
        }
        self.last_loop = game_loop;

        self.decision(bot);
        self.assignment(bot);
        self.micro(bot);
        self.queue_worker(bot, bot_info);
    }
}

impl EventListener for WorkerManager {
    fn on_event(&mut self, event: &Event) {
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
