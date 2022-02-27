use std::collections::{HashMap, HashSet};

use log::debug;
use rust_sc2::bot::Bot;
use rust_sc2::prelude::*;
use rust_sc2::units::Container;

use crate::command_queue::Command;
use crate::params::*;
use crate::utils::*;
use crate::*;

#[derive(Debug, Clone, Copy, PartialEq)]
enum UnitDecision {
    Advance,
    Retreat,
    Undefined,
}

#[derive(Default)]
pub struct ArmyManager {
    defending: bool,
    allowed_tech: HashSet<UnitTypeId>,
    allied_decision: HashMap<u64, UnitDecision>,
}

impl ArmyManager {
    fn army_unit_unlock(&mut self, bot: &Bot, bot_state: &BotState) {
        let workers = bot.counter().all().count(bot.race_values.worker);
        // for unit in bot.units.enemy.all.iter() {
        //     for counter in unit.type_id().countered_by() {
        //         if counter.from_race(bot) == bot.race {
        //             self.allowed_tech.insert(counter);
        //         }
        //     }
        // }
        self.allowed_tech.insert(UnitTypeId::Zergling);

        // Don't tech up if we're investing on producing an army
        if bot_state.spending_focus == SpendingFocus::Army {
            return;
        }
        if workers > UNLOCK_ROACH_WORKERS {
            self.allowed_tech.insert(UnitTypeId::Roach);
            // self.allowed_tech.insert(UnitTypeId::Ravager);
        }
        if workers >= UNLOCK_HYDRA_WORKERS || !bot_state.enemy_cache.units.flying().is_empty() {
            self.allowed_tech.insert(UnitTypeId::Hydralisk);
        }
        if workers >= UNLOCK_LATE_TECH_WORKERS {
            //     self.allowed_tech.insert(UnitTypeId::Mutalisk);
            self.allowed_tech.insert(UnitTypeId::Corruptor);
            //     self.allowed_tech.insert(UnitTypeId::Ultralisk);
        }
    }
}

