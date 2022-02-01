use std::collections::{HashMap, HashSet};

use log::debug;
use rand::prelude::*;
use rust_sc2::bot::Bot;
use rust_sc2::prelude::*;
use rust_sc2::units::Container;
use rust_sc2::Event::UnitDestroyed;

use crate::command_queue::Command;
use crate::managers::production_manager::BuildingRequirement;
use crate::{BotInfo, EventListener, Manager};

#[derive(Debug, Clone, Copy, PartialEq)]
enum UnitDecision {
    Advance,
    Retreat,
    Tank,
    Finish,
    Group,
}

pub struct UnitCache {
    unit: Unit,
    last_seen: f32,
}

impl UnitCache {
    fn new(unit: Unit, time: f32) -> Self {
        Self {
            unit,
            last_seen: time,
        }
    }
}

#[derive(Default)]
pub struct ArmyManager {
    last_loop: u32,
    defending: bool,
    allowed_tech: HashSet<UnitTypeId>,
    enemy_units: HashMap<u64, UnitCache>,
    allied_decision: HashMap<u64, UnitDecision>,
}

impl ArmyManager {
    fn tech_decision(&mut self, bot: &Bot) {
        let drones = bot.counter().all().count(UnitTypeId::Drone);
        if !bot.units.enemy.all.is_empty() || drones >= 19 {
            self.allowed_tech.insert(UnitTypeId::Zergling);
        }
        // for unit in bot.units.enemy.all.iter() {
        //     for counter in unit.type_id().countered_by() {
        //         if counter.from_race(bot) == bot.race {
        //             self.allowed_tech.insert(counter);
        //         }
        //     }
        // }
        if drones >= 25 {
            self.allowed_tech.insert(UnitTypeId::Roach);
        }
        // if drones >= 38 {
        //     self.allowed_tech.insert(UnitTypeId::Ravager);
        // }
        if drones >= 50 || !bot.units.enemy.all.flying().is_empty() {
            self.allowed_tech.insert(UnitTypeId::Hydralisk);
        }
        if drones >= 66 {
            //     self.allowed_tech.insert(UnitTypeId::Mutalisk);
            self.allowed_tech.insert(UnitTypeId::Corruptor);
            //     self.allowed_tech.insert(UnitTypeId::Ultralisk);
        }
    }
}

impl ArmyManager {
    const FOG_AREA_CACHE_TIME: f32 = 120f32;
    const VISIBLE_AREA_CACHE_TIME: f32 = 10f32;

    pub fn destroy_unit(&mut self, tag: u64) {
        if self.allied_decision.contains_key(&tag) {
            debug!("Unit [{tag:?}] destroyed")
        }
        self.allied_decision.remove(&tag);
        self.enemy_units.remove(&tag);
    }

    fn check_unit_cache(&mut self, bot: &Bot) {
        for unit in bot.units.enemy.all.iter() {
            self.enemy_units
                .insert(unit.tag(), UnitCache::new(unit.clone(), bot.time));
        }
        self.enemy_units.retain(|_, value| {
            if bot.is_visible(value.unit.position()) {
                value.last_seen + Self::VISIBLE_AREA_CACHE_TIME > bot.time
            } else {
                value.last_seen + Self::FOG_AREA_CACHE_TIME > bot.time
            }
        });
    }

    fn scout(&self, bot: &mut Bot) {
        let overlords = bot
            .units
            .my
            .units
            .of_type(UnitTypeId::Overlord)
            .sorted(|u| u.tag());
        let ramps = bot
            .ramps
            .enemy
            .points
            .iter()
            .map(|p| Point2::new(p.0 as f32, p.1 as f32).towards(bot.start_center, 7f32))
            .collect::<Vec<Point2>>();
        let mut actual_ramps = vec![];
        for ramp in ramps {
            if let Some(distance) = actual_ramps.iter().closest_distance(ramp) {
                if distance > 8f32 {
                    actual_ramps.push(ramp);
                }
            } else {
                actual_ramps.push(ramp);
            }
        }

        for overlord in overlords.iter() {
            let closest_ramp = actual_ramps.iter().closest(overlord).cloned();
            if let Some(ramp) = closest_ramp {
                actual_ramps.retain(|p| *p != ramp);
            }
            if let Some(closest_anti_air) = bot
                .units
                .enemy
                .all
                .filter(|f| {
                    f.can_attack_air() && f.in_real_range(overlord, f.speed() + overlord.speed())
                })
                .iter()
                .closest(overlord)
            {
                let position = overlord.position().towards(
                    closest_anti_air.position(),
                    -closest_anti_air.real_range_vs(overlord),
                );
                overlord.order_move_to(Target::Pos(position), 0.5f32, false);
            } else if overlord.hits_percentage().unwrap_or_default() > 0.9f32 && overlord.is_idle()
            {
                if let Some(ramp) = closest_ramp {
                    overlord.order_move_to(Target::Pos(ramp), 0.5f32, false);
                } else {
                    let mut rng = thread_rng();
                    let random_x = (rng.next_u64() % bot.game_info.map_size.x as u64) as f32;
                    let random_y = (rng.next_u64() % bot.game_info.map_size.y as u64) as f32;
                    let position = Point2::new(random_x, random_y);
                    overlord.order_move_to(Target::Pos(position), 0.5f32, false);
                }
            }
        }
    }

