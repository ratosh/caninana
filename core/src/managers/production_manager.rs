use log::debug;
use rust_sc2::bot::Bot;
use rust_sc2::prelude::*;

use crate::command_queue::Command;
use crate::command_queue::Command::*;
use crate::{BotInfo, Manager};

#[derive(Default)]
pub struct ProductionManager {
    last_loop: u32,
}

impl ProductionManager {
    const REQUIREMENT_QUEUE_PRIORITY: usize = 100_000;
    const PROCESS_DELAY: u32 = 20;

    fn cancel_buildings(&self, bot: &mut Bot) {
        for structure in bot
            .units
            .my
            .structures
            .filter(|u| {
                u.is_attacked()
                    && !u.is_ready()
                    && u.health_percentage().unwrap_or_default() < 0.1f32
            })
            .iter()
        {
            structure.cancel_building(false);
        }
    }

    fn produce_units(&self, bot: &mut Bot, bot_info: &mut BotInfo) {
        bot_info.build_queue.check_completion(bot);
        for command in bot_info.build_queue.into_iter() {
            match command {
                UnitCommand {
                    unit_type,
                    wanted_amount,
                    save_resources,
                } => {
                    self.produce(bot, bot_info, unit_type, wanted_amount, save_resources);
                }
                UpgradeCommand {
                    upgrade,
                    save_resources,
                } => {
                    self.upgrade(bot, bot_info, upgrade, save_resources);
                }
            }
        }
    }

    // TODO: Check if we have tech to produce (order it if we don't)
    // TODO: Check if we have building to produce (order it if we don't)
    fn produce(
        &self,
        bot: &mut Bot,
        bot_info: &mut BotInfo,
        unit_type: UnitTypeId,
        wanted_amount: usize,
        save_resources: bool,
    ) {
        if !bot.can_afford(unit_type, true) {
            if save_resources {
                bot.subtract_resources(unit_type, true);
            }
            return;
        } else if bot.counter().all().count(unit_type) >= wanted_amount {
            return;
        }
        if let Some(requirement) = unit_type.building_requirement() {
            if bot.counter().all().count(requirement) == 0 {
                bot_info.build_queue.push(
                    Command::new_unit(requirement, 1, save_resources),
                    false,
                    Self::REQUIREMENT_QUEUE_PRIORITY,
                );
                return;
            } else if bot.counter().ordered().count(requirement) > 0 {
                return;
            }
        }
        let upgrade_ability = unit_type.morph_ability();
        if upgrade_ability.is_some() {
            self.morph_upgrade(bot, bot_info, unit_type, save_resources);
        } else if unit_type.is_structure() {
            self.build(bot, unit_type, wanted_amount);
        } else {
            let current_amount = bot.counter().all().count(unit_type);
            for _ in current_amount..wanted_amount {
                if !bot.can_afford(unit_type, true) {
                    break;
                }
                self.produce_unit(bot, bot_info, unit_type, save_resources);
            }
        }
    }

    // TODO: Queens can be produced on multiple structures (Different types of hatcheries)
    fn produce_unit(
        &self,
        bot: &mut Bot,
        bot_info: &mut BotInfo,
        unit_type: UnitTypeId,
        save_resources: bool,
    ) {
        let produced_on = unit_type.produced_on();
        if produced_on == UnitTypeId::Larva {
            if let Some(larva) = bot.units.my.larvas.pop() {
                debug!("training a {:?} at {:?}", unit_type, produced_on);
                larva.train(unit_type, false);
                bot.subtract_resources(unit_type, true);
            }
        } else if let Some(train_at) = bot
            .units
            .my
            .structures
            .ready()
            .of_type(produced_on)
            .idle()
            .first()
        {
            debug!("training a {:?} at {:?}", unit_type, produced_on);
            train_at.train(unit_type, false);
            bot.subtract_resources(unit_type, true);
        } else if produced_on.is_structure()
            && bot.units.my.structures.of_type(produced_on).is_empty()
        {
            debug!("No building to create, pushing one to the queue");
            bot_info.build_queue.push(
                Command::new_unit(produced_on, 1, save_resources),
                false,
                Self::REQUIREMENT_QUEUE_PRIORITY,
            );
        }
    }

    fn upgrade(
        &self,
        bot: &mut Bot,
        bot_info: &mut BotInfo,
        upgrade: UpgradeId,
        save_resources: bool,
    ) {
        let produced_on = upgrade.produced_on();
        if bot.can_afford_upgrade(upgrade) {
            if let Some(requirement) = upgrade.building_requirement() {
                if bot.counter().count(requirement) == 0 {
                    bot_info.build_queue.push(
                        Command::new_unit(requirement, 1, save_resources),
                        false,
                        Self::REQUIREMENT_QUEUE_PRIORITY,
                    );
                    return;
                }
            }
            if produced_on.is_structure() {
                if let Some(building) = bot.units.my.structures.of_type(produced_on).idle().first()
                {
                    building.research(upgrade, false);
                    bot.subtract_upgrade_cost(upgrade);
                } else {
                    bot_info.build_queue.push(
                        Command::new_unit(produced_on, 1, save_resources),
                        false,
                        Self::REQUIREMENT_QUEUE_PRIORITY,
                    );
                }
            }
        }
    }

