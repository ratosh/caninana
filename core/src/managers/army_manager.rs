use std::collections::{HashMap, HashSet};

use log::debug;
use rand::prelude::*;
use rust_sc2::bot::Bot;
use rust_sc2::prelude::*;
use rust_sc2::units::Container;
use rust_sc2::Event::UnitDestroyed;

use crate::command_queue::Command;
use crate::{BotInfo, EventListener, Manager};

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

pub struct ArmyManager {
    last_loop: u32,
    going_aggro: bool,
    attack_wave_size: usize,
    retreat_wave_size: usize,
    defending: bool,
    retreating: HashSet<u64>,
    allowed_tech: HashSet<UnitTypeId>,
    enemy_units: HashMap<u64, UnitCache>,
}

impl ArmyManager {
    fn tech_decision(&mut self, bot: &Bot) {
        let drones = bot.counter().all().count(UnitTypeId::Drone);
        if !bot.units.enemy.all.is_empty() || drones >= 19 {
            self.allowed_tech.insert(UnitTypeId::Zergling);
        }
        if drones >= 25 {
            self.allowed_tech.insert(UnitTypeId::Roach);
        }
        if drones >= 38 {
            self.allowed_tech.insert(UnitTypeId::Ravager);
        }
        if drones >= 50 || !bot.units.enemy.all.flying().is_empty() {
            self.allowed_tech.insert(UnitTypeId::Hydralisk);
        }
        if drones >= 66 {
            self.allowed_tech.insert(UnitTypeId::Mutalisk);
            self.allowed_tech.insert(UnitTypeId::Corruptor);
            self.allowed_tech.insert(UnitTypeId::Ultralisk);
        }
    }
}

impl Default for ArmyManager {
    fn default() -> Self {
        Self {
            last_loop: 0,
            going_aggro: false,
            attack_wave_size: 4,
            retreat_wave_size: 2,
            defending: false,
            retreating: HashSet::new(),
            allowed_tech: HashSet::new(),
            enemy_units: HashMap::new(),
        }
    }
}

impl ArmyManager {
    const CACHE_TIME: f32 = 20f32;

    pub fn destroy_unit(&mut self, tag: u64) {
        self.enemy_units.remove(&tag);
    }

    fn check_unit_cache(&mut self, bot: &Bot) {
        for unit in bot.units.enemy.all.iter() {
            self.enemy_units
                .insert(unit.tag(), UnitCache::new(unit.clone(), bot.time));
        }
        self.enemy_units
            .retain(|_, value| value.last_seen + Self::CACHE_TIME > bot.time);
    }

    fn enemy_supply(&self, _: &Bot) -> usize {
        self.enemy_units
            .values()
            .filter(|p| !p.unit.is_worker())
            .map(|u| u.unit.supply_cost())
            .sum::<f32>() as usize
    }

    fn our_supply(&self, bot: &Bot) -> usize {
        bot.units
            .my
            .units
            .ready()
            .filter(|u| !u.is_worker() && !self.retreating.contains(&u.tag()))
            .iter()
            .map(|u| u.supply_cost())
            .sum::<f32>() as usize
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
            } else if overlord.health_percentage().unwrap_or_default() > 0.9f32
                && overlord.is_idle()
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
        let our_supply = self.our_supply(bot);
        let enemy_supply = self.enemy_supply(bot);
        debug!(
            "{:?}>{:?}|{:?}|{:?}",
            self.attack_wave_size, self.retreat_wave_size, our_supply, enemy_supply
        );
        let should_keep_aggro =
            our_supply >= self.retreat_wave_size && (our_supply > enemy_supply * 2 / 3);
        let should_go_aggro = (our_supply >= self.attack_wave_size) || bot.supply_used > 190;
        self.going_aggro = (self.going_aggro && should_keep_aggro) || should_go_aggro;

        self.attack_wave_size = self.attack_wave_size.max(our_supply);
        self.retreat_wave_size = self.attack_wave_size * 3 / 5;

        if self.going_aggro {
            priority_targets.extend(bot.units.enemy.all.filter(|u| {
                !u.is_flying()
                    && u.can_attack()
                    && u.can_be_attacked()
                    && u.type_id() != UnitTypeId::Larva
            }));

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
        } else {
            priority_targets.extend(enemy_attack_force);
        };

        Self::overseer_micro(bot);