    fn micro(&mut self, bot: &mut Bot) {
        let mut my_army = Units::new();
        my_army.extend(bot.units.my.units.ready().of_type(UnitTypeId::Zergling));
        my_army.extend(bot.units.my.units.ready().of_type(UnitTypeId::Baneling));
        my_army.extend(bot.units.my.units.ready().of_type(UnitTypeId::Roach));
        my_army.extend(bot.units.my.units.ready().of_type(UnitTypeId::Ravager));
        my_army.extend(bot.units.my.units.ready().of_type(UnitTypeId::Hydralisk));
        my_army.extend(bot.units.my.units.ready().of_type(UnitTypeId::Corruptor));
        my_army.extend(bot.units.my.units.ready().of_type(UnitTypeId::Mutalisk));
        my_army.extend(bot.units.my.units.ready().of_type(UnitTypeId::Ultralisk));

        // Attack when speed upgrade is > 80% ready
        // Defend our locations
        let base_count = bot.owned_expansions().count();
        let extra_range = 2f32 * base_count as f32;
        let defense_range = if self.defending {
            20f32 + extra_range
        } else {
            10f32 + extra_range
        };
        if self.defending {
            my_army.extend(
                bot.units
                    .my
                    .units
                    .ready()
                    .of_type(UnitTypeId::Queen)
                    .filter(|u| !u.is_using(AbilityId::EffectInjectLarva)),
            );
        }
        if my_army.is_empty() {
            return;
        }

        let enemy_attack_force = bot.units.enemy.all.visible().filter(|e| {
            bot.units
                .my
                .townhalls
                .iter()
                .any(|h| h.is_closer(defense_range, *e))
        });

        self.defending = !enemy_attack_force.is_empty();

        let mut priority_targets = Units::new();
        let mut secondary_targets = Units::new();

        // Retreat when aggression is small
        // Attack when we build enough numbers again
        priority_targets.extend(
            self.enemy_units
                .values()
                .filter(|u| {
                    !u.unit.is_flying()
                        && u.unit.can_attack()
                        && u.unit.can_be_attacked()
                        && u.unit.type_id() != UnitTypeId::Larva
                })
                .map(|u| u.unit.clone())
                .collect::<Vec<Unit>>(),
        );

        secondary_targets.extend(bot.units.enemy.all.ground().filter(|u| {
            !u.is_flying()
                && !u.can_attack()
                && u.can_be_attacked()
                && u.type_id() != UnitTypeId::Larva
        }));

        if !my_army.filter(|u| u.can_attack_air()).is_empty() {
            priority_targets.extend(
                bot.units
                    .enemy
                    .all
                    .flying()
                    .filter(|u| u.is_flying() && u.can_attack() && u.can_be_attacked()),
            );

            secondary_targets.extend(
                bot.units
                    .enemy
                    .all
                    .flying()
                    .filter(|u| u.is_flying() && !u.can_attack() && u.can_be_attacked()),
            );
        }

        Self::overseer_micro(bot);
        let mut close_allied_strength = HashMap::new();
        let mut close_allied_count = HashMap::new();
        let mut enemy_strength_per_enemy_unit = HashMap::new();
        let mut enemy_count_per_enemy_unit = HashMap::new();
        let mut enemy_units_being_targeted_count = HashMap::new();
        for unit in my_army.iter() {
            if let Some(tag) = unit.target_tag() {
                let count = enemy_units_being_targeted_count.entry(tag).or_insert(0);
                *count += 1;
            }
        }

        for unit in priority_targets.iter() {
            let their_strength = priority_targets
                .filter(|e| e.position().distance(unit) < 11f32)
                .strength(bot);
            let their_count = priority_targets
                .filter(|e| e.position().distance(unit) < 11f32 && !e.is_worker())
                .len();
            enemy_strength_per_enemy_unit.insert(unit.tag(), their_strength);
            enemy_count_per_enemy_unit.insert(unit.tag(), their_count);
        }
        debug!("Unit analysis");
        for unit in my_army.iter() {
            let friendly_units = bot
                .units
                .my
                .all
                .filter(|e| e.position().distance(unit) < 7f32);
            let our_strength = friendly_units.strength(bot);
            let our_count = friendly_units.len();
            let friendly_ranged = !friendly_units
                .filter(|f| f.can_attack() && !f.is_melee() && f.is_attacked())
                .is_empty();

            let their_strength = priority_targets
                .filter(|e| {
                    e.can_attack_unit(unit)
                        && e.position().distance(unit)
                            < e.range_vs(unit) + e.real_speed() + unit.real_speed()
                })
                .max_value(|f| *enemy_strength_per_enemy_unit.get(&f.tag()).unwrap())
                .unwrap_or_default();

            let their_count = priority_targets
                .filter(|e| {
                    e.can_attack_unit(unit)
                        && e.position().distance(unit)
                            < e.range_vs(unit) + e.real_speed() + unit.real_speed()
                })
                .max_value(|f| *enemy_count_per_enemy_unit.get(&f.tag()).unwrap())
                .unwrap_or_default();

            close_allied_strength.insert(unit.tag(), our_strength);
            close_allied_count.insert(unit.tag(), our_count);
            let surrounding = if let Some(tag) = unit.target_tag() {
                *enemy_units_being_targeted_count.get(&tag).unwrap() > 3
            } else {
                false
            };

            debug!(
                "Unit[{:?}|{:?}] {:?}[{:?}]vs{:?}[{:?}]",
                unit.tag(),
                unit.type_id(),
                our_strength,
                our_count,
                their_strength,
                their_count
            );
            let decision = if our_strength > their_strength * 1.5f32 {
                UnitDecision::Advance
            } else if surrounding && unit.is_melee() {
                UnitDecision::Finish
            } else if our_strength < their_strength * 0.8
                && unit.type_id() == UnitTypeId::Zergling
                && friendly_ranged
            {
                UnitDecision::Tank
            } else if our_strength < their_strength * 0.8 {
                UnitDecision::Retreat
            } else {
                UnitDecision::Group
            };

            self.allied_decision.insert(unit.tag(), decision);
        }

        debug!("Decision report");
        for unit in my_army.iter() {
            debug!(
                "Unit[{:?}|{:?}] {:?}",
                unit.tag(),
                unit.type_id(),
                self.allied_decision.get(&unit.tag())
            );
        }

        for unit in my_army.iter() {
            let local_allied_strength = *close_allied_strength.get(&unit.tag()).unwrap();
            let decision = *self.allied_decision.get(&unit.tag()).unwrap();

            let target_in_range = priority_targets
                .iter()
                .filter(|t| unit.can_attack_unit(t) && unit.in_real_range(t, 0.1f32))
                .min_by_key(|t| t.hits());

            let closest_threat = priority_targets
                .iter()
                .filter(|t| {
                    t.can_attack_unit(unit) && t.in_real_range(unit, t.speed() + unit.speed())
                })
                .closest(unit);

            let closest_weak = priority_targets
                .iter()
                .filter(|t| {
                    unit.can_attack_unit(t)
                        && unit.distance(t.position()) <= 15f32
                        && *enemy_strength_per_enemy_unit.get(&t.tag()).unwrap() * 2f32
                            < local_allied_strength
                })
                .closest(unit);

            let globally_weak = priority_targets
                .iter()
                .filter(|t| {
                    unit.can_attack_unit(t)
                        && *enemy_strength_per_enemy_unit.get(&t.tag()).unwrap() * 2f32
                            < local_allied_strength
                })
                .furthest(bot.enemy_start);

            let secondary_target = secondary_targets
                .iter()
                .filter(|f| unit.can_attack_unit(f))
                .furthest(bot.enemy_start);

            if let Some(target) = target_in_range {
                if decision == UnitDecision::Retreat && (unit.on_cooldown() || unit.is_melee()) {
                    Self::move_towards(bot, unit, -unit.speed());
                } else if unit.range_vs(target) > target.range_vs(unit)
                    && unit.weapon_cooldown().unwrap_or_default() > 5f32
                {
                    Self::move_towards(bot, unit, -0.5f32);
                } else if decision == UnitDecision::Advance
                    && unit.range_vs(target) < target.range_vs(unit)
                    && unit.weapon_cooldown().unwrap_or_default() > 5f32
                {
                    Self::move_towards(bot, unit, 0.5f32);
                } else {
                    unit.order_attack(Target::Tag(target.tag()), false);
                }
            } else if decision == UnitDecision::Advance || decision == UnitDecision::Tank {
                if let Some(target) = closest_threat {
                    unit.order_attack(Target::Tag(target.tag()), false);
                } else if let Some(target) = closest_weak {
                    unit.order_attack(Target::Pos(target.position()), false);
                } else if let Some(target) = globally_weak {
                    unit.order_attack(Target::Pos(target.position()), false);
                } else if let Some(target) = secondary_target {
                    unit.order_attack(Target::Pos(target.position()), false);
                }
            } else if decision == UnitDecision::Retreat {
                if let Some(allied) = my_army
                    .iter()
                    .filter(|u| *self.allied_decision.get(&u.tag()).unwrap() == UnitDecision::Group)
                    .closest(unit)
                {
                    unit.order_move_to(Target::Pos(allied.position()), 2f32, false);
                } else {
                    unit.order_move_to(Target::Pos(bot.start_location), 7f32, false);
                }
            } else if decision == UnitDecision::Group {
                if let Some(target) = closest_threat {
                    Self::move_towards(bot, target, -2f32);
                } else if let Some(allied) = my_army
                    .iter()
                    .filter(|u| {
                        *self.allied_decision.get(&u.tag()).unwrap() == UnitDecision::Advance
                    })
                    .closest(unit)
                {
                    unit.order_move_to(Target::Pos(allied.position()), 2f32, false);
                } else {
                    unit.order_move_to(Target::Pos(bot.start_location), 7f32, false);
                }
            } else {
                unit.order_move_to(Target::Pos(bot.start_location), 7f32, false);
            }

            // Bring back my queens
            if !self.defending {
                for queen in bot
                    .units
                    .my
                    .units
                    .iter()
                    .ready()
                    .of_type(UnitTypeId::Queen)
                    .filter(|u| u.is_attacking())
                {
                    if let Some(closest_hall) = bot.units.my.townhalls.closest(queen) {
                        queen.order_move_to(Target::Pos(closest_hall.position()), 0.5f32, false);
                    }
                }
            }
        }
    }