impl ArmyManager {
    fn micro(&mut self, bot: &mut Bot, bot_state: &BotState) {
        let mut my_army = Units::new();
        my_army.extend(bot.units.my.units.ready().of_type(UnitTypeId::Zergling));
        my_army.extend(bot.units.my.units.ready().of_type(UnitTypeId::Baneling));
        my_army.extend(bot.units.my.units.ready().of_type(UnitTypeId::Roach));
        my_army.extend(
            bot.units
                .my
                .units
                .ready()
                .of_type(UnitTypeId::RoachBurrowed),
        );
        my_army.extend(bot.units.my.units.ready().of_type(UnitTypeId::Ravager));
        my_army.extend(bot.units.my.units.ready().of_type(UnitTypeId::Hydralisk));
        my_army.extend(bot.units.my.units.ready().of_type(UnitTypeId::Corruptor));
        my_army.extend(bot.units.my.units.ready().of_type(UnitTypeId::Mutalisk));
        my_army.extend(bot.units.my.units.ready().of_type(UnitTypeId::Ultralisk));
        my_army.extend(bot.units.my.units.ready().of_type(UnitTypeId::BroodLord));
        my_army.sort(|u| u.tag());

        // Defend our townhalls
        let defense_range = if self.defending { 20f32 } else { 10f32 };
        if self.defending {
            my_army.extend(
                bot.units
                    .my
                    .units
                    .ready()
                    .of_type(UnitTypeId::Queen)
                    .filter(|u| {
                        !u.is_using(AbilityId::EffectInjectLarva)
                            || !bot
                                .units
                                .enemy
                                .all
                                .filter(|e| u.in_real_range(e, 1f32))
                                .is_empty()
                            || !bot
                                .units
                                .enemy
                                .all
                                .filter(|e| e.in_real_range(u, 1f32))
                                .is_empty()
                    }),
            );
        }
        if my_army.is_empty() {
            return;
        }

        let enemy_attack_force = bot_state.enemy_cache.units.filter(|e| {
            e.can_attack()
                && e.can_be_attacked()
                && bot
                    .units
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
        priority_targets.extend(bot_state.enemy_cache.units.filter(|u| {
            !u.is_flying()
                && !u.is_hallucination()
                && (u.can_attack()
                    || (u.type_id() == UnitTypeId::WidowMine
                        || u.type_id() == UnitTypeId::Infestor
                        || u.type_id() == UnitTypeId::Disruptor
                        || u.type_id() == UnitTypeId::Medivac))
        }));

        secondary_targets.extend(
            bot.units
                .enemy
                .all
                .ground()
                .filter(|u| !u.is_flying() && !priority_targets.contains_tag(u.tag())),
        );

        if !my_army.filter(|u| u.can_attack_air()).is_empty() {
            priority_targets.extend(
                bot.units
                    .enemy
                    .all
                    .flying()
                    .filter(|u| u.is_flying() && u.can_attack()),
            );

            secondary_targets.extend(
                bot.units
                    .enemy
                    .all
                    .flying()
                    .filter(|u| u.is_flying() && !priority_targets.contains_tag(u.tag())),
            );
        }

        let mut our_strength_per_unit = HashMap::new();
        let mut their_strength_per_enemy_unit = HashMap::new();

        for unit in priority_targets.iter() {
            let their_strength = priority_targets
                .filter(|e| {
                    e.in_real_range(unit, unit.speed() + e.speed() + unit.real_ground_range())
                })
                .strength(bot);
            their_strength_per_enemy_unit.insert(unit.tag(), their_strength);
            let our_strength = bot
                .units
                .my
                .all
                .filter(|e| {
                    e.can_attack_unit(unit)
                        && e.in_real_range(
                            unit,
                            unit.speed() + e.speed() + unit.real_ground_range(),
                        )
                })
                .strength(bot);
            our_strength_per_unit.insert(unit.tag(), our_strength);
        }

        for unit in my_army.iter() {
            let our_local_strength = bot
                .units
                .my
                .all
                .filter(|e| e.in_real_range(unit, unit.speed() + e.speed() + 1f32))
                .strength(bot);

            let their_strength = priority_targets
                .filter(|e| {
                    e.can_attack_unit(unit)
                        && e.in_real_range(unit, e.real_speed() + unit.real_speed())
                })
                .max_value(|f| *their_strength_per_enemy_unit.get(&f.tag()).unwrap())
                .unwrap_or_default();

            let our_surrounding_strength = priority_targets
                .filter(|u| {
                    unit.can_attack_unit(u)
                        && unit.in_real_range(u, u.real_speed() + unit.real_speed() + 1f32)
                })
                .max_value(|u| *our_strength_per_unit.get(&u.tag()).unwrap())
                .unwrap_or_default();

            let our_strength = our_local_strength.max(our_surrounding_strength);
            our_strength_per_unit.insert(unit.tag(), our_strength);

            let run_range = 3f32 + unit.real_speed();
            let their_banelings = bot
                .units
                .enemy
                .all
                .of_type(UnitTypeId::Baneling)
                .filter(|u| u.is_closer(run_range, unit));
            let their_broodlings = bot
                .units
                .enemy
                .all
                .of_type(UnitTypeId::Broodling)
                .filter(|u| u.is_closer(run_range, unit));
            let projectile = bot
                .units
                .all
                .of_type(UnitTypeId::RavagerCorrosiveBileMissile)
                .filter(|u| u.is_closer(run_range, unit));

            let run_from_units = (unit.is_melee()
                && (!their_banelings.is_empty() || !their_broodlings.is_empty()))
                || !projectile.is_empty();

            debug!(
                "Unit[{:?}|{:?}] {:?}[{:?}|{:?}]vs{:?}",
                unit.tag(),
                unit.type_id(),
                our_strength,
                our_local_strength,
                our_surrounding_strength,
                their_strength
            );
            let previous_decision = self.allied_decision.get(&unit.tag());

            let decision = if bot.minerals < 1_000
                && ((our_strength < their_strength * 0.3f32)
                    || (!self.defending && our_strength < their_strength * 0.8f32))
                || run_from_units
                || unit.type_id() == UnitTypeId::Roach
                    && unit.health_percentage().unwrap_or_default() < BURROW_HEALTH_PERCENTAGE
            {
                UnitDecision::Retreat
            } else if (self.defending && our_strength > their_strength * 0.8f32)
                || our_strength > their_strength * 1.6f32
                || bot.minerals > 2_000
                || their_strength <= 0.1f32
            {
                UnitDecision::Advance
            } else if let Some(existing_decision) = previous_decision {
                // Keep previous decision
                *existing_decision
            } else {
                UnitDecision::Undefined
            };

            self.allied_decision.insert(unit.tag(), decision);
        }

        for unit in my_army.iter() {
            let decision = *self.allied_decision.get(&unit.tag()).unwrap();
            let detectors = bot.detection_close_by(unit, BURROW_DETECTION_RANGE);
            if unit.type_id() == UnitTypeId::Roach
                && unit.has_ability(AbilityId::BurrowDownRoach)
                && unit.health_percentage().unwrap_or_default() < BURROW_HEALTH_PERCENTAGE
                && !detectors
            {
                unit.use_ability(AbilityId::BurrowDownRoach, false);
                continue;
            } else if unit.type_id() == UnitTypeId::RoachBurrowed {
                if unit.has_ability(AbilityId::BurrowUpRoach)
                    && (decision == UnitDecision::Advance
                        && unit.health_percentage().unwrap_or_default()
                            >= UNBURROW_HEALTH_PERCENTAGE
                        || detectors
                        || unit.is_revealed())
                {
                    unit.use_ability(AbilityId::BurrowUpRoach, false);
                    continue;
                }
                if !bot.has_upgrade(UpgradeId::TunnelingClaws) {
                    continue;
                }
            }
            let local_allied_strength = *our_strength_per_unit.get(&unit.tag()).unwrap();

            let target_in_range = priority_targets
                .iter()
                .filter(|t| {
                    unit.can_be_attacked()
                        && unit.can_attack_unit(t)
                        && unit.in_real_range(t, 0.1f32)
                })
                .min_by_key(|t| t.hits());

            let threats = priority_targets.iter().filter(|t| {
                t.can_attack_unit(unit) && t.in_real_range(unit, -unit.speed())
                    || t.type_id() == UnitTypeId::Baneling && t.is_closer(unit.speed() + 3f32, unit)
            });

            let closest_attackable = priority_targets
                .iter()
                .filter(|t| {
                    unit.can_be_attacked()
                        && unit.can_attack_unit(t)
                        && t.in_real_range(unit, t.speed() + unit.speed())
                        && (!unit.is_melee()
                            || bot
                                .pathing_distance(unit.position(), t.position())
                                .is_some())
                })
                .closest(unit);

            let closest_weak = priority_targets
                .iter()
                .filter(|t| {
                    unit.can_attack_unit(t)
                        && unit.distance(t.position()) <= 15f32
                        && *their_strength_per_enemy_unit.get(&t.tag()).unwrap() * 2f32
                            < local_allied_strength
                })
                .closest(unit);

            let extended_enemy = priority_targets
                .iter()
                .filter(|t| unit.can_attack_unit(t))
                .furthest(bot.enemy_start);

            let secondary_target = secondary_targets
                .iter()
                .filter(|f| unit.can_attack_unit(f))
                .furthest(bot.enemy_start);

            if let Some(target) = target_in_range {
                if decision == UnitDecision::Retreat && unit.on_cooldown() {
                    Self::move_towards(bot, unit, -2.0f32);
                } else if unit.range_vs(target) > target.range_vs(unit)
                    && unit.weapon_cooldown().unwrap_or_default() > 5f32
                {
                    Self::move_towards(bot, unit, -0.6f32);
                } else if decision == UnitDecision::Advance
                    && unit.range_vs(target) <= target.range_vs(unit)
                    && unit.weapon_cooldown().unwrap_or_default() > 5f32
                {
                    Self::move_towards(bot, unit, 0.5f32);
                } else {
                    unit.order_attack(Target::Tag(target.tag()), false);
                }
            } else if decision == UnitDecision::Advance {
                if let Some(target) = closest_attackable {
                    unit.order_attack(Target::Pos(target.position()), false);
                } else if let Some(target) = closest_weak {
                    unit.order_attack(Target::Pos(target.position()), false);
                } else if let Some(target) = extended_enemy {
                    unit.order_attack(Target::Pos(target.position()), false);
                } else if let Some(target) = secondary_target {
                    unit.order_attack(Target::Pos(target.position()), false);
                } else {
                    unit.order_attack(Target::Pos(bot.enemy_start), false);
                }
            } else if decision == UnitDecision::Retreat || decision == UnitDecision::Undefined {
                if threats.count() > 0 {
                    Self::move_towards(bot, unit, -2f32);
                } else if let Some(allied) = bot.units.my.townhalls.closest(bot.start_location) {
                    unit.order_move_to(
                        Target::Pos(allied.position().towards(bot.start_center, 7f32)),
                        2f32,
                        false,
                    );
                } else {
                    unit.order_move_to(Target::Pos(bot.start_location), 7f32, false);
                }
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
            .filter(|t| t.can_attack_unit(unit) && unit.distance(t.position()) < 16f32)
            .center()
        {
            let position = {
                let pos = unit
                    .position()
                    .towards(threat_center, unit.speed() * multiplier);
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

    fn queue_units(&mut self, bot: &mut Bot, bot_state: &mut BotState) {
        let extra_queens = match bot_state.spending_focus {
            SpendingFocus::Economy => 1,
            SpendingFocus::Balance => 3,
            SpendingFocus::Army => 6,
        };
        let min_queens = 8.min(bot.units.my.townhalls.len() + extra_queens);
        bot_state.build_queue.push(
            Command::new_unit(UnitTypeId::Queen, min_queens, false),
            false,
            90,
        );

        let drones = bot.counter().all().count(UnitTypeId::Drone);
        let wanted_army_supply = if drones < MAX_WORKERS {
            if bot_state.spending_focus == SpendingFocus::Army {
                debug!("Army focus!");
                (drones * 6 / 5) as isize
            } else if bot_state.spending_focus == SpendingFocus::Balance {
                (drones * 3 / 5) as isize
            } else {
                (drones / 5) as isize
            }
        } else {
            (bot.supply_army + bot.supply_left) as isize - (min_queens as isize * 2)
        };
        debug!("Wanted army supply {:?}", wanted_army_supply);

        if wanted_army_supply <= 0 {
            return;
        }

        let unit_distribution = self.army_distribution(bot, bot_state, wanted_army_supply);

        let total_weight = unit_distribution
            .values()
            .filter(|it| it.0 > 0)
            .map(|it| it.0)
            .sum::<usize>();
        if total_weight > 0 {
            for (unit_type, (amount, priority)) in unit_distribution {
                debug!("U[{:?}] A[{:?}]", unit_type, amount);
                bot_state.build_queue.push(
                    Command::new_unit(unit_type, amount, true),
                    false,
                    priority,
                );
            }
        }

        if drones > UNLOCK_OVERSEER_WORKERS {
            bot_state.build_queue.push(
                Command::new_unit(UnitTypeId::Overseer, drones / 30, true),
                false,
                100,
            );
        }
    }

    fn army_distribution(
        &self,
        bot: &Bot,
        bot_state: &mut BotState,
        wanted_army_supply: isize,
    ) -> HashMap<UnitTypeId, (usize, usize)> {
        let mut unit_distribution = HashMap::new();

        for unit_type in self.allowed_tech.iter() {
            if unit_type.has_requirement(bot) {
                unit_distribution.insert(*unit_type, Self::unit_value(bot, *unit_type));
            } else {
                bot_state
                    .build_queue
                    .push(Command::new_unit(*unit_type, 1, true), false, 100);
            }
        }
        let mut result = HashMap::new();

        let total_weight = unit_distribution
            .values()
            .filter(|u| u.0 > 0)
            .map(|u| u.0)
            .sum::<isize>();
        if total_weight > 0 {
            for (unit_type, (weight, priority)) in unit_distribution {
                if weight <= 0 {
                    continue;
                }
                let supply_cost = bot.game_data.units[&unit_type].food_required;
                let dedicated_supply = (wanted_army_supply * weight / total_weight) as f32;
                let existing_amount = bot.units.my.units.of_type(unit_type).len() as isize;
                let existing_supply = (existing_amount as f32 * supply_cost) as isize;
                let amount = (dedicated_supply / supply_cost).round() as usize;
                result.insert(unit_type, (amount, priority));
                debug!(
                    "Unit {:?}>{:?}|{:?}[{:?}]",
                    unit_type, existing_supply, dedicated_supply, amount
                );
            }
        }
        result
    }

    fn unit_value(bot_state: &BotState, unit_type: UnitTypeId) -> (isize, usize) {
        let mut value = match unit_type {
            UnitTypeId::Zergling => 10f32,
            UnitTypeId::Roach => 50f32,
            UnitTypeId::Ravager => 50f32,
            UnitTypeId::Hydralisk => 50f32,
            UnitTypeId::Corruptor => 5f32,
            UnitTypeId::Mutalisk => 5f32,
            UnitTypeId::Ultralisk => 5f32,
            _ => 50f32,
        };
        let mut priority = 35;
        for unit in bot_state.enemy_cache.units.iter() {
            if unit.type_id().countered_by().contains(&unit_type) {
                value += unit.supply_cost();
                priority += 1;
            }
            if unit_type.countered_by().contains(&unit.type_id()) {
                value -= unit.supply_cost();
            }
        }
        (value as isize, priority)
    }

    fn queue_upgrades(&self, bot: &mut Bot, bot_state: &mut BotState) {
        if bot.counter().all().count(UnitTypeId::Zergling) > 0
            && bot.can_afford_vespene_upgrade(UpgradeId::Zerglingmovementspeed)
        {
            bot_state.build_queue.push(
                Command::new_upgrade(UpgradeId::Zerglingmovementspeed, true),
                false,
                150,
            );
        }
        if bot_state.spending_focus == SpendingFocus::Army {
            return;
        }
        if bot.counter().all().count(UnitTypeId::Zergling) > 20 {
            bot_state.build_queue.push(
                Command::new_upgrade(UpgradeId::Zerglingattackspeed, true),
                false,
                50,
            );
        }
        if bot.counter().all().count(bot.race_values.worker) > UNLOCK_BURROW_WORKERS {
            bot_state
                .build_queue
                .push(Command::new_upgrade(UpgradeId::Burrow, true), false, 150);
        }
        if bot.counter().all().count(bot.race_values.worker) > OVERLORD_SPEED_WORKERS {
            bot_state.build_queue.push(
                Command::new_upgrade(UpgradeId::Overlordspeed, true),
                false,
                80,
            );
        }
        if bot.counter().all().count(UnitTypeId::Baneling) > 0 {
            bot_state.build_queue.push(
                Command::new_upgrade(UpgradeId::CentrificalHooks, false),
                false,
                50,
            );
        }
        if bot.counter().all().count(UnitTypeId::Roach) > 0 {
            bot_state.build_queue.push(
                Command::new_upgrade(UpgradeId::GlialReconstitution, true),
                false,
                100,
            );
        }
        if bot.counter().all().count(UnitTypeId::Hydralisk) > 0 {
            bot_state.build_queue.push(
                Command::new_upgrade(UpgradeId::EvolveGroovedSpines, false),
                false,
                70,
            );
            bot_state.build_queue.push(
                Command::new_upgrade(UpgradeId::EvolveMuscularAugments, true),
                false,
                80,
            );
        }
        if bot.counter().all().count(bot.race_values.worker) > DOUBLE_EVOLUTION_WORKERS {
            bot_state.build_queue.push(
                Command::new_unit(UnitTypeId::EvolutionChamber, 2, false),
                false,
                50,
            );
        }
        let melee_number = bot.units.my.units.filter(|u| u.is_melee() && !u.is_worker()).len();
        if melee_number > 0 && bot.can_afford_vespene_upgrade(UpgradeId::ZergMeleeWeaponsLevel1)
        {
            bot_state.build_queue.push(
                Command::new_upgrade(UpgradeId::ZergMeleeWeaponsLevel1, melee_number > SAVE_FOR_ATTACK_UPGRADES_ON_UNITS),
                false,
                70,
            );
        }
        if bot.has_upgrade(UpgradeId::ZergMeleeWeaponsLevel1) {
            bot_state.build_queue.push(
                Command::new_upgrade(UpgradeId::ZergMeleeWeaponsLevel2, melee_number > SAVE_FOR_ATTACK_UPGRADES_ON_UNITS),
                false,
                60,
            );
        }
        if bot.has_upgrade(UpgradeId::ZergMeleeWeaponsLevel2) {
            bot_state.build_queue.push(
                Command::new_upgrade(UpgradeId::ZergMeleeWeaponsLevel3, melee_number > SAVE_FOR_ATTACK_UPGRADES_ON_UNITS),
                false,
                60,
            );
        }
        let ground_number = bot.units.my.units.filter(|u| !u.is_flying() && !u.is_worker()).len();
        if ground_number > 0 && bot.can_afford_vespene_upgrade(UpgradeId::ZergGroundArmorsLevel1) {
            bot_state.build_queue.push(
                Command::new_upgrade(UpgradeId::ZergGroundArmorsLevel1, ground_number > SAVE_FOR_DEFENSE_UPGRADES_ON_UNITS),
                false,
                80,
            );
        }
        if bot.has_upgrade(UpgradeId::ZergGroundArmorsLevel1) {
            bot_state.build_queue.push(
                Command::new_upgrade(UpgradeId::ZergGroundArmorsLevel2, ground_number > SAVE_FOR_DEFENSE_UPGRADES_ON_UNITS),
                false,
                70,
            );
        }
        if bot.has_upgrade(UpgradeId::ZergGroundArmorsLevel2) {
            bot_state.build_queue.push(
                Command::new_upgrade(UpgradeId::ZergGroundArmorsLevel3, ground_number > SAVE_FOR_DEFENSE_UPGRADES_ON_UNITS),
                false,
                70,
            );
        }
        if bot.counter().all().count(UnitTypeId::Roach) > 0
            && bot.can_afford_vespene_upgrade(UpgradeId::ZergGroundArmorsLevel1)
        {
            bot_state.build_queue.push(
                Command::new_upgrade(UpgradeId::ZergMissileWeaponsLevel1, false),
                false,
                90,
            );
            bot_state.build_queue.push(
                Command::new_upgrade(UpgradeId::ZergGroundArmorsLevel1, false),
                false,
                80,
            );
        }
        let ranged_number = bot.units.my.units.filter(|u| !u.is_melee() && !u.is_worker()).len();
        if ranged_number > 0 && bot.can_afford_vespene_upgrade(UpgradeId::ZergMissileWeaponsLevel1)
        {
            bot_state.build_queue.push(
                Command::new_upgrade(UpgradeId::ZergMissileWeaponsLevel1, melee_number > SAVE_FOR_ATTACK_UPGRADES_ON_UNITS),
                false,
                70,
            );
        }
        if bot.has_upgrade(UpgradeId::ZergMissileWeaponsLevel1) {
            bot_state.build_queue.push(
                Command::new_upgrade(UpgradeId::ZergMissileWeaponsLevel2, melee_number > SAVE_FOR_ATTACK_UPGRADES_ON_UNITS),
                false,
                60,
            );
        }
        if bot.has_upgrade(UpgradeId::ZergMissileWeaponsLevel2) {
            bot_state.build_queue.push(
                Command::new_upgrade(UpgradeId::ZergMissileWeaponsLevel3, melee_number > SAVE_FOR_ATTACK_UPGRADES_ON_UNITS),
                false,
                60,
            );
        }
        if bot.counter().all().count(UnitTypeId::Ultralisk) > 0 {
            bot_state.build_queue.push(
                Command::new_upgrade(UpgradeId::ChitinousPlating, false),
                false,
                50,
            );
            bot_state.build_queue.push(
                Command::new_upgrade(UpgradeId::AnabolicSynthesis, true),
                false,
                55,
            );
        }
    }
}

impl AIComponent for ArmyManager {
    fn process(&mut self, bot: &mut Bot, bot_state: &mut BotState) {
        self.army_unit_unlock(bot, bot_state);
        self.queue_upgrades(bot, bot_state);
        self.queue_units(bot, bot_state);
        self.micro(bot, bot_state);
    }
}
