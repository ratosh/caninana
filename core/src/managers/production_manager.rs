use log::debug;
use rust_sc2::bot::Bot;
use rust_sc2::prelude::*;
use std::collections::HashSet;

use crate::command_queue::Command;
use crate::command_queue::Command::*;
use crate::utils::*;
use crate::*;

#[derive(Default)]
pub struct ProductionManager {
    producing: HashSet<u64>,
}

impl ProductionManager {
    const REQUIREMENT_QUEUE_PRIORITY: usize = 100_000;

    fn cancel_buildings(&self, bot: &mut Bot) {
        for structure in bot
            .units
            .my
            .structures
            .filter(|u| {
                u.is_attacked() && !u.is_ready() && u.hits_percentage().unwrap_or_default() < 0.1f32
            })
            .iter()
        {
            structure.cancel_building(false);
        }
    }

    fn produce_units(&mut self, bot: &mut Bot, bot_state: &mut BotState) {
        bot_state.build_queue.check_completion(bot);
        self.producing.clear();
        for element in bot_state.build_queue.into_iter() {
            match element.command {
                UnitCommand {
                    unit_type,
                    wanted_amount,
                    save_resources,
                } => {
                    self.produce(
                        bot,
                        bot_state,
                        unit_type,
                        wanted_amount,
                        save_resources,
                        element.priority,
                    );
                }
                UpgradeCommand {
                    upgrade,
                    save_resources,
                } => {
                    self.upgrade(bot, bot_state, upgrade, save_resources, element.priority);
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
        priority: usize,
    ) {
        if bot.counter().all().count(unit_type) >= wanted_amount {
            return;
        } else if !bot.can_afford(unit_type, true) {
            self.save_unit_resources(bot, bot_state, unit_type, save_resources, true, true);
            return;
        }
        if self.missing_unit_requirements(bot, bot_state, unit_type, save_resources, priority) {
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
                    self.save_unit_resources(bot, bot_state, unit_type, save_resources, true, true);
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
        save_resources: bool,
        priority: usize,
    ) -> bool {
        let has_requirement = unit_type.has_requirement(bot);
        if has_requirement {
            return false;
        }

        if let Some(requirement) = unit_type.building_requirements().first() {
            if !self.missing_unit_requirements(
                bot,
                bot_state,
                *requirement,
                save_resources,
                priority,
            ) {
                bot_state.build_queue.push(
                    Command::new_unit(*requirement, 1, save_resources),
                    false,
                    Self::REQUIREMENT_QUEUE_PRIORITY + priority,
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
        save_resources: bool,
        priority: usize,
    ) -> bool {
        let has_requirement = upgrade_id.has_requirement(bot);
        if has_requirement {
            return false;
        }

        if let Some(requirement) = upgrade_id.building_requirements().first() {
            if !self.missing_unit_requirements(
                bot,
                bot_state,
                *requirement,
                save_resources,
                priority,
            ) {
                bot_state.build_queue.push(
                    Command::new_unit(*requirement, 1, save_resources),
                    false,
                    100,
                );
            }
        }
        true
    }

    fn produce_unit(
        &self,
        bot: &mut Bot,
        bot_state: &mut BotState,
        unit_type: UnitTypeId,
        save_resources: bool,
    ) {
        let larvas = bot.units.my.larvas.clone();
        let larva = if unit_type.is_worker() || !unit_type.is_melee() {
            larvas.first()
        } else {
            larvas.closest(bot.start_location)
        };
        let produced_on = unit_type.produced_on();
        if produced_on.contains(&UnitTypeId::Larva) {
            if let Some(larva) = larva {
                debug!("training a {:?} at {:?}", unit_type, produced_on);
                bot.units.my.larvas.remove(larva.tag());
                larva.train(unit_type, false);
                bot.subtract_resources(unit_type, true);
            }
        } else if let Some(train_at) = bot
            .units
            .my
            .structures
            .ready()
            .of_types(&produced_on)
            .almost_idle()
            .first()
        {
            if self.producing.contains(&train_at.tag()) {
                self.save_unit_resources(bot, bot_state, unit_type, save_resources, true, true);
            } else {
                debug!("training a {:?} at {:?}", unit_type, produced_on);
                train_at.train(unit_type, false);
                bot.subtract_resources(unit_type, true);
            }
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
        &mut self,
        bot: &mut Bot,
        bot_state: &mut BotState,
        upgrade: UpgradeId,
        save_resources: bool,
        priority: usize,
    ) {
        if !bot.can_afford_vespene_upgrade(upgrade) {
            return;
        }
        let produced_on = upgrade.produced_on();
        if bot.is_ordered_upgrade(upgrade) {
            return;
        }
        if bot.can_afford_upgrade(upgrade) {
            if self.missing_upgrade_requirements(bot, bot_state, upgrade, save_resources, priority)
            {
                return;
            }
            if let Some(building) = bot
                .units
                .my
                .structures
                .of_types(&produced_on)
                .almost_idle()
                .first()
            {
                if !self.producing.contains(&building.tag()) {
                    building.research(upgrade, false);
                    self.producing.insert(building.tag());
                }
                bot.subtract_upgrade_cost(upgrade);
            } else if bot.units.my.structures.of_types(&produced_on).is_empty() {
                bot_state.build_queue.push(
                    Command::new_unit(*produced_on.first().unwrap(), 1, save_resources),
                    false,
                    Self::REQUIREMENT_QUEUE_PRIORITY,
                );
            }
        } else if save_resources {
            self.save_upgrade_cost(bot, bot_state, upgrade);
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
                    .towards(bot.game_info.map_center, 4f32),
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

    fn build_expansion(&self, bot: &mut Bot, bot_state: &BotState, unit_type: UnitTypeId) {
        if bot_state.spending_focus == SpendingFocus::Army && bot.minerals < 300 {
            return;
        }
        if !bot
            .units
            .my
            .townhalls
            .filter(|u| u.build_progress() < 0.1f32)
            .is_empty()
        {
            return;
        }
        if let Some(expansion_location) = bot
            .expansions
            .iter()
            .filter(|e| {
                e.alliance.is_neutral()
                    && bot.pathing_distance(bot.start_location, e.loc).is_some()
                    && e.geysers.len() > 1
            })
            .map(|e| e.loc)
            .next()
        {
            if let Some(builder) = self.get_builder(bot, expansion_location) {
                builder.build(unit_type, expansion_location, false);
                bot.subtract_resources(unit_type, false);
            } else {
                debug!("No builder");
            }
        } else {
            debug!("No expansion location?");
        }
    }

    fn build_static_defense(&self, bot: &mut Bot, unit_type: UnitTypeId) {
        let defenses = bot.units.my.all.filter(|unit| unit.type_id() == unit_type);
        let defenseless_halls = bot
            .units
            .my
            .townhalls
            .filter(|u| u.is_ready() && defenses.closer(11f32, u.position()).is_empty());
        if let Some(townhall) = defenseless_halls.iter().closest(bot.start_center) {
            let resources = bot.units.resources.closer(9f32, townhall.position());
            if let Some(defense_center) = resources.center() {
                let multiplier = if unit_type == UnitTypeId::SpineCrawler {
                    -1f32
                } else {
                    1f32
                };
                let placement_position = townhall
                    .position()
                    .towards(defense_center, (townhall.radius() + 1f32) * multiplier);
                if let Some(builder) = self.get_builder(bot, placement_position) {
                    let options = PlacementOptions {
                        max_distance: 3,
                        step: 1,
                        random: false,
                        addon: false,
                    };
                    if let Some(placement) =
                        bot.find_placement(unit_type, placement_position, options)
                    {
                        builder.build(unit_type, placement, false);
                        bot.subtract_resources(unit_type, false);
                    }
                } else {
                    debug!("No builder");
                }
            } else {
                debug!("No defense center");
            }
        } else {
            debug!("No defenseless townhall");
        }
    }

    fn build_gas(&self, bot: &mut Bot) {
        let mut geysers = Units::new();
        for owned_expansion in bot.owned_expansions() {
            if let Some(base_tag) = owned_expansion.base {
                if let Some(base) = bot.units.my.townhalls.get(base_tag) {
                    if base.is_ready() {
                        if let Some(geyser) = bot.find_gas_placement(owned_expansion.loc) {
                            geysers.push(geyser);
                        }
                    }
                }
            }
        }
        if let Some(geyser) = geysers.iter().closest(bot.start_location) {
            if let Some(builder) = self.get_builder(bot, geyser.position()) {
                builder.build_gas(geyser.tag(), false);
                bot.subtract_resources(bot.race_values.gas, false);
            }
        }
    }

    fn save_unit_resources(
        &self,
        bot: &mut Bot,
        bot_state: &BotState,
        unit_type: UnitTypeId,
        save_resources: bool,
        use_supply: bool,
        save_larva: bool,
    ) {
        if !save_resources {
            return;
        }
        if bot_state.spending_focus == SpendingFocus::Army && unit_type.is_structure() {
            return;
        }
        bot.subtract_resources(unit_type, use_supply);
        if unit_type.produced_on().contains(&UnitTypeId::Larva) && save_larva {
            bot.units.my.larvas.pop();
        }
    }

    fn save_upgrade_cost(&self, bot: &mut Bot, _bot_state: &BotState, upgrade_id: UpgradeId) {
        bot.subtract_upgrade_cost(upgrade_id);
    }
}

impl AIComponent for ProductionManager {
    fn process(&mut self, bot: &mut Bot, bot_state: &mut BotState) {
        self.produce_units(bot, bot_state);
        self.cancel_buildings(bot);
    }
}