    fn move_towards(bot: &Bot, unit: &Unit, multiplier: f32) {
        if let Some(threat_center) = bot
            .units
            .enemy
            .all
            .filter(|t| unit.distance(t.position()) < 16f32)
            .center()
        {
            let position = {
                let pos = unit
                    .position()
                    .towards(threat_center, multiplier * unit.speed());
                if bot.is_pathable(pos) {
                    pos
                } else {
                    *unit
                        .position()
                        .neighbors8()
                        .iter()
                        .filter(|p| bot.is_pathable(**p))
                        .furthest(threat_center)
                        .unwrap_or(&bot.start_location)
                }
            };
            unit.order_move_to(Target::Pos(position), 0.5f32, false);
        }
    }

    fn overseer_micro(bot: &Bot) {
        let overseers = bot.units.my.units.of_type(UnitTypeId::Overseer);
        for overseer in overseers.iter() {
            let position = if let Some(closest_anti_air) = bot
                .units
                .enemy
                .all
                .filter(|f| {
                    f.can_attack_air() && f.in_real_range(overseer, f.speed() + overseer.speed())
                })
                .iter()
                .closest(overseer)
            {
                overseer
                    .position()
                    .towards(closest_anti_air.position(), -overseer.speed())
            } else if let Some(closest_invisible) = bot
                .units
                .enemy
                .all
                .filter(|f| f.is_cloaked())
                .closest(bot.start_location)
            {
                closest_invisible.position()
            } else if let Some(closest_enemy) = bot
                .units
                .enemy
                .all
                .filter(|f| {
                    !f.is_worker()
                        && !f.is_structure()
                        && f.type_id() != UnitTypeId::Overseer
                        && f.type_id() != UnitTypeId::Overlord
                })
                .closest(overseer)
            {
                closest_enemy.position()
            } else {
                bot.enemy_start
            };
            overseer.order_move_to(Target::Pos(position), 1.0f32, false);
        }
    }

