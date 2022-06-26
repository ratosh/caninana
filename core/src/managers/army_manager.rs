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
    money_engaging: bool,
    strength_engaging: bool,
    allowed_tech: HashSet<UnitTypeId>,
    allied_decision: HashMap<u64, UnitDecision>,
}

impl ArmyManager {
    fn army_unit_unlock(&mut self, bot: &mut Bot, bot_state: &BotState) {
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
        if workers >= UNLOCK_ROACH_WORKERS
            || bot_state
                .enemy_cache
                .units
                .filter(|u| u.need_roaches())
                .len()
                > 1
        {
            self.unlock_tech(bot, UnitTypeId::Roach);
        }
        if workers >= UNLOCK_HYDRA_WORKERS
            || !bot_state
                .enemy_cache
                .units
                .filter(|u| u.is_flying() && u.can_attack_ground())
                .is_empty()
        {
            self.unlock_tech(bot, UnitTypeId::Hydralisk);
        }
        if workers >= UNLOCK_LATE_TECH_WORKERS
            || !bot_state
                .enemy_cache
                .units
                .filter(|u| u.need_corruptors())
                .is_empty()
        {
            self.unlock_tech(bot, UnitTypeId::Corruptor);
        }
        if workers >= UNLOCK_REALLY_LATE_TECH_WORKERS {
            self.unlock_tech(bot, UnitTypeId::BroodLord);
        }
    }

