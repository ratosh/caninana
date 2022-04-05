use crate::BotState;
use rust_sc2::bot::Bot;
use rust_sc2::prelude::*;

pub trait Strength {
    fn strength(&self, bot: &Bot) -> f32;
}

impl Strength for Units {
    fn strength(&self, bot: &Bot) -> f32 {
        self.iter().map(|u| u.strength(bot)).sum::<f32>()
            * (1f32 + (self.len() as f32 + 1f32).log(10f32))
    }
}

pub trait DynamicStrength {
    fn dynamic_strength(&self, bot: &Bot) -> f32;
}

impl DynamicStrength for Units {
    fn dynamic_strength(&self, bot: &Bot) -> f32 {
        self.filter(|u| !u.is_structure() || !u.is_close_to_their_base(bot))
            .strength(bot)
    }
}

pub trait CloseToTheirBase {
    fn is_close_to_their_base(&self, bot: &Bot) -> bool;
}

impl CloseToTheirBase for Unit {
    fn is_close_to_their_base(&self, bot: &Bot) -> bool {
        bot.units
            .enemy
            .townhalls
            .closest_distance(self.position())
            .unwrap_or_max()
            < 25f32
    }
}

pub trait CloseToOurBase {
    fn is_close_to_our_base(&self, bot: &Bot) -> bool;
}

impl CloseToOurBase for Unit {
    fn is_close_to_our_base(&self, bot: &Bot) -> bool {
        bot.units
            .my
            .townhalls
            .closest_distance(self.position())
            .unwrap_or_default()
            < 35f32
    }
}

pub trait IsDangerous {
    fn is_dangerous(&self) -> bool;
}

impl IsDangerous for Unit {
    fn is_dangerous(&self) -> bool {
        self.can_attack()
            || SPECIAL_DANGEROUS.contains(&self.type_id())
            || SPECIAL_UNITS.contains(&self.type_id())
    }
}

const SPECIAL_DANGEROUS: [UnitTypeId; 9] = [
    UnitTypeId::Infestor,
    UnitTypeId::InfestorBurrowed,
    UnitTypeId::LurkerMP,
    UnitTypeId::LurkerMPBurrowed,
    UnitTypeId::Disruptor,
    UnitTypeId::Liberator,
    UnitTypeId::LiberatorAG,
    UnitTypeId::WidowMine,
    UnitTypeId::Raven,
];

const SPECIAL_UNITS: [UnitTypeId; 5] = [
    UnitTypeId::Observer,
    UnitTypeId::WarpPrism,
    UnitTypeId::Medivac,
    UnitTypeId::Overlord,
    UnitTypeId::Overseer,
];

//TODO: Give bonus for units better at one role.
// e.g. thor anti air
impl Strength for Unit {
    fn strength(&self, _: &Bot) -> f32 {
        let multiplier = if !self.is_almost_ready() || self.is_hallucination() {
            0.0f32
        } else if self.is_worker() {
            0.2f32
        } else if SPECIAL_DANGEROUS.contains(&self.type_id()) {
            1.0f32
        } else if !self.can_attack() {
            0.0f32
        } else if self.is_structure() {
            1.5f32
        } else if self.is_cloaked() {
            2f32
        } else {
            1f32
        };
        multiplier
            * (self.cost().vespene + self.cost().minerals) as f32
            * self.hits_percentage().unwrap_or(1f32)
    }
}

pub trait CounteredBy {
    fn countered_by(&self) -> Vec<UnitTypeId>;
}