    fn queue_units(&mut self, bot: &mut Bot, bot_info: &mut BotInfo) {
        let min_queens = 7.min(bot.units.my.townhalls.len() + 2);
        bot_info.build_queue.push(
            Command::new_unit(UnitTypeId::Queen, min_queens, false),
            false,
            35,
        );

        let drones = bot.counter().all().count(UnitTypeId::Drone);
        let their_supply = self
            .enemy_units
            .values()
            .filter(|f| !f.unit.is_worker())
            .map(|f| f.unit.supply_cost())
            .sum::<f32>() as usize;
        let wanted_army_supply = if drones < 76 {
            (drones / 3).max(their_supply + 2) as isize
        } else {
            (bot.supply_army + bot.supply_left) as isize - (min_queens as isize * 2)
        };

        if wanted_army_supply <= 0 {
            return;
        }

        // TODO: Base a difference on enemy units
        // TODO: When facing air enemies make anti-air
        let unit_distribution = self.army_distribution(bot, bot_info);

        let total_weight = unit_distribution
            .values()
            .filter(|u| **u > 0)
            .sum::<isize>();
        if total_weight > 0 {
            let mut used_supply = 0f32;
            for (unit_type, weight) in unit_distribution {
                if weight <= 0 {
                    continue;
                }
                let supply_cost = bot.game_data.units[&unit_type].food_required;
                let existing_supply =
                    (bot.units.my.units.of_type(unit_type).len() as f32 * supply_cost) as isize;
                let dedicated_supply = (wanted_army_supply * weight / total_weight) as f32;
                let amount = (dedicated_supply / supply_cost).round() as usize;

                bot_info
                    .build_queue
                    .push(Command::new_unit(unit_type, amount, true), false, 35);
                used_supply += dedicated_supply;
                debug!(
                    "Unit {:?}>{:?}|{:?}[{:?}]",
                    unit_type, existing_supply, dedicated_supply, amount
                );
            }
            debug!(
                "Final army supply {:?}>{:?}",
                wanted_army_supply, used_supply
            );
        }

        if drones > 34 {
            bot_info
                .build_queue
                .push(Command::new_unit(UnitTypeId::Overseer, 2, false), false, 1);
        }
    }