    fn morph_upgrade(
        &self,
        bot: &mut Bot,
        bot_info: &mut BotInfo,
        unit_type: UnitTypeId,
        save_resources: bool,
    ) {
        let upgrade_ability = unit_type.morph_ability();
        if upgrade_ability.is_none() {
            return;
        }
        let produced_on = unit_type.produced_on();
        debug!(
            "Morphing a {:?} from {:?} using {:?}",
            unit_type, produced_on, upgrade_ability
        );
        if let Some(unit) = bot.units.my.all.of_type(produced_on).idle().first() {
            unit.use_ability(upgrade_ability.unwrap(), false);
            bot.subtract_resources(unit_type, false);
        } else {
            bot_info.build_queue.push(
                Command::new_unit(produced_on, 1, save_resources),
                false,
                Self::REQUIREMENT_QUEUE_PRIORITY,
            );
        }
    }

    fn get_builder(&self, bot: &Bot, pos: Point2) -> Option<Unit> {
        let result = bot
            .units
            .my
            .workers
            .iter()
            .filter(|u| !(u.is_constructing() || u.is_returning() || u.is_carrying_resource()))
            .closest(pos);
        result.cloned()
    }

    fn build(&self, bot: &mut Bot, unit_type: UnitTypeId, wanted_amount: usize) {
        debug!("Trying to build {:?} {:?}", unit_type, wanted_amount);
        if unit_type.is_structure() {
            if bot.race_values.gas == unit_type || bot.race_values.rich_gas == unit_type {
                self.build_gas(bot);
            } else if bot.race_values.start_townhall == unit_type {
                self.build_expansion(bot, unit_type);
            } else if let Some(location) = bot.find_placement(
                unit_type,
                bot.units
                    .my
                    .townhalls
                    .closest(bot.start_location)
                    .map_or(bot.start_location, |f| f.position())
                    .towards(bot.game_info.map_center, 8f32),
                PlacementOptions {
                    max_distance: 20,
                    step: 2,
                    random: false,
                    addon: false,
                },
            ) {
                debug!("Placing a {:?} at {:?}", unit_type, location);
                // TODO: improve default building placement
                if let Some(builder) = self.get_builder(bot, location) {
                    builder.build(unit_type, location, false);
                    bot.subtract_resources(unit_type, false);
                } else {
                    debug!("Can't find a builder");
                }
            }
        }
    }

    fn build_expansion(&self, bot: &mut Bot, unit_type: UnitTypeId) {
        if let Some(expansion_location) = bot.free_expansions().map(|e| e.loc).closest(
            bot.units
                .my
                .townhalls
                .center()
                .unwrap_or(bot.start_location),
        ) {
            if let Some(builder) = self.get_builder(bot, expansion_location) {
                let options = PlacementOptions {
                    max_distance: 2,
                    step: 1,
                    random: false,
                    addon: false,
                };
                if let Some(placement) = bot.find_placement(unit_type, expansion_location, options)
                {
                    builder.build(unit_type, placement, false);
                    bot.subtract_resources(bot.race_values.gas, false);
                }
            }
        } else {
            debug!("No neutral expansion?");
        }
    }

    fn build_gas(&self, bot: &mut Bot) {
        let mut geysers = Units::new();
        for owned_expansion in bot.owned_expansions() {
            if let Some(geyser) = bot.find_gas_placement(owned_expansion.loc) {
                geysers.push(geyser);
            }
        }
        if let Some(geyser) = geysers.iter().closest(bot.start_location) {
            if let Some(builder) = self.get_builder(bot, geyser.position()) {
                builder.build_gas(geyser.tag(), false);
                bot.subtract_resources(bot.race_values.gas, false);
            }
        }
    }
}

// TODO: Check if all this info could prob be retrieved from game_info.
trait ProducedOn {
    fn produced_on(&self) -> UnitTypeId;
}

trait MorphUpgrade {
    fn morph_ability(&self) -> Option<AbilityId>;
}

trait BuildingRequirement {
    fn building_requirement(&self) -> Option<UnitTypeId>;
}

