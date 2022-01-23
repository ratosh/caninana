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
    unit_type: UnitTypeId,
    last_seen: f32,
}

impl UnitCache {
    fn new(unit_type: UnitTypeId, time: f32) -> Self {
        Self {
            unit_type,
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
    enemy_units: HashMap<u64, UnitCache>, // TODO: Forget about enemy units not seen for for a long time (30 seconds+)
}

impl Default for ArmyManager {
    fn default() -> Self {
        Self {
            last_loop: 0,
            going_aggro: false,
            attack_wave_size: 16,
            retreat_wave_size: 8,
            defending: false,
            retreating: HashSet::new(),
            enemy_units: HashMap::new(),
        }
    }
}

impl ArmyManager {
    const CACHE_TIME: f32 = 60f32;

    pub fn destroy_unit(&mut self, tag: u64) {
        self.enemy_units.remove(&tag);
    }

    fn check_unit_cache(&mut self, bot: &Bot) {
        for unit in bot.units.enemy.all.iter() {
            if !self.enemy_units.contains(&unit.tag()) {
                debug!(
                    "Found a new unit {:?} {:?} ({:?})",
                    unit.type_id(),
                    unit.tag(),
                    unit.position()
                );
            }
            self.enemy_units
                .insert(unit.tag(), UnitCache::new(unit.type_id(), bot.time));
        }
        self.enemy_units
            .retain(|_, value| value.last_seen + Self::CACHE_TIME > bot.time);
    }

    fn enemy_supply(&self, bot: &Bot) -> usize {
        self.enemy_units
            .values()
            .filter(|p| !p.unit_type.is_worker())
            .map(|u| bot.game_data.units[&u.unit_type].food_required)
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
                    f.can_attack_air() && f.in_range(overlord, f.speed() + overlord.speed())
                })
                .iter()
                .closest(overlord)
            {
                let position = overlord.position().towards(
                    closest_anti_air.position(),
                    -closest_anti_air.range_vs(overlord),
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
        my_army.extend(bot.units.my.units.ready().of_type(UnitTypeId::Hydralisk));
        my_army.extend(bot.units.my.units.ready().of_type(UnitTypeId::Corruptor));
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
        let has_speed_boost = bot.has_upgrade(UpgradeId::Zerglingmovementspeed)
            || bot.upgrade_progress(UpgradeId::Zerglingmovementspeed) >= 0.9f32;
        let our_supply = self.our_supply(bot);
        let enemy_supply = self.enemy_supply(bot);
        debug!(
            "{:?}>{:?}|{:?}|{:?}",
            self.attack_wave_size, self.retreat_wave_size, our_supply, enemy_supply
        );
        let should_keep_aggro =
            our_supply >= self.retreat_wave_size && (our_supply > enemy_supply * 2 / 3);
        let should_go_aggro = (our_supply >= self.attack_wave_size) || bot.supply_used > 190;
        self.going_aggro =
            has_speed_boost && ((self.going_aggro && should_keep_aggro) || should_go_aggro);

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

        let overseers = bot.units.my.units.of_type(UnitTypeId::Overseer);
        for overseer in overseers.iter() {
            let position = if let Some(closest_anti_air) = bot
                .units
                .enemy
                .all
                .filter(|f| {
                    f.can_attack_air() && f.in_range(overseer, f.speed() + overseer.speed())
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
                .filter(|f| !f.is_worker() && !f.is_structure())
                .closest(overseer)
            {
                closest_enemy.position()
            } else if let Some(army_center) = my_army.center() {
                army_center
            } else {
                bot.start_location
            };
            overseer.order_move_to(Target::Pos(position), 1.0f32, false);
        }

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

    fn queue_units(&mut self, bot: &mut Bot, bot_info: &mut BotInfo) {
        let min_queens = 7.min(bot.units.my.townhalls.len() + 1);
        bot_info
            .build_queue
            .push(Command::new_unit(UnitTypeId::Queen, min_queens), false, 35);

        let drones = bot.counter().all().count(UnitTypeId::Drone);
        // Try to have some lings
        let min_lings = drones;
        bot_info.build_queue.push(
            Command::new_unit(UnitTypeId::Zergling, min_lings),
            false,
            30,
        );

        if !bot.is_ordered_upgrade(UpgradeId::Zerglingmovementspeed)
            && !bot.has_upgrade(UpgradeId::Zerglingmovementspeed)
        {
            return;
        }

        // TODO: Base a difference on enemy units
        // TODO: When facing air enemies make anti-air
        // if drones > 32 {
        //     let wanted_roaches = drones / 6;
        //     bot_info
        //         .build_queue
        //         .push(Command::new_unit(UnitTypeId::Roach, wanted_roaches), false, 35);
        // }
        // if drones > 48 {
        //     let wanted_hydras = drones / 6;
        //     bot_info.build_queue.push(
        //         Command::new_unit(UnitTypeId::Hydralisk, wanted_hydras),
        //         false,
        //         35,
        //     );
        // }
        if !bot
            .units
            .enemy
            .all
            .filter(|u| u.is_cloaked() || u.is_burrowed())
            .is_empty()
        {
            bot_info
                .build_queue
                .push(Command::new_unit(UnitTypeId::Overseer, 1), false, 1);
        }

        if drones > 60 {
            bot_info
                .build_queue
                .push(Command::new_unit(UnitTypeId::Ultralisk, 10), false, 40);
            if bot.units.enemy.structures.ground().is_empty()
                && !bot.units.enemy.structures.flying().is_empty()
            {
                bot_info
                    .build_queue
                    .push(Command::new_unit(UnitTypeId::Corruptor, 4), false, 50);
            }
        }
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
                Command::new_upgrade(UpgradeId::Zerglingmovementspeed),
                true,
                80,
            );
        }
        if bot.counter().all().count(UnitTypeId::Zergling) > 20 {
            bot_info.build_queue.push(
                Command::new_upgrade(UpgradeId::Zerglingattackspeed),
                false,
                50,
            );
        }
        if bot.counter().all().count(UnitTypeId::Overseer) > 0 {
            bot_info
                .build_queue
                .push(Command::new_upgrade(UpgradeId::Overlordspeed), false, 50);
        }
        if (!bot.is_ordered_upgrade(UpgradeId::Zerglingmovementspeed)
            && !bot.has_upgrade(UpgradeId::Zerglingmovementspeed))
            || bot.units.my.townhalls.len() < 3
        {
            return;
        }
        if bot.counter().all().count(UnitTypeId::Baneling) > 0 {
            bot_info
                .build_queue
                .push(Command::new_upgrade(UpgradeId::CentrificalHooks), false, 50);
        }
        if bot.counter().all().count(UnitTypeId::Roach) > 0 && bot.vespene > 100 {
            bot_info.build_queue.push(
                Command::new_upgrade(UpgradeId::GlialReconstitution),
                false,
                50,
            );
        }
        if bot.counter().all().count(UnitTypeId::Hydralisk) > 0 && bot.vespene > 100 {
            bot_info.build_queue.push(
                Command::new_upgrade(UpgradeId::EvolveGroovedSpines),
                false,
                80,
            );
            bot_info.build_queue.push(
                Command::new_upgrade(UpgradeId::EvolveMuscularAugments),
                false,
                80,
            );
        }
        if bot.counter().all().count(UnitTypeId::Zergling) > 5 && bot.vespene > 150 {
            bot_info.build_queue.push(
                Command::new_upgrade(UpgradeId::ZergMeleeWeaponsLevel1),
                false,
                70,
            );
            bot_info.build_queue.push(
                Command::new_upgrade(UpgradeId::ZergGroundArmorsLevel1),
                false,
                70,
            );
        }
        if bot.counter().all().count(UnitTypeId::Zergling) > 10 && bot.vespene > 150 {
            bot_info.build_queue.push(
                Command::new_unit(UnitTypeId::EvolutionChamber, 2),
                false,
                50,
            );
            bot_info.build_queue.push(
                Command::new_upgrade(UpgradeId::ZergMeleeWeaponsLevel2),
                false,
                60,
            );
            bot_info.build_queue.push(
                Command::new_upgrade(UpgradeId::ZergGroundArmorsLevel2),
                false,
                60,
            );
        }
        if bot.counter().all().count(UnitTypeId::Roach) > 0 {
            bot_info.build_queue.push(
                Command::new_upgrade(UpgradeId::ZergMissileWeaponsLevel1),
                false,
                70,
            );
        }
        if bot.counter().all().count(UnitTypeId::Roach) > 5 && bot.vespene > 250 {
            bot_info.build_queue.push(
                Command::new_upgrade(UpgradeId::ZergMissileWeaponsLevel2),
                false,
                60,
            );
            bot_info.build_queue.push(
                Command::new_upgrade(UpgradeId::ZergMissileWeaponsLevel3),
                false,
                50,
            );
        }
        if bot.vespene > 350 {
            bot_info.build_queue.push(
                Command::new_upgrade(UpgradeId::ZergMeleeWeaponsLevel3),
                false,
                50,
            );
            bot_info.build_queue.push(
                Command::new_upgrade(UpgradeId::ZergGroundArmorsLevel3),
                false,
                50,
            );
            bot_info.build_queue.push(
                Command::new_upgrade(UpgradeId::Zerglingattackspeed),
                false,
                50,
            );
        }
        if bot.counter().all().count(UnitTypeId::Ultralisk) > 0 {
            bot_info
                .build_queue
                .push(Command::new_upgrade(UpgradeId::ChitinousPlating), false, 50);
            bot_info.build_queue.push(
                Command::new_upgrade(UpgradeId::AnabolicSynthesis),
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