    fn army_distribution(&self, bot: &Bot, bot_info: &mut BotInfo) -> HashMap<UnitTypeId, isize> {
        let mut unit_distribution = HashMap::new();

        for unit_type in self.allowed_tech.iter() {
            if let Some(requirement) = unit_type.building_requirement() {
                if !bot.units.my.all.ready().of_type(requirement).is_empty() {
                    unit_distribution.insert(*unit_type, Self::unit_value(bot, *unit_type));
                } else if let Some(another_requirement) = requirement.building_requirement() {
                    if !bot
                        .units
                        .my
                        .all
                        .ready()
                        .of_type(another_requirement)
                        .is_empty()
                    {
                        bot_info.build_queue.push(
                            Command::new_unit(requirement, 1, true),
                            false,
                            100,
                        );
                    }
                } else {
                    bot_info
                        .build_queue
                        .push(Command::new_unit(requirement, 1, true), false, 100);
                }
            } else {
                unit_distribution.insert(*unit_type, Self::unit_value(bot, *unit_type));
            }
        }
        unit_distribution
    }

    fn unit_value(bot: &Bot, unit_type: UnitTypeId) -> isize {
        let mut value = match unit_type {
            UnitTypeId::Zergling => 20f32,
            UnitTypeId::Roach => 30f32,
            UnitTypeId::Ravager => 30f32,
            UnitTypeId::Hydralisk => 50f32,
            UnitTypeId::Corruptor => 5f32,
            UnitTypeId::Mutalisk => 5f32,
            UnitTypeId::Ultralisk => 5f32,
            _ => 50f32,
        };
        for unit in bot.units.enemy.all.iter() {
            if unit.type_id().countered_by().contains(&unit_type) {
                value += unit.supply_cost();
            }
            if unit_type.countered_by().contains(&unit.type_id()) {
                value -= unit.supply_cost();
            }
        }
        value as isize
    }