impl CounteredBy for UnitTypeId {
    fn countered_by(&self) -> Vec<UnitTypeId> {
        match self {
            // Race::Protoss
            UnitTypeId::Zealot => vec![
                UnitTypeId::Roach,
                UnitTypeId::Ravager,
                // UnitTypeId::Mutalisk,
                UnitTypeId::BroodLord,
                UnitTypeId::Ultralisk,
            ],
            UnitTypeId::Sentry => vec![
                UnitTypeId::Roach,
                UnitTypeId::Hydralisk,
                UnitTypeId::Ultralisk,
                UnitTypeId::BroodLord,
            ],
            UnitTypeId::Stalker => vec![UnitTypeId::Zergling, UnitTypeId::Hydralisk],
            UnitTypeId::Immortal => vec![
                UnitTypeId::Zergling,
                UnitTypeId::Hydralisk,
                UnitTypeId::BroodLord,
            ],
            UnitTypeId::Colossus => vec![UnitTypeId::Corruptor],
            UnitTypeId::Phoenix => vec![UnitTypeId::Hydralisk],
            UnitTypeId::VoidRay => vec![UnitTypeId::Hydralisk, UnitTypeId::Corruptor],
            UnitTypeId::HighTemplar => vec![UnitTypeId::Ultralisk],
            UnitTypeId::DarkTemplar => vec![
                // UnitTypeId::Mutalisk,
                UnitTypeId::BroodLord,
            ],
            UnitTypeId::Carrier => vec![UnitTypeId::Hydralisk, UnitTypeId::Corruptor],
            UnitTypeId::Mothership => vec![UnitTypeId::Corruptor],
            UnitTypeId::Oracle => vec![
                UnitTypeId::Hydralisk,
                // UnitTypeId::Mutalisk
                UnitTypeId::Corruptor,
            ],
            UnitTypeId::Tempest => vec![UnitTypeId::Hydralisk, UnitTypeId::Corruptor],
            UnitTypeId::Adept => vec![
                UnitTypeId::Roach,
                UnitTypeId::Hydralisk,
                UnitTypeId::BroodLord,
            ],
            UnitTypeId::Disruptor => vec![UnitTypeId::Ultralisk],
            // Race::Terran
            UnitTypeId::Marine => vec![
                // UnitTypeId::Baneling,
                UnitTypeId::Roach,
                UnitTypeId::Ravager,
                UnitTypeId::Ultralisk,
                // UnitTypeId::LurkerMP,
            ],
            UnitTypeId::Marauder => vec![
                UnitTypeId::Zergling,
                UnitTypeId::Hydralisk,
                // UnitTypeId::Mutalisk,
            ],
            UnitTypeId::Medivac => vec![UnitTypeId::Hydralisk, UnitTypeId::Corruptor],
            UnitTypeId::Reaper => vec![UnitTypeId::Ravager],
            UnitTypeId::Ghost => vec![UnitTypeId::Zergling],
            UnitTypeId::Hellion => vec![
                UnitTypeId::Roach,
                // UnitTypeId::Mutalisk
            ],
            UnitTypeId::SiegeTank => vec![
                // UnitTypeId::Mutalisk,
                UnitTypeId::BroodLord,
                UnitTypeId::Ravager,
            ],
            UnitTypeId::SiegeTankSieged => vec![
                // UnitTypeId::Mutalisk,
                UnitTypeId::BroodLord,
                UnitTypeId::Ravager,
            ],
            UnitTypeId::Thor => vec![UnitTypeId::Zergling, UnitTypeId::Hydralisk],
            UnitTypeId::Banshee => vec![
                UnitTypeId::Hydralisk,
                // UnitTypeId::Mutalisk,
                UnitTypeId::Corruptor,
            ],
            UnitTypeId::Viking => vec![UnitTypeId::Hydralisk],
            UnitTypeId::Raven => vec![UnitTypeId::Hydralisk, UnitTypeId::Corruptor],
            UnitTypeId::Battlecruiser => vec![UnitTypeId::Corruptor],
            UnitTypeId::Cyclone => vec![UnitTypeId::Zergling],
            UnitTypeId::HellionTank => vec![UnitTypeId::Roach],
            UnitTypeId::Liberator => vec![UnitTypeId::Corruptor],
            // Race::Zerg
            UnitTypeId::Zergling => vec![
                UnitTypeId::Zealot,
                UnitTypeId::Adept,
                UnitTypeId::Sentry,
                UnitTypeId::Marine,
                UnitTypeId::Reaper,
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
                // UnitTypeId::Mutalisk,
                UnitTypeId::Roach,
                UnitTypeId::Ultralisk,
            ],
            UnitTypeId::Roach => vec![
                UnitTypeId::Immortal,
                UnitTypeId::VoidRay,
                UnitTypeId::SiegeTank,
                UnitTypeId::SiegeTankSieged,
                UnitTypeId::Marauder,
                UnitTypeId::Ultralisk,
                // UnitTypeId::Mutalisk,
            ],
            UnitTypeId::Hydralisk => vec![
                UnitTypeId::Sentry,
                UnitTypeId::Colossus,
                UnitTypeId::Hellion,
                UnitTypeId::HellionTank,
                UnitTypeId::SiegeTank,
                UnitTypeId::SiegeTank,
                UnitTypeId::SiegeTankSieged,
                UnitTypeId::Zergling,
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
                UnitTypeId::Phoenix,
                UnitTypeId::Marine,
                UnitTypeId::Viking,
                UnitTypeId::Thor,
            ],
            UnitTypeId::Infestor => vec![
                UnitTypeId::Immortal,
                UnitTypeId::Colossus,
                UnitTypeId::SiegeTank,
                UnitTypeId::SiegeTankSieged,
                UnitTypeId::Ghost,
            ],
            UnitTypeId::Ultralisk => vec![
                UnitTypeId::Immortal,
                UnitTypeId::VoidRay,
                UnitTypeId::Ghost,
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
                UnitTypeId::Hydralisk,
                // UnitTypeId::Mutalisk,
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
            UnitTypeId::PhotonCannon => vec![UnitTypeId::Ravager],
            UnitTypeId::Bunker => vec![UnitTypeId::Ravager],
            _ => vec![],
        }
    }
}

trait RaceFinder {
    fn race(&self, bot: &Bot) -> Race;
}

impl RaceFinder for UnitTypeId {
    fn race(&self, bot: &Bot) -> Race {
        bot.game_data.units[self].race
    }
}

pub trait Supply {
    fn supply(&self) -> u32;
}

impl Supply for Units {
    fn supply(&self) -> u32 {
        self.sum(|f| f.supply_cost()) as u32
    }
}

pub trait Between {
    fn between(self, pos: Point2) -> Point2;
}

impl Between for Point2 {
    fn between(self, other: Self) -> Self {
        (self + other) / 2f32
    }
}

pub trait PathingDistance {
    fn pathing_distance(&self, p1: Point2, p2: Point2) -> Option<f32>;
}

impl PathingDistance for Bot {
    fn pathing_distance(&self, p1: Point2, p2: Point2) -> Option<f32> {
        if let Result::Ok(result) = self.query_pathing(vec![(Target::Pos(p1), p2)]) {
            if result.is_empty() {
                None
            } else {
                Some(result.iter().map(|d| d.unwrap_or_max()).sum())
            }
        } else {
            None
        }
    }
}

pub trait UnitOrderCheck {
    fn order_move_to(&self, target: Target, range: f32, queue: bool);
    fn order_attack(&self, target: Target, queue: bool);
    fn order_gather(&self, target: u64, queue: bool);
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

