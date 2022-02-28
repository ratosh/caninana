use log::debug;
use rust_sc2::bot::Bot;
use rust_sc2::prelude::*;

use crate::command_queue::Command;
use crate::command_queue::Command::*;
use crate::utils::*;
use crate::*;

#[derive(Default)]
pub struct ProductionManager {}

impl ProductionManager {
    const REQUIREMENT_QUEUE_PRIORITY: usize = 100_000;

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

    fn produce_units(&self, bot: &mut Bot, bot_state: &mut BotState) {
        bot_state.build_queue.check_completion(bot);
        for command in bot_state.build_queue.into_iter() {
            match command {
                UnitCommand {
                    unit_type,
                    wanted_amount,
                    save_resources,
                } => {
                    self.produce(bot, bot_state, unit_type, wanted_amount, save_resources);
                }
                UpgradeCommand {
                    upgrade,
                    save_resources,
                } => {
                    self.upgrade(bot, bot_state, upgrade, save_resources);
                }
            }
        }
    }

    // TODO: Check if we have tech to produce (order it if we don't)
    // TODO: Check if we have building to produce (order it if we don't)
    fn produce(
        &self,
        bot: &mut Bot,
        bot_state: &mut BotState,
        unit_type: UnitTypeId,
        wanted_amount: usize,
        save_resources: bool,
    ) {
        if bot.counter().all().count(unit_type) >= wanted_amount {
            return;
        } else if !bot.can_afford(unit_type, true) {
            if save_resources {
                bot.subtract_resources(unit_type, true);
                bot.units.my.larvas.pop();
            }
            return;
        }
        if self.missing_unit_requirements(bot, bot_state, unit_type) {
            return;
        }
        let upgrade_ability = unit_type.morph_ability();
        if upgrade_ability.is_some() {
            self.morph_upgrade(bot, bot_state, unit_type, save_resources);
        } else if unit_type.is_structure() {
            self.build(bot, bot_state, unit_type, wanted_amount);
        } else {
            let current_amount = bot.counter().all().count(unit_type);
            for _ in current_amount..wanted_amount {
                if !bot.can_afford(unit_type, true) {
                    break;
                }
                self.produce_unit(bot, bot_state, unit_type, save_resources);
            }
        }
    }

    fn missing_unit_requirements(
        &self,
        bot: &Bot,
        bot_state: &mut BotState,
        unit_type: UnitTypeId,
    ) -> bool {
        let has_requirement = unit_type.has_requirement(bot);
        if has_requirement {
            return false;
        }

        if let Some(requirement) = unit_type.building_requirements().first() {
            if !self.missing_unit_requirements(bot, bot_state, *requirement) {
                bot_state.build_queue.push(
                    Command::new_unit(*requirement, 1, true),
                    false,
                    Self::REQUIREMENT_QUEUE_PRIORITY,
                );
            }
        }
        true
    }

    fn missing_upgrade_requirements(
        &self,
        bot: &Bot,
        bot_state: &mut BotState,
        upgrade_id: UpgradeId,
    ) -> bool {
        let has_requirement = upgrade_id.has_requirement(bot);
        if has_requirement {
            return false;
        }

        if let Some(requirement) = upgrade_id.building_requirements().first() {
            if !self.missing_unit_requirements(bot, bot_state, *requirement) {
                bot_state
                    .build_queue
                    .push(Command::new_unit(*requirement, 1, true), false, 100);
            }
        }
        true
    }