    fn queue_upgrades(&self, bot: &mut Bot, bot_info: &mut BotInfo) {
        if bot.counter().all().count(UnitTypeId::Zergling) > 0
            && bot.vespene
                > bot
                    .game_data
                    .upgrades
                    .get(&UpgradeId::Zerglingmovementspeed)
                    .unwrap()
                    .vespene_cost
        {
            bot_info.build_queue.push(
                Command::new_upgrade(UpgradeId::Zerglingmovementspeed, true),
                false,
                150,
            );
        }
        if bot.counter().all().count(UnitTypeId::Zergling) > 20 {
            bot_info.build_queue.push(
                Command::new_upgrade(UpgradeId::Zerglingattackspeed, false),
                false,
                50,
            );
        }
        if bot.counter().all().count(UnitTypeId::Drone) > 30 {
            bot_info.build_queue.push(
                Command::new_upgrade(UpgradeId::Overlordspeed, true),
                false,
                80,
            );
        }
        if bot.counter().all().count(UnitTypeId::Baneling) > 0 {
            bot_info.build_queue.push(
                Command::new_upgrade(UpgradeId::CentrificalHooks, false),
                false,
                50,
            );
        }
        if bot.counter().all().count(UnitTypeId::Roach) > 0 {
            bot_info.build_queue.push(
                Command::new_upgrade(UpgradeId::GlialReconstitution, true),
                false,
                100,
            );
        }
        if bot.counter().all().count(UnitTypeId::Hydralisk) > 0 {
            bot_info.build_queue.push(
                Command::new_upgrade(UpgradeId::EvolveGroovedSpines, true),
                false,
                70,
            );
            bot_info.build_queue.push(
                Command::new_upgrade(UpgradeId::EvolveMuscularAugments, true),
                false,
                80,
            );
        }
        if bot.counter().all().count(UnitTypeId::Zergling) > 0
            && bot.can_afford_upgrade(UpgradeId::ZergMeleeWeaponsLevel1)
        {
            bot_info.build_queue.push(
                Command::new_upgrade(UpgradeId::ZergMeleeWeaponsLevel1, false),
                false,
                70,
            );
            bot_info.build_queue.push(
                Command::new_upgrade(UpgradeId::ZergGroundArmorsLevel1, true),
                false,
                80,
            );
        }
        if bot.counter().all().count(bot.race_values.worker) > 45 {
            bot_info.build_queue.push(
                Command::new_unit(UnitTypeId::EvolutionChamber, 2, false),
                false,
                50,
            );
        }
        if bot.counter().all().count(UnitTypeId::Zergling) > 0
            && bot.can_afford_upgrade(UpgradeId::ZergMeleeWeaponsLevel2)
        {
            bot_info.build_queue.push(
                Command::new_upgrade(UpgradeId::ZergMeleeWeaponsLevel2, false),
                false,
                60,
            );
            bot_info.build_queue.push(
                Command::new_upgrade(UpgradeId::ZergGroundArmorsLevel2, true),
                false,
                70,
            );
        }
        if bot.counter().all().count(UnitTypeId::Roach) > 0
            && bot.can_afford_upgrade(UpgradeId::ZergMissileWeaponsLevel1)
        {
            bot_info.build_queue.push(
                Command::new_upgrade(UpgradeId::ZergMissileWeaponsLevel1, false),
                false,
                90,
            );
        }
        if bot.counter().all().count(UnitTypeId::Roach) > 0
            && bot.can_afford_upgrade(UpgradeId::ZergMissileWeaponsLevel2)
        {
            bot_info.build_queue.push(
                Command::new_upgrade(UpgradeId::ZergMissileWeaponsLevel2, false),
                false,
                80,
            );
            bot_info.build_queue.push(
                Command::new_upgrade(UpgradeId::ZergGroundArmorsLevel2, true),
                false,
                70,
            );
        }
        if bot.can_afford_upgrade(UpgradeId::ZergGroundArmorsLevel3) {
            bot_info.build_queue.push(
                Command::new_upgrade(UpgradeId::ZergGroundArmorsLevel3, true),
                false,
                60,
            );
            bot_info.build_queue.push(
                Command::new_upgrade(UpgradeId::ZergMeleeWeaponsLevel3, false),
                false,
                50,
            );
            bot_info.build_queue.push(
                Command::new_upgrade(UpgradeId::ZergMissileWeaponsLevel3, false),
                false,
                70,
            );
        }
        if bot.counter().all().count(UnitTypeId::Ultralisk) > 0 {
            bot_info.build_queue.push(
                Command::new_upgrade(UpgradeId::ChitinousPlating, false),
                false,
                50,
            );
            bot_info.build_queue.push(
                Command::new_upgrade(UpgradeId::AnabolicSynthesis, true),
                false,
                55,
            );
        }
    }
}

trait UnitOrderCheck {
    fn order_move_to(&self, target: Target, range: f32, queue: bool);
    fn order_attack(&self, target: Target, queue: bool);
}

impl UnitOrderCheck for Unit {
    fn order_move_to(&self, target: Target, range: f32, queue: bool) {
        if should_send_order(self, target, range, queue) {
            self.move_to(target, queue);
        }
    }

    fn order_attack(&self, target: Target, queue: bool) {
        if should_send_order(self, target, 0.3f32, queue) {
            self.attack(target, queue);
        }
    }
}

fn should_send_order(unit: &Unit, target: Target, range: f32, queue: bool) -> bool {
    if queue {
        true
    } else {
        match (unit.target(), target) {
            (Target::Pos(current_pos), Target::Pos(wanted_pos)) => {
                current_pos.distance(wanted_pos) > range
            }
            (_, Target::Pos(wanted_pos)) => unit.position().distance(wanted_pos) > range,
            (Target::Tag(current_tag), Target::Tag(wanted_tag)) => current_tag != wanted_tag,
            (_, _) => true,
        }
    }
}

impl Strength for Units {
    fn strength(&self, bot: &Bot) -> f32 {
        self.iter()
            .filter(|f| f.can_attack_ground())
            .map(|u| u.strength(bot))
            .sum()
    }
}

impl StrengthVs for Units {
    fn strength_vs(&self, bot: &Bot, unit: &Unit) -> f32 {
        self.iter()
            .filter(|f| f.can_attack_unit(unit))
            .map(|u| u.strength(bot))
            .sum()
    }
}