    fn order_gather(&self, target: u64, queue: bool) {
        let target_tag = Target::Tag(target);
        if should_send_order(self, target_tag, 0.1f32, queue) {
            self.gather(target, false);
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

// TODO: Check if all this info could prob be retrieved from game_info.
pub trait ProducedOn {
    fn produced_on(&self) -> Vec<UnitTypeId>;
}

pub trait IsStaticDefense {
    fn is_static_defense(&self) -> bool;
}

impl IsStaticDefense for UnitTypeId {
    fn is_static_defense(&self) -> bool {
        matches!(
            self,
            UnitTypeId::SpineCrawler
                | UnitTypeId::SporeCrawler
                | UnitTypeId::PhotonCannon
                | UnitTypeId::Bunker
        )
    }
}

pub trait MorphUpgrade {
    fn morph_ability(&self) -> Option<AbilityId>;
}

pub trait BuildingRequirement {
    fn building_requirements(&self) -> Vec<UnitTypeId>;
}

impl ProducedOn for UnitTypeId {
    fn produced_on(&self) -> Vec<UnitTypeId> {
        match *self {
            UnitTypeId::Queen => vec![UnitTypeId::Hatchery, UnitTypeId::Lair, UnitTypeId::Hive],
            UnitTypeId::Lair => vec![UnitTypeId::Hatchery],
            UnitTypeId::Hive => vec![UnitTypeId::Lair],
            UnitTypeId::Baneling => vec![UnitTypeId::Zergling],
            UnitTypeId::Ravager => vec![UnitTypeId::Roach],
            UnitTypeId::BroodLord => vec![UnitTypeId::Corruptor],
            UnitTypeId::Overseer => vec![UnitTypeId::Overlord],
            UnitTypeId::GreaterSpire => vec![UnitTypeId::Spire],
            _ => {
                if self.is_structure() {
                    vec![UnitTypeId::Drone]
                } else {
                    vec![UnitTypeId::Larva]
                }
            }
        }
    }
}

impl MorphUpgrade for UnitTypeId {
    fn morph_ability(&self) -> Option<AbilityId> {
        match *self {
            UnitTypeId::Lair => Some(AbilityId::UpgradeToLairLair),
            UnitTypeId::Hive => Some(AbilityId::UpgradeToHiveHive),
            UnitTypeId::Baneling => Some(AbilityId::MorphZerglingToBanelingBaneling),
            UnitTypeId::Ravager => Some(AbilityId::MorphToRavagerRavager),
            UnitTypeId::Overseer => Some(AbilityId::MorphOverseer),
            UnitTypeId::BroodLord => Some(AbilityId::MorphToBroodLordBroodLord),
            UnitTypeId::GreaterSpire => Some(AbilityId::UpgradeToGreaterSpireGreaterSpire),
            _ => None,
        }
    }
}

impl BuildingRequirement for UnitTypeId {
    fn building_requirements(&self) -> Vec<UnitTypeId> {
        match *self {
            // Units
            UnitTypeId::Queen => vec![UnitTypeId::SpawningPool],
            UnitTypeId::Zergling => vec![UnitTypeId::SpawningPool],
            UnitTypeId::Baneling => vec![UnitTypeId::BanelingNest],
            UnitTypeId::Roach => vec![UnitTypeId::RoachWarren],
            UnitTypeId::Ravager => vec![UnitTypeId::RoachWarren],
            UnitTypeId::Hydralisk => vec![UnitTypeId::HydraliskDen],
            UnitTypeId::Mutalisk => vec![UnitTypeId::Spire, UnitTypeId::GreaterSpire],
            UnitTypeId::Overseer => vec![UnitTypeId::Lair, UnitTypeId::Hive],
            UnitTypeId::Ultralisk => vec![UnitTypeId::UltraliskCavern],
            UnitTypeId::Corruptor => vec![UnitTypeId::Spire, UnitTypeId::GreaterSpire],
            UnitTypeId::BroodLord => vec![UnitTypeId::GreaterSpire],

            // Buildings
            UnitTypeId::Lair => vec![UnitTypeId::SpawningPool],
            UnitTypeId::Hive => vec![UnitTypeId::InfestationPit],
            UnitTypeId::HydraliskDen => vec![UnitTypeId::Lair, UnitTypeId::Hive],
            UnitTypeId::Spire => vec![UnitTypeId::Lair, UnitTypeId::Hive],
            UnitTypeId::GreaterSpire => vec![UnitTypeId::Hive],
            UnitTypeId::UltraliskCavern => vec![UnitTypeId::Hive],
            UnitTypeId::SpineCrawler | UnitTypeId::SporeCrawler => vec![UnitTypeId::SpawningPool],
            _ => vec![],
        }
    }
}

impl ProducedOn for UpgradeId {
    fn produced_on(&self) -> Vec<UnitTypeId> {
        match *self {
            UpgradeId::Burrow => vec![UnitTypeId::Hatchery, UnitTypeId::Lair, UnitTypeId::Hive],
            UpgradeId::Zerglingattackspeed | UpgradeId::Zerglingmovementspeed => {
                vec![UnitTypeId::SpawningPool]
            }
            UpgradeId::CentrificalHooks => vec![UnitTypeId::BanelingNest],
            UpgradeId::GlialReconstitution | UpgradeId::TunnelingClaws => {
                vec![UnitTypeId::RoachWarren]
            }
            UpgradeId::EvolveGroovedSpines | UpgradeId::EvolveMuscularAugments => {
                vec![UnitTypeId::HydraliskDen]
            }
            UpgradeId::ChitinousPlating | UpgradeId::AnabolicSynthesis => {
                vec![UnitTypeId::UltraliskCavern]
            }
            UpgradeId::Overlordspeed => {
                vec![UnitTypeId::Hatchery, UnitTypeId::Lair, UnitTypeId::Hive]
            }
            UpgradeId::ZergGroundArmorsLevel1
            | UpgradeId::ZergGroundArmorsLevel2
            | UpgradeId::ZergGroundArmorsLevel3
            | UpgradeId::ZergMissileWeaponsLevel1
            | UpgradeId::ZergMissileWeaponsLevel2
            | UpgradeId::ZergMissileWeaponsLevel3
            | UpgradeId::ZergMeleeWeaponsLevel1
            | UpgradeId::ZergMeleeWeaponsLevel2
            | UpgradeId::ZergMeleeWeaponsLevel3 => vec![UnitTypeId::EvolutionChamber],
            UpgradeId::ZergFlyerArmorsLevel1
            | UpgradeId::ZergFlyerArmorsLevel2
            | UpgradeId::ZergFlyerArmorsLevel3
            | UpgradeId::ZergFlyerWeaponsLevel1
            | UpgradeId::ZergFlyerWeaponsLevel2
            | UpgradeId::ZergFlyerWeaponsLevel3 => {
                vec![UnitTypeId::Spire, UnitTypeId::GreaterSpire]
            }
            _ => {
                panic!("Idk where to produce {:?}", self);
            }
        }
    }
}

impl BuildingRequirement for UpgradeId {
    fn building_requirements(&self) -> Vec<UnitTypeId> {
        match *self {
            UpgradeId::CentrificalHooks
            | UpgradeId::ZergGroundArmorsLevel2
            | UpgradeId::ZergMissileWeaponsLevel2
            | UpgradeId::ZergMeleeWeaponsLevel2
            | UpgradeId::ZergFlyerArmorsLevel2
            | UpgradeId::ZergFlyerWeaponsLevel2 => vec![UnitTypeId::Lair, UnitTypeId::Hive],
            UpgradeId::Zerglingattackspeed
            | UpgradeId::ZergGroundArmorsLevel3
            | UpgradeId::ZergMissileWeaponsLevel3
            | UpgradeId::ZergMeleeWeaponsLevel3
            | UpgradeId::ZergFlyerArmorsLevel3
            | UpgradeId::ZergFlyerWeaponsLevel3 => vec![UnitTypeId::Hive],
            _ => vec![],
        }
    }
}

pub trait CanAffordVespeneUpgrade {
    fn can_afford_vespene_upgrade(&self, upgrade: UpgradeId) -> bool;
}

impl CanAffordVespeneUpgrade for Bot {
    fn can_afford_vespene_upgrade(&self, upgrade: UpgradeId) -> bool {
        let cost = self.get_upgrade_cost(upgrade);
        self.vespene >= cost.vespene
    }
}

pub trait DetectionCloseBy {
    fn detection_close_by(&self, unit: &Unit, range: f32) -> bool;
}

impl DetectionCloseBy for Bot {
    fn detection_close_by(&self, unit: &Unit, range: f32) -> bool {
        if !self
            .units
            .enemy
            .all
            .filter(|u| u.is_detector() && u.is_closer(u.detect_range() + range, unit))
            .is_empty()
        {
            return true;
        }

        let scans = self
            .state
            .observation
            .raw
            .effects
            .iter()
            .filter(|e| e.id == EffectId::ScannerSweep && e.alliance != unit.alliance())
            .collect::<Vec<_>>();

        for scan in scans {
            for p in &scan.positions {
                if unit.is_closer(range + scan.radius, *p) {
                    return true;
                }
            }
        }
        false
    }
}

pub trait HasRequirement {
    fn has_requirement(&self, bot: &Bot) -> bool;
}

pub trait UnitUpgradeList {
    fn unit_upgrades(&self) -> Vec<UpgradeId>;
}

impl UnitUpgradeList for UnitTypeId {
    fn unit_upgrades(&self) -> Vec<UpgradeId> {
        let mut result = vec![];
        // Weapons check
        match self {
            UnitTypeId::BroodLord => {
                result.push(UpgradeId::ZergMeleeWeaponsLevel1);
                result.push(UpgradeId::ZergMeleeWeaponsLevel2);
                result.push(UpgradeId::ZergMeleeWeaponsLevel3);
                result.push(UpgradeId::ZergFlyerWeaponsLevel1);
                result.push(UpgradeId::ZergFlyerWeaponsLevel2);
                result.push(UpgradeId::ZergFlyerWeaponsLevel3);
            }
            UnitTypeId::Zergling | UnitTypeId::Baneling | UnitTypeId::Ultralisk => {
                result.push(UpgradeId::ZergMeleeWeaponsLevel1);
                result.push(UpgradeId::ZergMeleeWeaponsLevel2);
                result.push(UpgradeId::ZergMeleeWeaponsLevel3);
            }
            UnitTypeId::Roach | UnitTypeId::Ravager | UnitTypeId::Hydralisk => {
                result.push(UpgradeId::ZergMissileWeaponsLevel1);
                result.push(UpgradeId::ZergMissileWeaponsLevel2);
                result.push(UpgradeId::ZergMissileWeaponsLevel3);
            }
            UnitTypeId::Corruptor => {
                result.push(UpgradeId::ZergFlyerWeaponsLevel1);
                result.push(UpgradeId::ZergFlyerWeaponsLevel2);
                result.push(UpgradeId::ZergFlyerWeaponsLevel3);
            }
            _ => {}
        }
        // Defense check
        match self {
            UnitTypeId::Zergling
            | UnitTypeId::Baneling
            | UnitTypeId::Roach
            | UnitTypeId::Ravager
            | UnitTypeId::Hydralisk
            | UnitTypeId::Infestor
            | UnitTypeId::LurkerMP
            | UnitTypeId::SwarmHostMP
            | UnitTypeId::Ultralisk => {
                result.push(UpgradeId::ZergGroundArmorsLevel1);
                result.push(UpgradeId::ZergGroundArmorsLevel2);
                result.push(UpgradeId::ZergGroundArmorsLevel3);
            }
            UnitTypeId::Mutalisk
            | UnitTypeId::Viper
            | UnitTypeId::Corruptor
            | UnitTypeId::BroodLord => {
                result.push(UpgradeId::ZergFlyerArmorsLevel1);
                result.push(UpgradeId::ZergFlyerArmorsLevel2);
                result.push(UpgradeId::ZergFlyerArmorsLevel3);
            }
            _ => {}
        }
        result
    }
}

pub trait UpgradeCounter {
    fn count_upgrades(&self, bot: &Bot) -> usize;
}

impl UpgradeCounter for UnitTypeId {
    fn count_upgrades(&self, bot: &Bot) -> usize {
        let mut counter = 0;
        for upgrade_id in self.unit_upgrades() {
            if bot.has_upgrade(upgrade_id) {
                counter += 1;
            }
        }
        counter
    }
}

impl HasRequirement for UnitTypeId {
    fn has_requirement(&self, bot: &Bot) -> bool {
        for requirement in self.building_requirements() {
            if !bot.units.my.all.ready().of_type(requirement).is_empty() {
                return true;
            }
        }
        self.building_requirements().is_empty()
    }
}

impl HasRequirement for UpgradeId {
    fn has_requirement(&self, bot: &Bot) -> bool {
        for requirement in self.building_requirements() {
            if !bot.units.my.all.ready().of_type(requirement).is_empty() {
                return true;
            }
        }
        self.building_requirements().is_empty()
    }
}

pub trait Center {
    fn center_point(&self) -> Option<Point2>;
}

impl Center for Vec<(usize, usize)> {
    fn center_point(&self) -> Option<Point2> {
        if self.is_empty() {
            None
        } else {
            let (sum, len) = self.iter().fold(((0, 0), 0), |(sum, len), p| {
                ((sum.0 + p.0, sum.1 + p.1), len + 1)
            });
            Some(Point2::from((sum.0 / len, sum.1 / len)))
        }
    }
}

pub trait UnwrapOrMax<T> {
    fn unwrap_or_max(self) -> T;
}

impl UnwrapOrMax<f32> for Option<f32> {
    fn unwrap_or_max(self) -> f32 {
        match self {
            Some(x) => x,
            None => f32::MAX,
        }
    }
}

pub trait MoveTowards {
    fn move_towards(&self, bot: &Bot, bot_state: &BotState, multiplier: f32);
}

impl MoveTowards for Unit {
    fn move_towards(&self, bot: &Bot, bot_state: &BotState, multiplier: f32) {
        let center = bot_state
            .enemy_cache
            .units
            .filter(|t| t.can_attack_unit(self) && self.distance(t.position()) < 16f32)
            .center();
        if let Some(center_point) = center {
            let position = {
                let pos = self
                    .position()
                    .towards(center_point, self.speed() * multiplier);
                if self.is_flying() || bot.is_pathable(pos) {
                    pos
                } else {
                    *self
                        .position()
                        .neighbors8()
                        .iter()
                        .filter(|p| bot.is_pathable(**p))
                        .furthest(center_point)
                        .unwrap_or(&bot.start_location)
                }
            };
            self.order_move_to(Target::Pos(position), 0.5f32, false);
        }
    }
}