    // TODO: Queens can be produced on multiple structures (Different types of hatcheries)
    fn produce_unit(
        &self,
        bot: &mut Bot,
        bot_state: &mut BotState,
        unit_type: UnitTypeId,
        save_resources: bool,
    ) {
        let produced_on = unit_type.produced_on();
        if produced_on.contains(&UnitTypeId::Larva) {
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
            .of_types(&produced_on)
            .idle()
            .first()
        {
            debug!("training a {:?} at {:?}", unit_type, produced_on);
            train_at.train(unit_type, false);
            bot.subtract_resources(unit_type, true);
        } else {
            debug!("No building to create, pushing one to the queue");
            bot_state.build_queue.push(
                Command::new_unit(*produced_on.first().unwrap(), 1, save_resources),
                false,
                Self::REQUIREMENT_QUEUE_PRIORITY,
            );
        }
    }

    fn upgrade(
        &self,
        bot: &mut Bot,
        bot_state: &mut BotState,
        upgrade: UpgradeId,
        save_resources: bool,
    ) {
        let produced_on = upgrade.produced_on();
        if bot.is_ordered_upgrade(upgrade) {
            return;
        }
        if bot.can_afford_upgrade(upgrade) {
            if self.missing_upgrade_requirements(bot, bot_state, upgrade) {
                return;
            }
            if let Some(building) = bot
                .units
                .my
                .structures
                .of_types(&produced_on)
                .idle()
                .first()
            {
                building.research(upgrade, false);
                bot.subtract_upgrade_cost(upgrade);
            } else {
                bot_state.build_queue.push(
                    Command::new_unit(*produced_on.first().unwrap(), 1, save_resources),
                    false,
                    Self::REQUIREMENT_QUEUE_PRIORITY,
                );
            }
        } else if save_resources {
            bot.subtract_upgrade_cost(upgrade);
        }
    }

    fn morph_upgrade(
        &self,
        bot: &mut Bot,
        bot_state: &mut BotState,
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
        if let Some(unit) = bot
            .units
            .my
            .all
            .of_types(&produced_on)
            .closest(bot.start_location)
        {
            unit.use_ability(upgrade_ability.unwrap(), false);
            bot.subtract_resources(unit_type, false);
        } else {
            bot_state.build_queue.push(
                Command::new_unit(*produced_on.first().unwrap(), 1, save_resources),
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

    fn build(
        &self,
        bot: &mut Bot,
        bot_state: &BotState,
        unit_type: UnitTypeId,
        wanted_amount: usize,
    ) {
        debug!("Trying to build {:?} {:?}", unit_type, wanted_amount);
        if unit_type.is_structure() {
            if bot.race_values.gas == unit_type || bot.race_values.rich_gas == unit_type {
                self.build_gas(bot);
            } else if bot.race_values.start_townhall == unit_type {
                self.build_expansion(bot, bot_state, unit_type);
            } else if unit_type.is_static_defense() {
                self.build_static_defense(bot, unit_type);
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

    fn build_expansion(&self, bot: &mut Bot, _bot_state: &BotState, unit_type: UnitTypeId) {
        if bot.counter().ordered().count(unit_type) > 0 {
            return;
        }
        if let Some(expansion_location) = bot
            .free_expansions()
            .filter(|e| bot.pathing_distance(bot.start_location, e.loc).is_some())
            .map(|e| e.loc)
            .closest(
                bot.units
                    .my
                    .townhalls
                    .center()
                    .unwrap_or(bot.start_location),
            )
        {
            if let Some(builder) = self.get_builder(bot, expansion_location) {
                builder.build(unit_type, expansion_location, false);
                bot.subtract_resources(bot.race_values.gas, false);
            } else {
                debug!("No builder");
            }
        } else {
            debug!("No expansion location?");
        }
    }

    fn build_static_defense(&self, bot: &mut Bot, unit_type: UnitTypeId) {
        let defenses = bot
            .units
            .my
            .all
            .filter(|unit| unit.type_id().is_static_defense());
        let defenseless_halls = bot
            .units
            .my
            .townhalls
            .filter(|u| defenses.in_range(u, 11f32).is_empty());
        let defense_towards = bot.start_center;
        if let Some(townhall) = defenseless_halls.iter().closest(defense_towards) {
            let placement_position = townhall
                .position()
                .towards(defense_towards, townhall.radius() + 1f32);
            if let Some(builder) = self.get_builder(bot, placement_position) {
                let options = PlacementOptions {
                    max_distance: 3,
                    step: 1,
                    random: false,
                    addon: false,
                };
                if let Some(placement) = bot.find_placement(unit_type, placement_position, options)
                {
                    builder.build(unit_type, placement, false);
                    bot.subtract_resources(unit_type, false);
                }
            }
        } else {
            debug!("No defenseless townhall");
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

impl AIComponent for ProductionManager {
    fn process(&mut self, bot: &mut Bot, bot_state: &mut BotState) {
        self.produce_units(bot, bot_state);
        self.cancel_buildings(bot);
    }
}