trait StrengthVs {
    fn strength_vs(&self, bot: &Bot, unit: &Unit) -> f32;
}

trait Strength {
    fn strength(&self, bot: &Bot) -> f32;
}

//TODO: Give bonus for units better at one role.
// e.g. thor anti air
impl Strength for Unit {
    fn strength(&self, _: &Bot) -> f32 {
        let multiplier = if self.is_worker() { 0.1f32 } else { 1f32 };
        multiplier
            * (self.cost().vespene + self.cost().minerals) as f32
            * self.hits_percentage().unwrap_or(1f32)
    }
}

trait CounteredBy {
    fn countered_by(&self) -> Vec<UnitTypeId>;
}

impl CounteredBy for UnitTypeId {
    fn countered_by(&self) -> Vec<UnitTypeId> {
        match self {
            // Race::Protoss
            UnitTypeId::Zealot => vec![
                UnitTypeId::Roach,
                UnitTypeId::Mutalisk,
                // UnitTypeId::BroodLord,
            ],
            UnitTypeId::Sentry => vec![
                // UnitTypeId::BroodLord,
                UnitTypeId::Ultralisk,
            ],
            UnitTypeId::Stalker => vec![UnitTypeId::Zergling],
            UnitTypeId::Immortal => vec![UnitTypeId::Zergling, UnitTypeId::Hydralisk],
            // UnitTypeId::Colossus => vec![UnitTypeId::Corruptor],
            UnitTypeId::Phoenix => vec![UnitTypeId::Hydralisk],
            UnitTypeId::VoidRay => vec![UnitTypeId::Hydralisk],
            UnitTypeId::HighTemplar => vec![UnitTypeId::Ultralisk],
            UnitTypeId::DarkTemplar => vec![
                UnitTypeId::Mutalisk,
                // UnitTypeId::BroodLord
            ],
            UnitTypeId::Carrier => vec![UnitTypeId::Hydralisk, UnitTypeId::Corruptor],
            // UnitTypeId::Mothership => vec![UnitTypeId::Corruptor],
            UnitTypeId::Oracle => vec![UnitTypeId::Hydralisk, UnitTypeId::Mutalisk],
            // UnitTypeId::Tempest => vec![UnitTypeId::Corruptor],
            UnitTypeId::Adept => vec![UnitTypeId::Roach],
            UnitTypeId::Disruptor => vec![UnitTypeId::Ultralisk],
            // Race::Terran
            UnitTypeId::Marine => vec![
                // UnitTypeId::Baneling,
                UnitTypeId::Roach,
                UnitTypeId::Ultralisk,
                // UnitTypeId::BroodLord,
                // UnitTypeId::LurkerMP,
            ],
            UnitTypeId::Marauder => vec![
                UnitTypeId::Hydralisk,
                UnitTypeId::Mutalisk,
                // UnitTypeId::BroodLord,
            ],
            UnitTypeId::Medivac => vec![UnitTypeId::Hydralisk],
            UnitTypeId::Reaper => vec![UnitTypeId::Roach],
            UnitTypeId::Ghost => vec![UnitTypeId::Roach, UnitTypeId::Ultralisk],
            UnitTypeId::Hellion => vec![UnitTypeId::Roach, UnitTypeId::Mutalisk],
            UnitTypeId::SiegeTank => vec![
                UnitTypeId::Mutalisk,
                // UnitTypeId::BroodLord,
                UnitTypeId::Ravager,
            ],
            UnitTypeId::SiegeTankSieged => vec![
                UnitTypeId::Mutalisk,
                // UnitTypeId::BroodLord,
                UnitTypeId::Ravager,
            ],
            UnitTypeId::Thor => vec![
                UnitTypeId::Zergling,
                UnitTypeId::Hydralisk,
                // UnitTypeId::BroodLord,
            ],
            // UnitTypeId::Banshee => vec![UnitTypeId::Mutalisk, UnitTypeId::Corruptor],
            UnitTypeId::Viking => vec![UnitTypeId::Hydralisk],
            UnitTypeId::Raven => vec![UnitTypeId::Corruptor],
            // UnitTypeId::Battlecruiser => vec![UnitTypeId::Corruptor],
            UnitTypeId::HellionTank => vec![UnitTypeId::Roach],
            // UnitTypeId::Liberator => vec![UnitTypeId::Corruptor],
            // Race::Zerg
            UnitTypeId::Zergling => vec![
                UnitTypeId::Zealot,
                UnitTypeId::Sentry,
                UnitTypeId::Colossus,
                UnitTypeId::Reaper,
                UnitTypeId::Hellion,
                UnitTypeId::HellionTank,
                // UnitTypeId::Baneling,
                UnitTypeId::Roach,
                UnitTypeId::Ultralisk,
            ],
            UnitTypeId::Baneling => vec![
                UnitTypeId::Colossus,
                UnitTypeId::SiegeTank,
                UnitTypeId::SiegeTankSieged,
                UnitTypeId::Mutalisk,
                UnitTypeId::Roach,
                UnitTypeId::Ultralisk,
            ],
            UnitTypeId::Roach => vec![
                UnitTypeId::Immortal,
                UnitTypeId::VoidRay,
                UnitTypeId::SiegeTank,
                UnitTypeId::SiegeTankSieged,
                UnitTypeId::Marauder,
                UnitTypeId::Mutalisk,
                // UnitTypeId::BroodLord,
            ],
            UnitTypeId::Hydralisk => vec![
                UnitTypeId::Sentry,
                UnitTypeId::Colossus,
                UnitTypeId::Hellion,
                UnitTypeId::HellionTank,
                UnitTypeId::SiegeTank,
                UnitTypeId::SiegeTank,
                UnitTypeId::SiegeTankSieged,
                UnitTypeId::Roach,
                // UnitTypeId::BroodLord,
            ],
            UnitTypeId::Mutalisk => vec![
                UnitTypeId::Sentry,
                UnitTypeId::Phoenix,
                UnitTypeId::Marine,
                UnitTypeId::Thor,
                UnitTypeId::Hydralisk,
                // UnitTypeId::Corruptor,
            ],
            UnitTypeId::Corruptor => vec![
                UnitTypeId::Stalker,
                UnitTypeId::Sentry,
                UnitTypeId::Marine,
                UnitTypeId::Thor,
                UnitTypeId::Hydralisk,
            ],
            UnitTypeId::Infestor => vec![
                UnitTypeId::Immortal,
                UnitTypeId::Colossus,
                UnitTypeId::SiegeTank,
                UnitTypeId::SiegeTankSieged,
                UnitTypeId::Ghost,
                // UnitTypeId::BroodLord,
            ],
            UnitTypeId::Ultralisk => vec![
                UnitTypeId::Immortal,
                UnitTypeId::VoidRay,
                UnitTypeId::Banshee,
                UnitTypeId::Hydralisk,
                // UnitTypeId::BroodLord,
            ],
            UnitTypeId::BroodLord => vec![
                UnitTypeId::Stalker,
                UnitTypeId::VoidRay,
                UnitTypeId::Phoenix,
                UnitTypeId::Viking,
                // UnitTypeId::Corruptor,
            ],
            UnitTypeId::Viper => vec![
                UnitTypeId::Phoenix,
                UnitTypeId::Viking,
                UnitTypeId::Mutalisk,
                // UnitTypeId::Corruptor,
            ],
            UnitTypeId::Ravager => vec![
                UnitTypeId::Immortal,
                UnitTypeId::Marauder,
                UnitTypeId::Ultralisk,
            ],
            UnitTypeId::LurkerMP => vec![
                UnitTypeId::Disruptor,
                UnitTypeId::SiegeTank,
                UnitTypeId::SiegeTankSieged,
                UnitTypeId::Ultralisk,
            ],
            UnitTypeId::LurkerMPBurrowed => vec![
                UnitTypeId::Disruptor,
                UnitTypeId::SiegeTank,
                UnitTypeId::SiegeTankSieged,
                UnitTypeId::Ultralisk,
            ],
            UnitTypeId::PhotonCannon => vec![UnitTypeId::Ravager],
            UnitTypeId::Bunker => vec![UnitTypeId::Ravager],
            _ => vec![],
        }
    }
}

trait FromRace {
    fn from_race(&self, bot: &Bot) -> Race;
}

impl FromRace for UnitTypeId {
    fn from_race(&self, bot: &Bot) -> Race {
        bot.game_data.units[self].race
    }
}

impl ArmyManager {
    const PROCESS_DELAY: u32 = 5;
}

impl Manager for ArmyManager {
    fn process(&mut self, bot: &mut Bot, bot_info: &mut BotInfo) {
        let last_loop = self.last_loop;
        let game_loop = bot.state.observation.game_loop();
        if last_loop + Self::PROCESS_DELAY > game_loop {
            return;
        }
        self.last_loop = game_loop;
        self.check_unit_cache(bot);
        self.tech_decision(bot);
        self.queue_upgrades(bot, bot_info);
        self.queue_units(bot, bot_info);
        self.scout(bot);
        self.micro(bot);
    }
}

impl EventListener for ArmyManager {
    fn on_event(&mut self, event: &Event) {
        if let UnitDestroyed(tag, _) = event {
            self.destroy_unit(*tag);
        }
    }
}