    fn unlock_tech(&mut self, bot: &mut Bot, unit_type: UnitTypeId) {
        if !self.allowed_tech.contains(&unit_type) {
            bot.chat_ally(format!("Unlocking {:?}", unit_type).as_str())
        }
        self.allowed_tech.insert(unit_type);
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
        my_army.extend(
            bot.units
                .my
                .units
                .ready()
                .of_type(UnitTypeId::Queen)
                .filter(|u| {
                    !u.is_using(AbilityId::TransfusionTransfusion)
                        && !bot_state
                            .enemy_cache
                            .units
                            .filter(|e| u.in_real_range(e, 0f32))
                            .is_empty()
                }),
        );

        // Defend our townhalls
        let defense_range = if self.defending { 16f32 } else { 8f32 }
            + 12f32.min(bot.owned_expansions().count() as f32 * 4f32);

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
        my_army.sort(|u| u.tag() / ((u.is_flying() as u64) + 1));
        let mut defense_points = bot
            .units
            .my
            .townhalls
            .iter()
            .map(|u| u.position())
            .collect::<Vec<Point2>>();

        if defense_points.len() < 5 {
            if let Some(next_expansion) = bot
                .expansions
                .iter()
                .find(|e| e.alliance.is_neutral())
                .map(|e| e.loc)
            {
                defense_points.push(next_expansion);
            }
        }

        let enemy_attack_force = bot_state.enemy_cache.units.filter(|e| {
            defense_points
                .iter()
                .any(|h| h.is_closer(defense_range, *e))
        });

        let mut priority_targets = Units::new();
        let mut secondary_targets = Units::new();

        // Retreat when aggression is small
        // Attack when we build enough numbers again
        priority_targets.extend(bot_state.enemy_cache.units.filter(|u| u.is_dangerous()));

        secondary_targets.extend(
            bot.units
                .enemy
                .all
                .filter(|u| !priority_targets.contains_tag(u.tag())),
        );

        let our_global_strength = my_army.strength(bot);
        let their_global_strength = priority_targets.strength(bot);
        let mut our_strength_per_unit = HashMap::new();
        let mut their_strength_per_enemy_unit = HashMap::new();

        self.defending = !enemy_attack_force.is_empty();
        let defending = self.defending;
        self.money_engaging = (self.money_engaging && bot.minerals > 500) || bot.minerals > 2_000;
        self.strength_engaging = (self.strength_engaging
            && our_global_strength >= their_global_strength * 0.9f32)
            || our_global_strength >= their_global_strength * 1.3f32;

        let engaging =
            (self.money_engaging || self.strength_engaging) && self.can_be_aggressive(bot);

        for unit in priority_targets.iter() {
            let their_strength = priority_targets
                .filter(|e| {
                    e.in_real_range(unit, unit.speed() + e.speed() + unit.real_ground_range())
                })
                .strength(bot);
            their_strength_per_enemy_unit.insert(unit.tag(), their_strength);
            let our_strength = bot_state
                .squads
                .find_squads_close_by(unit)
                .iter()
                .map(|s| s.squad.strength(bot))
                .sum();
            our_strength_per_unit.insert(unit.tag(), our_strength);
        }
        let mut units_limit = priority_targets
            .iter()
            .map(|f| {
                (
                    f.tag(),
                    (f.base_strength(bot) * 2f32).max(f.strength(bot) * 3f32),
                )
            })
            .collect::<HashMap<_, _>>();

        let has_healing_queen = !bot
            .units
            .my
            .units
            .ready()
            .of_type(UnitTypeId::Queen)
            .filter(|u| u.energy().unwrap_or_default() > TRANSFUSION_MIN_ENERGY)
            .is_empty();
        for unit in my_army.iter() {
            let squad_strength = bot_state
                .squads
                .find_unit_squad(unit)
                .unwrap()
                .squad
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

            let our_strength = squad_strength.max(our_surrounding_strength);
            our_strength_per_unit.insert(unit.tag(), our_strength);

            debug!(
                "Unit[{:?}|{:?}] {:?}[{:?}|{:?}]vs{:?}",
                unit.tag(),
                unit.type_id(),
                our_strength,
                squad_strength,
                our_surrounding_strength,
                their_strength
            );

            let fallback = (unit.type_id() == UnitTypeId::Roach
                && unit.hits_percentage().unwrap_or_default() < BURROW_HEALTH_PERCENTAGE)
                || (!unit.can_attack()
                    && unit.hits_percentage().unwrap_or_default() < UNBURROW_HEALTH_PERCENTAGE)
                || (has_healing_queen
                    && unit.base_strength(bot) >= RETREAT_BASE_STRENGTH
                    && unit.hits_percentage().unwrap_or_default() < RETREAT_HEALTH_PERCENTAGE);

            let defensive_unit = defense_points
                .iter()
                .any(|h| h.is_closer(defense_range + 3f32, unit));
            let resource_reduction = bot.minerals as f32 / 10_000f32;
            let strength_multiplier = if defensive_unit {
                0.6f32
            } else if engaging {
                0.8f32
            } else if defending {
                1.0f32
            } else {
                1.2f32
            } - resource_reduction;
            let decision = if fallback {
                UnitDecision::Retreat
            } else if (defending || engaging)
                && our_strength >= their_strength * strength_multiplier
            {
                UnitDecision::Advance
            } else {
                UnitDecision::Retreat
            };

            self.allied_decision.insert(unit.tag(), decision);
        }

        for unit in my_army.iter() {
            let decision = *self.allied_decision.get(&unit.tag()).unwrap();
            let avoid_burrow = (bot.detection_close_by(unit, BURROW_DETECTION_RANGE)
                || unit.is_revealed())
                && !bot.has_upgrade(UpgradeId::TunnelingClaws);
            if unit.type_id() == UnitTypeId::Roach
                && unit.has_ability(AbilityId::BurrowDownRoach)
                && unit.hits_percentage().unwrap_or_default() < BURROW_HEALTH_PERCENTAGE
                && !avoid_burrow
            {
                unit.use_ability(AbilityId::BurrowDownRoach, false);
                continue;
            } else if unit.type_id() == UnitTypeId::RoachBurrowed {
                if unit.has_ability(AbilityId::BurrowUpRoach)
                    && (decision == UnitDecision::Advance
                        && unit.hits_percentage().unwrap_or_default() >= UNBURROW_HEALTH_PERCENTAGE
                        || avoid_burrow)
                {
                    unit.use_ability(AbilityId::BurrowUpRoach, false);
                    continue;
                }
                if !bot.has_upgrade(UpgradeId::TunnelingClaws) {
                    continue;
                }
            }
            let healing_queen = if engaging {
                bot.units
                    .my
                    .units
                    .ready()
                    .of_type(UnitTypeId::Queen)
                    .filter(|u| {
                        u.energy().unwrap_or_default() > TRANSFUSION_MIN_ENERGY
                            && !u.position().is_closer(8f32, unit)
                    })
                    .closest(unit)
                    .cloned()
            } else {
                None
            };

            let local_allied_strength = *our_strength_per_unit.get(&unit.tag()).unwrap();

            let target_in_range = priority_targets
                .iter()
                .filter(|t| {
                    unit.can_attack() && unit.can_attack_unit(t) && unit.in_real_range(t, 0.1f32)
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
                        && unit.distance(t.position()) <= 17f32
                        && *their_strength_per_enemy_unit.get(&t.tag()).unwrap() * 2f32
                            < local_allied_strength
                })
                .closest(unit);

            let extended_enemy = if unit.type_id() == UnitTypeId::Queen {
                priority_targets
                    .iter()
                    .filter(|t| {
                        unit.can_attack_unit(t)
                            && !bot
                                .units
                                .my
                                .townhalls
                                .closer(defense_range, t.position())
                                .is_empty()
                    })
                    .closest(bot.start_location)
            } else {
                priority_targets
                    .iter()
                    .filter(|t| unit.can_attack_unit(t))
                    .closest(bot.start_location)
            };

            let secondary_target = if unit.type_id() == UnitTypeId::Queen {
                None
            } else {
                secondary_targets
                    .iter()
                    .filter(|f| unit.can_attack_unit(f))
                    .closest(bot.start_location)
            };

            let mut final_target: Option<Unit> = None;

            if let Some(target) = target_in_range {
                if decision == UnitDecision::Retreat
                    && unit.weapon_cooldown().unwrap_or_default() > 10f32
                {
                    Self::move_towards(bot, unit, -2.0f32);
                } else if unit.range_vs(target) > target.range_vs(unit)
                    && unit.weapon_cooldown().unwrap_or_default() > 10f32
                {
                    Self::move_towards(bot, unit, -0.6f32);
                } else if decision == UnitDecision::Advance
                    && unit.range_vs(target) <= target.range_vs(unit)
                    && unit.weapon_cooldown().unwrap_or_default() > 10f32
                {
                    Self::move_towards(bot, unit, 0.5f32);
                } else if target.is_revealed() {
                    unit.order_attack(Target::Pos(target.position()), false);
                    final_target = Some(target.clone());
                } else {
                    final_target = Some(target.clone());
                    unit.order_attack(Target::Tag(target.tag()), false);
                }
            } else if decision == UnitDecision::Advance {
                let possible_target = if let Some(target) = closest_attackable {
                    Some(target)
                } else if let Some(target) = closest_weak {
                    Some(target)
                } else if let Some(target) = extended_enemy {
                    Some(target)
                } else {
                    secondary_target
                };
                let attack_goal = if unit.is_flying() {
                    bot.start_location
                } else if unit.type_id() == UnitTypeId::Queen {
                    if let Some(close_townhall) = bot.units.my.townhalls.closest(unit) {
                        close_townhall.position()
                    } else {
                        bot.start_location
                    }
                } else {
                    bot.enemy_start
                };
                if let Some(target) = possible_target {
                    unit.order_attack(Target::Pos(target.position()), false);
                    final_target = Some(target.clone());
                } else {
                    unit.order_attack(Target::Pos(attack_goal), false);
                }
            } else if decision == UnitDecision::Retreat || decision == UnitDecision::Undefined {
                if threats.count() > 6 && !unit.is_burrowed() {
                    Self::move_towards(bot, unit, -2f32);
                } else if let Some(ref queen) = healing_queen {
                    unit.order_move_to(Target::Pos(queen.position()), 5f32, false);
                } else if !self.defending {
                    if let Some(center) = bot.units.my.townhalls.center() {
                        unit.order_move_to(
                            Target::Pos(center.towards(bot.start_location, 1f32)),
                            10f32,
                            false,
                        );
                    }
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
            if let Some(target) = final_target {
                let limit = units_limit.entry(target.tag()).or_insert(0f32);
                *limit -= unit.strength(bot);
                if *limit < 0f32 {
                    priority_targets.remove(target.tag());
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
                    .filter(|u| {
                        !u.is_using_any(&vec![
                            AbilityId::EffectInjectLarva,
                            AbilityId::TransfusionTransfusion,
                            AbilityId::BuildCreepTumorQueen,
                        ])
                    })
                {
                    if let Some(closest_hall) = bot.units.my.townhalls.closest(queen) {
                        queen.order_move_to(Target::Pos(closest_hall.position()), 7f32, false);
                    }
                }
            }
        }
    }

    fn move_towards(bot: &Bot, unit: &Unit, multiplier: f32) {
        let center = if let Some(threat_center) = bot
            .units
            .enemy
            .all
            .filter(|t| t.can_attack_unit(unit) && unit.distance(t.position()) < 16f32)
            .center()
        {
            Some(threat_center)
        } else {
            bot.units
                .enemy
                .all
                .filter(|t| unit.distance(t.position()) < 16f32)
                .center()
        };
        if let Some(center_point) = center {
            let position = {
                let pos = unit
                    .position()
                    .towards(center_point, unit.speed() * multiplier);
                if bot.is_pathable(pos) {
                    pos
                } else {
                    *unit
                        .position()
                        .neighbors8()
                        .iter()
                        .filter(|p| bot.is_pathable(**p))
                        .furthest(center_point)
                        .unwrap_or(&bot.start_location)
                }
            };
            unit.order_move_to(Target::Pos(position), 0.5f32, false);
        }
    }

    fn queue_units(&mut self, bot: &mut Bot, bot_state: &mut BotState) {
        let extra_queens: usize = match bot_state.spending_focus {
            SpendingFocus::Economy => 2,
            SpendingFocus::Balance => 4,
            SpendingFocus::Army => 6,
        };
        let min_queens = MAX_QUEENS.min(bot.units.my.townhalls.len() + extra_queens);
        bot_state.build_queue.push(
            Command::new_unit(UnitTypeId::Queen, min_queens, false),
            false,
            PRIORITY_QUEEN,
        );
        if !bot
            .units
            .my
            .structures
            .of_type(UnitTypeId::SpawningPool)
            .is_empty()
        {
            bot_state.build_queue.push(
                Command::new_unit(UnitTypeId::Zergling, MIN_LINGS, false),
                false,
                PRIORITY_MIN_LINGS,
            );
        }

        let wanted_army_supply = (bot.supply_army + bot.supply_left) as isize;
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
    }

    fn army_distribution(
        &self,
        bot: &Bot,
        bot_state: &mut BotState,
        wanted_army_supply: isize,
    ) -> HashMap<UnitTypeId, (usize, usize)> {
        let mut unit_distribution = HashMap::new();

        for unit_type in self.allowed_tech.iter() {
            let (weight, priority) = Self::unit_value(bot, bot_state, *unit_type);
            if unit_type.has_requirement(bot) {
                unit_distribution.insert(*unit_type, (weight, priority));
            } else {
                bot_state.build_queue.push(
                    Command::new_unit(*unit_type, 1, true),
                    false,
                    priority + PRIORITY_ARMY_REQUIREMENT,
                );
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

    fn unit_value(bot: &Bot, bot_state: &BotState, unit_type: UnitTypeId) -> (isize, usize) {
        let mut value = match unit_type {
            UnitTypeId::Zergling => 2f32,
            UnitTypeId::Corruptor => 3f32,
            UnitTypeId::Mutalisk => 1f32,
            UnitTypeId::Ultralisk => 1f32,
            UnitTypeId::Ravager => 2f32,
            _ => 10f32,
        };
        let mut priority = 35f32;
        for unit in bot_state.enemy_cache.units.iter() {
            if unit.type_id().countered_by().contains(&unit_type) {
                value += unit.supply_cost();
                priority += unit.supply_cost();
            }
            if unit_type.countered_by().contains(&unit.type_id()) {
                value -= unit.supply_cost();
            }
        }
        priority -= (bot.units.my.units.of_type(unit_type).supply() / 2) as f32;
        (value.max(1f32) as isize, priority.max(16f32) as usize)
    }

    fn queue_upgrades(&self, bot: &mut Bot, bot_state: &mut BotState) {
        if bot.counter().all().count(UnitTypeId::SpawningPool) > 0
            && bot.can_afford_vespene_upgrade(UpgradeId::Zerglingmovementspeed)
        {
            bot_state.build_queue.push(
                Command::new_upgrade(UpgradeId::Zerglingmovementspeed, true),
                false,
                PRIORITY_LING_SPEED,
            );
        }
        if bot_state.spending_focus == SpendingFocus::Army {
            return;
        }
        if bot.counter().all().count(UnitTypeId::Zergling) > 20
            && bot.counter().all().count(UnitTypeId::Hive) > 0
        {
            bot_state.build_queue.push(
                Command::new_upgrade(UpgradeId::Zerglingattackspeed, false),
                false,
                150,
            );
        }
        let workers = bot.counter().all().count(bot.race_values.worker);
        if workers >= UNLOCK_BURROW_WORKERS && bot.counter().all().count(UnitTypeId::Lair) > 0 {
            bot_state.build_queue.push(
                Command::new_upgrade(UpgradeId::Burrow, true),
                false,
                PRIORITY_BURROW,
            );
        }
        if workers >= UNLOCK_TUNNELING_CLAWS_WORKERS {
            bot_state.build_queue.push(
                Command::new_upgrade(UpgradeId::TunnelingClaws, true),
                false,
                PRIORITY_TUNNELING_CLAWS,
            );
        }
        if workers >= OVERLORD_SPEED_WORKERS && bot.counter().all().count(UnitTypeId::Lair) > 0 {
            bot_state.build_queue.push(
                Command::new_upgrade(
                    UpgradeId::Overlordspeed,
                    bot.can_afford_vespene_upgrade(UpgradeId::Overlordspeed),
                ),
                false,
                PRIORITY_LORD_SPEED,
            );
        }
        if bot.counter().all().count(UnitTypeId::Baneling) > 0 {
            bot_state.build_queue.push(
                Command::new_upgrade(UpgradeId::CentrificalHooks, false),
                false,
                150,
            );
        }
        if bot.counter().all().count(UnitTypeId::RoachWarren) > 0 {
            bot_state.build_queue.push(
                Command::new_upgrade(UpgradeId::GlialReconstitution, true),
                false,
                200,
            );
        }
        if bot.counter().all().count(UnitTypeId::HydraliskDen) > 0 {
            bot_state.build_queue.push(
                Command::new_upgrade(UpgradeId::EvolveGroovedSpines, false),
                bot.counter().all().count(UnitTypeId::Hydralisk) > 0,
                210,
            );
            bot_state.build_queue.push(
                Command::new_upgrade(UpgradeId::EvolveMuscularAugments, true),
                bot.counter().all().count(UnitTypeId::Hydralisk) > 0,
                220,
            );
        }

        let chambers = 0.max(3.min(workers.saturating_sub(30) / 10));
        if chambers > 0 {
            bot_state.build_queue.push(
                Command::new_unit(
                    UnitTypeId::EvolutionChamber,
                    chambers,
                    bot_state.spending_focus == SpendingFocus::Economy,
                ),
                false,
                PRIORITY_EVOLUTION_CHAMBER,
            );
        }
        if workers >= UNLOCK_UPGRADES_WORKERS {
            let melee_number = bot
                .units
                .my
                .units
                .filter(|u| u.is_melee() && !u.is_worker())
                .len();
            if melee_number > SAVE_FOR_ATTACK_UPGRADES_ON_UNITS
                && bot.upgrade_progress(UpgradeId::ZergGroundArmorsLevel1) > 0.1f32
            {
                self.queue_upgrade(
                    bot,
                    bot_state,
                    [
                        UpgradeId::ZergMeleeWeaponsLevel1,
                        UpgradeId::ZergMeleeWeaponsLevel2,
                        UpgradeId::ZergMeleeWeaponsLevel3,
                    ],
                    PRIORITY_MELEE_WEAPON,
                );
            }

            let ground_number = bot
                .units
                .my
                .units
                .filter(|u| !u.is_flying() && !u.is_worker())
                .len();
            if ground_number > SAVE_FOR_DEFENSE_UPGRADES_ON_UNITS
                && bot.can_afford_vespene_upgrade(UpgradeId::ZergGroundArmorsLevel1)
            {
                self.queue_upgrade(
                    bot,
                    bot_state,
                    [
                        UpgradeId::ZergGroundArmorsLevel1,
                        UpgradeId::ZergGroundArmorsLevel2,
                        UpgradeId::ZergGroundArmorsLevel3,
                    ],
                    PRIORITY_GROUND_ARMOR,
                );
            }

            let ranged_number = bot
                .units
                .my
                .units
                .filter(|u| !u.is_melee() && !u.is_worker())
                .len();
            if ranged_number > SAVE_FOR_ATTACK_UPGRADES_ON_UNITS
                && bot.upgrade_progress(UpgradeId::ZergGroundArmorsLevel1) > 0.1f32
            {
                self.queue_upgrade(
                    bot,
                    bot_state,
                    [
                        UpgradeId::ZergMissileWeaponsLevel1,
                        UpgradeId::ZergMissileWeaponsLevel2,
                        UpgradeId::ZergMissileWeaponsLevel3,
                    ],
                    PRIORITY_MISSILE_WEAPON,
                );
            }

            if bot.counter().count(UnitTypeId::GreaterSpire) > 0 {
                let flying_number = bot.units.my.units.filter(|u| u.is_flying()).len();
                if flying_number > SAVE_FOR_DEFENSE_UPGRADES_ON_UNITS {
                    self.queue_upgrade(
                        bot,
                        bot_state,
                        [
                            UpgradeId::ZergFlyerArmorsLevel1,
                            UpgradeId::ZergFlyerArmorsLevel2,
                            UpgradeId::ZergFlyerArmorsLevel3,
                        ],
                        PRIORITY_FLYER_ARMOR,
                    );
                }
                if flying_number > SAVE_FOR_ATTACK_UPGRADES_ON_UNITS {
                    self.queue_upgrade(
                        bot,
                        bot_state,
                        [
                            UpgradeId::ZergFlyerWeaponsLevel1,
                            UpgradeId::ZergFlyerWeaponsLevel2,
                            UpgradeId::ZergFlyerWeaponsLevel3,
                        ],
                        PRIORITY_FLYER_WEAPON,
                    );
                }
            }
        }
        if bot.counter().all().count(UnitTypeId::Ultralisk) > 0 {
            bot_state.build_queue.push(
                Command::new_upgrade(UpgradeId::ChitinousPlating, false),
                false,
                150,
            );
            bot_state.build_queue.push(
                Command::new_upgrade(UpgradeId::AnabolicSynthesis, true),
                false,
                155,
            );
        }
    }

    fn queue_upgrade(
        &self,
        bot: &Bot,
        bot_state: &mut BotState,
        upgrades: [UpgradeId; 3],
        priority: usize,
    ) {
        let upgrade = *upgrades.first().unwrap();
        bot_state
            .build_queue
            .push(Command::new_upgrade(upgrade, true), false, priority);
        for (&requirement, &upgrade) in upgrades.iter().zip(upgrades.iter().skip(1)) {
            if bot.has_upgrade(requirement) {
                bot_state
                    .build_queue
                    .push(Command::new_upgrade(upgrade, true), false, priority);
            }
        }
    }

    fn can_be_aggressive(&self, bot: &Bot) -> bool {
        bot.units.my.units.of_type(UnitTypeId::Zergling).is_empty()
            || bot.enemy_race != Race::Zerg
            || bot.upgrade_progress(UpgradeId::Zerglingmovementspeed) > 0.9f32
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