impl ProducedOn for UnitTypeId {
    fn produced_on(&self) -> UnitTypeId {
        match *self {
            UnitTypeId::Queen | UnitTypeId::Lair => UnitTypeId::Hatchery,
            UnitTypeId::Hive => UnitTypeId::Lair,
            UnitTypeId::Baneling => UnitTypeId::Zergling,
            UnitTypeId::Overseer => UnitTypeId::Overlord,
            _ => UnitTypeId::Larva,
        }
    }
}

impl MorphUpgrade for UnitTypeId {
    fn morph_ability(&self) -> Option<AbilityId> {
        match *self {
            UnitTypeId::Lair => Some(AbilityId::UpgradeToLairLair),
            UnitTypeId::Hive => Some(AbilityId::UpgradeToHiveHive),
            UnitTypeId::Baneling => Some(AbilityId::MorphZerglingToBanelingBaneling),
            UnitTypeId::Overseer => Some(AbilityId::MorphOverseer),
            _ => None,
        }
    }
}

impl BuildingRequirement for UnitTypeId {
    fn building_requirement(&self) -> Option<UnitTypeId> {
        match *self {
            // Units
            UnitTypeId::Queen => Some(UnitTypeId::SpawningPool),
            UnitTypeId::Zergling => Some(UnitTypeId::SpawningPool),
            UnitTypeId::Baneling => Some(UnitTypeId::BanelingNest),
            UnitTypeId::Roach => Some(UnitTypeId::RoachWarren),
            UnitTypeId::Hydralisk => Some(UnitTypeId::HydraliskDen),
            UnitTypeId::HydraliskDen => Some(UnitTypeId::Lair),
            UnitTypeId::Overseer => Some(UnitTypeId::Lair),
            UnitTypeId::Ultralisk => Some(UnitTypeId::UltraliskCavern),
            UnitTypeId::Corruptor => Some(UnitTypeId::Spire),

            // Buildings
            UnitTypeId::Lair => Some(UnitTypeId::SpawningPool),
            UnitTypeId::Hive => Some(UnitTypeId::InfestationPit),
            UnitTypeId::Spire => Some(UnitTypeId::Lair),
            UnitTypeId::UltraliskCavern => Some(UnitTypeId::Hive),
            _ => None,
        }
    }
}

impl ProducedOn for UpgradeId {
    fn produced_on(&self) -> UnitTypeId {
        match *self {
            UpgradeId::Zerglingattackspeed | UpgradeId::Zerglingmovementspeed => {
                UnitTypeId::SpawningPool
            }
            UpgradeId::CentrificalHooks => UnitTypeId::BanelingNest,
            UpgradeId::GlialReconstitution => UnitTypeId::RoachWarren,
            UpgradeId::EvolveGroovedSpines | UpgradeId::EvolveMuscularAugments => {
                UnitTypeId::HydraliskDen
            }
            UpgradeId::ChitinousPlating | UpgradeId::AnabolicSynthesis => {
                UnitTypeId::UltraliskCavern
            }
            UpgradeId::Overlordspeed => UnitTypeId::Hatchery,
            UpgradeId::ZergGroundArmorsLevel1
            | UpgradeId::ZergGroundArmorsLevel2
            | UpgradeId::ZergGroundArmorsLevel3
            | UpgradeId::ZergMissileWeaponsLevel1
            | UpgradeId::ZergMissileWeaponsLevel2
            | UpgradeId::ZergMissileWeaponsLevel3
            | UpgradeId::ZergMeleeWeaponsLevel1
            | UpgradeId::ZergMeleeWeaponsLevel2
            | UpgradeId::ZergMeleeWeaponsLevel3 => UnitTypeId::EvolutionChamber,
            _ => {
                panic!("Idk where to produce {:?}", self);
            }
        }
    }
}

impl BuildingRequirement for UpgradeId {
    fn building_requirement(&self) -> Option<UnitTypeId> {
        match *self {
            UpgradeId::CentrificalHooks
            | UpgradeId::ZergGroundArmorsLevel2
            | UpgradeId::ZergMissileWeaponsLevel2
            | UpgradeId::ZergMeleeWeaponsLevel2 => Some(UnitTypeId::Lair),
            UpgradeId::Zerglingattackspeed
            | UpgradeId::ZergGroundArmorsLevel3
            | UpgradeId::ZergMissileWeaponsLevel3
            | UpgradeId::ZergMeleeWeaponsLevel3 => Some(UnitTypeId::Hive),
            _ => None,
        }
    }
}

impl Manager for ProductionManager {
    fn process(&mut self, bot: &mut Bot, bot_info: &mut BotInfo) {
        let last_loop = self.last_loop;
        let game_loop = bot.state.observation.game_loop();
        if last_loop + Self::PROCESS_DELAY > game_loop {
            return;
        }
        self.last_loop = game_loop;
        self.produce_units(bot, bot_info);
        self.cancel_buildings(bot);
    }
}
