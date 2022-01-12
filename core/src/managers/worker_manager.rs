use std::cmp::Ordering;

use rust_sc2::bot::Bot;
use rust_sc2::prelude::*;

use crate::command_queue::Command;
use crate::*;

#[derive(Default)]
pub struct WorkerManager {
    last_loop: u32,
}

impl WorkerManager {
    const PROCESS_DELAY: u32 = 10;

    // TODO: Remove workers from geysers on no_gas priority
    // TODO: Put workers on long distance mining (if needed)
    fn worker_harversting(&mut self, bot: &mut Bot, bot_info: &BotInfo) {
        let mut free_workers = bot.units.my.workers.idle();
        let mut deficit_minerals = Units::new();
        let mut deficit_geysers = Units::new();

        let bases = bot
            .units
            .my
            .all
            .filter(|b| bot.owned_expansions().any(|e| e.base.unwrap() == b.tag()));
        for base in bases {
            match base.assigned_harvesters().cmp(&base.ideal_harvesters()) {
                Ordering::Less => deficit_minerals.extend(
                    bot.units
                        .mineral_fields
                        .closer(11.0, base.position())
                        .iter()
                        .take(
                            (base.ideal_harvesters().unwrap() - base.assigned_harvesters().unwrap())
                                as usize,
                        )
                        .cloned(),
                ),
                Ordering::Greater => {
                    free_workers.extend(
                        bot.units
                            .my
                            .workers
                            .filter(|u| {
                                u.target_tag().map_or(false, |target_tag| {
                                    u.is_carrying_minerals() && target_tag == base.tag()
                                })
                            })
                            .iter()
                            .take(
                                (base.assigned_harvesters().unwrap()
                                    - base.ideal_harvesters().unwrap())
                                    as usize,
                            )
                            .cloned(),
                    );
                }
                _ => {}
            }
        }

        for geyser in bot.units.my.gas_buildings.ready().iter() {
            match geyser.assigned_harvesters().cmp(&geyser.ideal_harvesters()) {
                Ordering::Less => {
                    deficit_geysers.push(geyser.clone());
                }
                Ordering::Greater => {
                    free_workers.extend(
                        bot.units
                            .my
                            .workers
                            .filter(|u| {
                                u.target_tag().map_or(false, |target_tag| {
                                    !u.is_carrying_vespene() && target_tag == geyser.tag()
                                })
                            })
                            .iter()
                            .take(
                                (geyser.assigned_harvesters().unwrap()
                                    - geyser.ideal_harvesters().unwrap())
                                    as usize,
                            )
                            .cloned(),
                    );
                }
                _ => {}
            }
        }

        if free_workers.is_empty() {
            return;
        }

        for worker in free_workers.iter() {
            match bot_info.gather_distribution {
                GatherDistribution::GasPriority => {
                    if let Some(closest) = deficit_geysers.closest(worker) {
                        let tag = closest.tag();
                        worker.gather(tag, worker.is_carrying_resource());
                        deficit_geysers.remove(tag);
                    } else if let Some(closest) = deficit_minerals.closest(worker) {
                        let tag = closest.tag();
                        worker.gather(tag, worker.is_carrying_resource());
                        deficit_minerals.remove(tag);
                    }
                }
                _ => {
                    if let Some(closest) = deficit_minerals.closest(worker) {
                        let tag = closest.tag();
                        worker.gather(tag, worker.is_carrying_resource());
                        deficit_minerals.remove(tag);
                    } else if let Some(closest) = deficit_geysers.closest(worker) {
                        let tag = closest.tag();
                        worker.gather(tag, worker.is_carrying_resource());
                        deficit_geysers.remove(tag);
                    }
                }
            }
        }
    }

    fn queue_worker(&self, bot: &mut Bot, bot_info: &mut BotInfo) {
        let ideal_miners = bot.units
            .my
            .townhalls
            .iter()
            .map(|e| e.ideal_harvesters().unwrap_or(0))
            .sum::<u32>();
        let ideal_geysers = bot
            .units
            .my
            .gas_buildings
            .iter()
            .map(|e| e.ideal_harvesters().unwrap_or(0))
            .sum::<u32>();

        let wanted_workers = 82.min(ideal_miners + ideal_geysers);
        bot_info.build_queue.push(
            Command::new_unit(UnitTypeId::Drone, wanted_workers as usize),
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

        self.worker_harversting(bot, bot_info);
        self.queue_worker(bot, bot_info);
    }
}
