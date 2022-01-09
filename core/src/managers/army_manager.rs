use std::collections::HashSet;

use log::debug;
use rand::prelude::*;
use rust_sc2::bot::Bot;
use rust_sc2::prelude::*;

use crate::command_queue::Command;
use crate::{BotInfo, Manager};

pub struct ArmyManager {
    last_loop: u32,
    going_aggro: bool,
    attack_wave_size: usize,
    retreat_wave_size: usize,
    defending: bool,
    retreating: HashSet<u64>,
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
        }
    }
}

impl ArmyManager {
    fn scout(&self, bot: &mut Bot) {
        let overs = bot.units.my.units.of_type(UnitTypeId::Overlord).idle();
        if bot.units.enemy.all.is_empty() {
            let mut rng = thread_rng();
            for overlord in overs {
                let random_x = (rng.next_u64() % bot.game_info.map_size.x as u64) as f32;
                let random_y = (rng.next_u64() % bot.game_info.map_size.y as u64) as f32;
                overlord.order_move_to(
                    Target::Pos(Point2 {
                        x: random_x,
                        y: random_y,
                    }),
                    false,
                );
            }
        } else {
            for overlord in overs {
                if overlord.position().distance_squared(bot.start_location) > 15f32 {
                    overlord.order_move_to(Target::Pos(bot.start_location), false);
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
            my_army.extend(bot.units.my.units.ready().of_type(UnitTypeId::Queen)
                .filter(|u| !u.is_using(AbilityId::EffectInjectLarva)));
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
        let has_speed_boost = bot.has_upgrade(UpgradeId::Zerglingmovementspeed);
        let active_army_size = my_army
            .filter(|u| !self.retreating.contains(&u.tag()))
            .len();
        self.going_aggro = has_speed_boost
            && ((self.going_aggro && active_army_size >= self.retreat_wave_size)
                || (!self.going_aggro
                    && (active_army_size >= self.attack_wave_size * 9 / 10 || bot.supply_used > 190)));

        self.attack_wave_size = self.attack_wave_size.max(active_army_size);
        self.retreat_wave_size = self.attack_wave_size / 2;

        if self.going_aggro {
            priority_targets.extend(
                bot.units
                    .enemy
                    .all
                    .filter(|u| u.can_attack() && u.is_visible()),
            );
            secondary_targets.extend(
                bot.units
                    .enemy
                    .all
                    .filter(|u| !u.can_attack() && u.is_visible()),
            );
        } else {
            priority_targets.extend(enemy_attack_force.clone());
        };

        let overseers = bot.units.my.units.of_type(UnitTypeId::Overseer);
        if !overseers.is_empty() {
            let mut invisible_enemies = bot.units.enemy.all.filter(|u| u.is_cloaked() || u.is_burrowed());
            for overseer in overseers.iter() {
                if let Some(closest_invisible) = invisible_enemies.closest(overseer) {
                    overseer.order_move_to(Target::Tag(closest_invisible.tag()), false);
                    invisible_enemies.remove(closest_invisible.tag());
                } else if let Some(army_center) = my_army.center() {
                    overseer.order_move_to(Target::Pos(army_center), false);
                }
            }
        }

        if !priority_targets.is_empty() {
            debug!("Attacking targets");
            for u in my_army.iter() {
                if u.health_percentage().unwrap() > 0.75 {
                    self.retreating.remove(&u.tag());
                } else if u.health_percentage().unwrap() < 0.5 {
                    self.retreating.insert(u.tag());
                }
                let is_retreating = self.retreating.contains(&u.tag());
                if !u.is_melee() && (is_retreating || u.on_cooldown())  {
                    if let Some(closest_attacker) = bot
                        .units
                        .enemy
                        .all
                        .filter(|t| t.in_range(u, t.speed() + if is_retreating { 2.0 } else { 0.5 }))
                        .closest(u)
                    {
                        let flee_position = {
                            let pos = u.position()
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
                        u.order_move_to(Target::Pos(flee_position), false);
                    } else if u.has_ability(AbilityId::BurrowDown) {
                        u.use_ability(AbilityId::BurrowDown, false);
                    }
                } else {
                    if u.has_ability(AbilityId::BurrowUp) {
                        u.use_ability(AbilityId::BurrowUp, false);
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
                                }
                            }
                        }
                    }
                }
            }
        } else if !secondary_targets.is_empty() {
            for u in my_army.iter() {
                if let Some(closest) = secondary_targets
                    .iter()
                    .filter(|t| u.can_attack_unit(t))
                    .closest(u)
                {
                    u.order_attack(Target::Pos(closest.position()), false);
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
                if u.distance(target) > u.ground_range() + u.speed() {
                    u.order_attack(Target::Pos(target), false);
                }
            }
        }
        // Bring back my queens
        if !self.defending {
            for queen in bot.units.my.units.iter().ready().of_type(UnitTypeId::Queen).filter(|u| u.is_attacking()) {
                if let Some(closest_hall) = bot.units.my.townhalls.closest(queen) {
                    queen.order_move_to(Target::Pos(closest_hall.position()), false);
                }
            }
        }
    }

    fn order_units(&mut self, bot: &mut Bot, bot_info: &mut BotInfo) {
        let min_queens = bot.units.my.townhalls.len() + 2;
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
        let roaches = bot.counter().all().count(UnitTypeId::Roach)
            + (bot.vespene / 2 / bot.game_data.units[&UnitTypeId::Roach].vespene_cost) as usize;
        let hydras = bot.counter().all().count(UnitTypeId::Hydralisk)
            + (bot.vespene / 2 / bot.game_data.units[&UnitTypeId::Hydralisk].vespene_cost) as usize;
        let new_amount = roaches.min(hydras);
        bot_info.build_queue.push(
            Command::new_unit(UnitTypeId::Hydralisk, new_amount),
            false,
            35,
        );
        bot_info
            .build_queue
            .push(Command::new_unit(UnitTypeId::Roach, new_amount), false, 35);

        bot_info
            .build_queue
            .push(Command::new_unit(UnitTypeId::Overseer, 1), false, 1);
    }

    fn order_upgrades(&self, bot: &mut Bot, bot_info: &mut BotInfo) {
        if bot.counter().all().count(UnitTypeId::Zergling) > 6 {
            bot_info.build_queue.push(
                Command::new_upgrade(UpgradeId::Zerglingmovementspeed),
                false,
                50,
            );
        }
        if bot.counter().all().count(UnitTypeId::Zergling) > 30 {
            bot_info.build_queue.push(
                Command::new_upgrade(UpgradeId::Zerglingattackspeed),
                false,
                50,
            );
        }
        if !bot.is_ordered_upgrade(UpgradeId::Zerglingmovementspeed)
            && !bot.has_upgrade(UpgradeId::Zerglingmovementspeed)
        {
            return;
        }
        if bot.counter().all().count(UnitTypeId::Overseer) > 0 {
            bot_info
                .build_queue
                .push(Command::new_upgrade(UpgradeId::Overlordspeed), false, 50);
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
                50,
            );
            bot_info.build_queue.push(
                Command::new_upgrade(UpgradeId::EvolveMuscularAugments),
                false,
                50,
            );
        }
        if bot.counter().all().count(UnitTypeId::Zergling) > 5 && bot.vespene > 150 {
            bot_info.build_queue.push(
                Command::new_upgrade(UpgradeId::ZergMeleeWeaponsLevel1),
                false,
                50,
            );
            bot_info.build_queue.push(
                Command::new_upgrade(UpgradeId::ZergGroundArmorsLevel1),
                false,
                50,
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
        if bot.counter().all().count(UnitTypeId::Roach) > 0 && bot.vespene > 150 {
            bot_info.build_queue.push(
                Command::new_upgrade(UpgradeId::ZergMissileWeaponsLevel1),
                false,
                50,
            );
        }
        if bot.counter().all().count(UnitTypeId::Roach) > 5 && bot.vespene > 250 {
            bot_info.build_queue.push(
                Command::new_upgrade(UpgradeId::ZergMissileWeaponsLevel2),
                false,
                60,
            );
        }
        if bot.vespene > 250 {
            bot_info.build_queue.push(
                Command::new_upgrade(UpgradeId::ZergMeleeWeaponsLevel3),
                false,
                70,
            );
            bot_info.build_queue.push(
                Command::new_upgrade(UpgradeId::ZergGroundArmorsLevel3),
                false,
                70,
            );
            bot_info.build_queue.push(
                Command::new_upgrade(UpgradeId::ZergMissileWeaponsLevel3),
                false,
                70,
            );
            bot_info.build_queue.push(
                Command::new_upgrade(UpgradeId::Zerglingattackspeed),
                false,
                70,
            );
        }
    }
}

trait UnitOrderCheck {
    fn order_move_to(&self, target: Target, queue: bool);
    fn order_attack(&self, target: Target, queue: bool);
}

impl UnitOrderCheck for Unit {
    fn order_move_to(&self, target: Target, queue: bool) {
        if queue {
            self.move_to(target, queue);
        } else {
            match (self.target(), target) {
                (Target::Pos(current_pos), Target::Pos(wanted_pos)) => {
                    if current_pos.distance(wanted_pos) > 0.1f32 {
                        self.move_to(target, queue);
                    }
                },
                (Target::Tag(current_tag), Target::Tag(wanted_tag)) => {
                    if current_tag != wanted_tag {
                        self.move_to(target, queue);
                    }
                }
                (_, _) => {
                    self.move_to(target, queue);
                }
            }
        }
    }

    fn order_attack(&self, target: Target, queue: bool) {
        if queue {
            self.attack(target, queue);
        } else {
            match (self.target(), target) {
                (Target::Pos(current_pos), Target::Pos(wanted_pos)) => {
                    if current_pos.distance(wanted_pos) > 0.1f32 {
                        self.attack(target, queue);
                    }
                },
                (Target::Tag(current_tag), Target::Tag(wanted_tag)) => {
                    if current_tag != wanted_tag {
                        self.attack(target, queue);
                    }
                }
                (_, _) => {
                    self.attack(target, queue);
                }
            }
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
        self.order_upgrades(bot, bot_info);
        self.order_units(bot, bot_info);
        self.scout(bot);
        self.micro(bot);
    }
}