        if !priority_targets.is_empty() {
            debug!("Attacking targets");
            for u in my_army.iter() {
                if u.health_percentage().unwrap() > 0.8 {
                    self.retreating.remove(&u.tag());
                } else if u.health_percentage().unwrap() < 0.5 {
                    self.retreating.insert(u.tag());
                }
                let is_retreating = self.retreating.contains(&u.tag());
                if is_retreating && !u.on_cooldown() && !u.is_melee() {
                    if let Some(target) = priority_targets
                        .iter()
                        .filter(|t| u.in_range(t, 0.0))
                        .min_by_key(|t| t.hits())
                    {
                        u.order_attack(Target::Tag(target.tag()), false);
                    }
                } else if !u.is_melee() && (is_retreating || u.on_cooldown()) {
                    if let Some(closest_attacker) = bot
                        .units
                        .enemy
                        .all
                        .filter(|t| t.in_range(u, t.speed() + u.speed()))
                        .closest(u)
                    {
                        let flee_position = {
                            let pos = u
                                .position()
                                .towards(closest_attacker.position(), -u.speed());
                            if bot.is_pathable(pos) {
                                pos
                            } else {
                                *u.position()
                                    .neighbors8()
                                    .iter()
                                    .filter(|p| bot.is_pathable(**p))
                                    .furthest(closest_attacker)
                                    .unwrap_or(&bot.start_location)
                            }
                        };
                        u.order_move_to(Target::Pos(flee_position), 0.1f32, false);
                    }
                } else {
                    match priority_targets
                        .iter()
                        .filter(|t| u.in_range(t, 0.1))
                        .min_by_key(|t| t.hits())
                    {
                        Some(target) => u.order_attack(Target::Tag(target.tag()), false),
                        None => {
                            if let Some(closest) = priority_targets
                                .iter()
                                .filter(|t| u.can_attack_unit(t))
                                .closest(u)
                            {
                                u.order_attack(Target::Pos(closest.position()), false);
                            } else if let Some(closest) = secondary_targets
                                .iter()
                                .filter(|t| u.can_attack_unit(t))
                                .closest(u)
                            {
                                u.order_attack(Target::Pos(closest.position()), false);
                            }
                        }
                    }
                }
            }
        } else if !secondary_targets.is_empty() {
            for u in my_army.iter() {
                if let Some(target) = secondary_targets
                    .iter()
                    .filter(|t| u.can_attack_unit(t))
                    .furthest(bot.enemy_start)
                {
                    u.order_attack(Target::Pos(target.position()), false);
                }
            }
        } else {
            let target = if self.going_aggro {
                debug!("Go to enemy start");
                bot.enemy_start
            } else if let Some(base_center) = bot.units.my.townhalls.closest(bot.enemy_start) {
                debug!("Go to defensive position 1");
                base_center.position().towards(bot.enemy_start, 5f32)
            } else {
                debug!("Go to defensive position 2");
                bot.start_location.towards(bot.enemy_start, 5f32)
            };
            for u in &my_army {
                if u.distance(target) > u.ground_range() + u.speed() * 2f32 {
                    u.order_attack(Target::Pos(target), false);
                }
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
                .closest(overseer)
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
        let mut wanted_army_supply = if drones < 76 {
            (drones * 3 / 4) as isize
        } else {
            (bot.supply_army + bot.supply_left) as isize
        };

        wanted_army_supply -= (min_queens * 2) as isize;

        // TODO: Base a difference on enemy units
        // TODO: When facing air enemies make anti-air
        let unit_distribution = self.army_distribution(bot);
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
                println!(
                    "Unit {:?}>{:?}|{:?}[{:?}]",
                    unit_type, existing_supply, dedicated_supply, amount
                );
            }
            println!(
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

    fn army_distribution(&self, bot: &Bot) -> HashMap<UnitTypeId, isize> {
        let mut unit_distribution = HashMap::new();

        for unit_type in self.allowed_tech.iter() {
            unit_distribution.insert(*unit_type, Self::unit_value(bot, *unit_type));
        }
        unit_distribution
    }

    fn unit_value(bot: &Bot, unit_type: UnitTypeId) -> isize {
        let mut value = match unit_type {
            UnitTypeId::Zergling => 10f32,
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
                true,
                80,
            );
        }
        if bot.counter().all().count(UnitTypeId::Zergling) > 20 {
            bot_info.build_queue.push(
                Command::new_upgrade(UpgradeId::Zerglingattackspeed, false),
                false,
                50,
            );
        }
        if bot.counter().all().count(UnitTypeId::Overseer) > 0 {
            bot_info.build_queue.push(
                Command::new_upgrade(UpgradeId::Overlordspeed, true),
                false,
                50,
            );
        }
        if (!bot.is_ordered_upgrade(UpgradeId::Zerglingmovementspeed)
            && !bot.has_upgrade(UpgradeId::Zerglingmovementspeed))
            || bot.units.my.townhalls.len() < 3
        {
            return;
        }
        if bot.counter().all().count(UnitTypeId::Baneling) > 0 {
            bot_info.build_queue.push(
                Command::new_upgrade(UpgradeId::CentrificalHooks, true),
                false,
                50,
            );
        }
        if bot.counter().all().count(UnitTypeId::Roach) > 0 {
            bot_info.build_queue.push(
                Command::new_upgrade(UpgradeId::GlialReconstitution, true),
                false,
                50,
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
        if bot.counter().all().count(UnitTypeId::Zergling) > 0 && bot.vespene > 150 {
            bot_info.build_queue.push(
                Command::new_upgrade(UpgradeId::ZergMeleeWeaponsLevel1, true),
                false,
                70,
            );
            bot_info.build_queue.push(
                Command::new_upgrade(UpgradeId::ZergGroundArmorsLevel1, true),
                false,
                70,
            );
        }
        if bot.counter().all().count(bot.race_values.worker) > 45 {
            bot_info.build_queue.push(
                Command::new_unit(UnitTypeId::EvolutionChamber, 2, false),
                false,
                50,
            );
        }
        if bot.counter().all().count(UnitTypeId::Zergling) > 0 && bot.vespene > 250 {
            bot_info.build_queue.push(
                Command::new_upgrade(UpgradeId::ZergMeleeWeaponsLevel2, true),
                false,
                60,
            );
            bot_info.build_queue.push(
                Command::new_upgrade(UpgradeId::ZergGroundArmorsLevel2, true),
                false,
                60,
            );
        }
        if bot.counter().all().count(UnitTypeId::Roach) > 0 {
            bot_info.build_queue.push(
                Command::new_upgrade(UpgradeId::ZergMissileWeaponsLevel1, true),
                false,
                70,
            );
        }
        if bot.counter().all().count(UnitTypeId::Roach) > 0 && bot.vespene > 350 {
            bot_info.build_queue.push(
                Command::new_upgrade(UpgradeId::ZergMissileWeaponsLevel2, true),
                false,
                60,
            );
            bot_info.build_queue.push(
                Command::new_upgrade(UpgradeId::ZergGroundArmorsLevel2, true),
                false,
                60,
            );
        }
        if bot.vespene > 450 {
            bot_info.build_queue.push(
                Command::new_upgrade(UpgradeId::ZergMeleeWeaponsLevel3, true),
                false,
                50,
            );
            bot_info.build_queue.push(
                Command::new_upgrade(UpgradeId::ZergGroundArmorsLevel3, true),
                false,
                50,
            );
            bot_info.build_queue.push(
                Command::new_upgrade(UpgradeId::ZergMissileWeaponsLevel3, true),
                false,
                50,
            );
            bot_info.build_queue.push(
                Command::new_upgrade(UpgradeId::Zerglingattackspeed, true),
                false,
                50,
            );
        }
        if bot.counter().all().count(UnitTypeId::Ultralisk) > 0 {
            bot_info.build_queue.push(
                Command::new_upgrade(UpgradeId::ChitinousPlating, true),
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
                UnitTypeId::BroodLord,
            ],
            UnitTypeId::Sentry => vec![UnitTypeId::BroodLord, UnitTypeId::Ultralisk],
            UnitTypeId::Stalker => vec![UnitTypeId::Roach, UnitTypeId::Hydralisk],
            UnitTypeId::Immortal => vec![UnitTypeId::Zergling, UnitTypeId::Hydralisk],
            UnitTypeId::Colossus => vec![UnitTypeId::Corruptor],
            UnitTypeId::Phoenix => vec![UnitTypeId::Hydralisk],
            UnitTypeId::VoidRay => vec![UnitTypeId::Hydralisk],
            UnitTypeId::HighTemplar => vec![UnitTypeId::Ultralisk],
            UnitTypeId::DarkTemplar => vec![UnitTypeId::Mutalisk, UnitTypeId::BroodLord],
            UnitTypeId::Carrier => vec![UnitTypeId::Corruptor],
            UnitTypeId::Mothership => vec![UnitTypeId::Corruptor],
            UnitTypeId::Oracle => vec![UnitTypeId::Mutalisk],
            UnitTypeId::Tempest => vec![UnitTypeId::Corruptor],
            UnitTypeId::Adept => vec![UnitTypeId::Roach],
            UnitTypeId::Disruptor => vec![UnitTypeId::Ultralisk],
            // Race::Terran
            UnitTypeId::Marine => vec![
                UnitTypeId::Baneling,
                UnitTypeId::Roach,
                UnitTypeId::Ultralisk,
                UnitTypeId::BroodLord,
                UnitTypeId::LurkerMP,
            ],
            UnitTypeId::Marauder => vec![
                UnitTypeId::Hydralisk,
                UnitTypeId::Mutalisk,
                UnitTypeId::BroodLord,
            ],
            UnitTypeId::Medivac => vec![
                UnitTypeId::Hydralisk,
                UnitTypeId::Mutalisk,
                UnitTypeId::Corruptor,
            ],
            UnitTypeId::Reaper => vec![UnitTypeId::Roach],
            UnitTypeId::Ghost => vec![UnitTypeId::Roach, UnitTypeId::Ultralisk],
            UnitTypeId::Hellion => vec![UnitTypeId::Roach, UnitTypeId::Mutalisk],
            UnitTypeId::SiegeTank => vec![
                UnitTypeId::Mutalisk,
                UnitTypeId::BroodLord,
                UnitTypeId::Ravager,
            ],
            UnitTypeId::SiegeTankSieged => vec![
                UnitTypeId::Mutalisk,
                UnitTypeId::BroodLord,
                UnitTypeId::Ravager,
            ],
            UnitTypeId::Thor => vec![
                UnitTypeId::Zergling,
                UnitTypeId::Hydralisk,
                UnitTypeId::BroodLord,
            ],
            UnitTypeId::Banshee => vec![UnitTypeId::Mutalisk, UnitTypeId::Corruptor],
            UnitTypeId::Viking => vec![UnitTypeId::Hydralisk],
            UnitTypeId::Raven => vec![UnitTypeId::Corruptor],
            UnitTypeId::Battlecruiser => vec![UnitTypeId::Corruptor],
            UnitTypeId::HellionTank => vec![UnitTypeId::Baneling],
            UnitTypeId::Liberator => vec![UnitTypeId::Corruptor],
            // Race::Zerg
            UnitTypeId::Zergling => vec![
                UnitTypeId::Zealot,
                UnitTypeId::Sentry,
                UnitTypeId::Colossus,
                UnitTypeId::Hellion,
                UnitTypeId::HellionTank,
                UnitTypeId::Baneling,
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
                UnitTypeId::BroodLord,
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
                UnitTypeId::BroodLord,
            ],
            UnitTypeId::Mutalisk => vec![
                UnitTypeId::Sentry,
                UnitTypeId::Phoenix,
                UnitTypeId::Marine,
                UnitTypeId::Thor,
                UnitTypeId::Hydralisk,
                UnitTypeId::Corruptor,
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
                UnitTypeId::BroodLord,
            ],
            UnitTypeId::Ultralisk => vec![
                UnitTypeId::Immortal,
                UnitTypeId::VoidRay,
                UnitTypeId::Banshee,
                UnitTypeId::Hydralisk,
                UnitTypeId::BroodLord,
            ],
            UnitTypeId::BroodLord => vec![
                UnitTypeId::Stalker,
                UnitTypeId::VoidRay,
                UnitTypeId::Phoenix,
                UnitTypeId::Viking,
                UnitTypeId::Corruptor,
            ],
            UnitTypeId::Viper => vec![
                UnitTypeId::Phoenix,
                UnitTypeId::Viking,
                UnitTypeId::Mutalisk,
                UnitTypeId::Corruptor,
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
            _ => vec![],
        }
    }
}

impl ArmyManager {
    const PROCESS_DELAY: u32 = 10;
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
