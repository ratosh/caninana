use crate::UnwrapOrMax;
use rust_sc2::bot::Bot;
use rust_sc2::prelude::*;

impl Strength for Units {
    fn strength(&self, bot: &Bot) -> f32 {
        self.iter().map(|u| u.strength(bot)).sum()
    }
}

pub trait Strength {
    fn strength(&self, bot: &Bot) -> f32;
}

impl StrengthVs for Units {
    fn strength_vs(&self, bot: &Bot, unit: &Unit) -> f32 {
        self.iter()
            .filter(|f| f.can_attack_unit(unit))
            .map(|u| u.strength(bot))
            .sum()
    }
}

pub trait StrengthVs {
    fn strength_vs(&self, bot: &Bot, unit: &Unit) -> f32;
}

//TODO: Give bonus for units better at one role.
// e.g. thor anti air
impl Strength for Unit {
    fn strength(&self, _: &Bot) -> f32 {
        let multiplier = if self.is_worker() {
            0.1f32
        } else if !self.can_attack() {
            0.5f32
        } else if self.is_structure() {
            1.5f32
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
            ],
            UnitTypeId::Sentry => vec![
                UnitTypeId::BroodLord,
                UnitTypeId::Ultralisk,
            ],
            UnitTypeId::Stalker => vec![UnitTypeId::Zergling],
            UnitTypeId::Immortal => vec![UnitTypeId::Zergling, UnitTypeId::Hydralisk],
            UnitTypeId::Colossus => vec![UnitTypeId::Corruptor],
            UnitTypeId::Phoenix => vec![UnitTypeId::Hydralisk],
            UnitTypeId::VoidRay => vec![UnitTypeId::Hydralisk],
            UnitTypeId::HighTemplar => vec![UnitTypeId::Ultralisk],
            UnitTypeId::DarkTemplar => vec![
                // UnitTypeId::Mutalisk,
                UnitTypeId::BroodLord
            ],
            UnitTypeId::Carrier => vec![UnitTypeId::Hydralisk, UnitTypeId::Corruptor],
            UnitTypeId::Mothership => vec![UnitTypeId::Corruptor],
            UnitTypeId::Oracle => vec![
                UnitTypeId::Hydralisk,
                // UnitTypeId::Mutalisk
            ],
            UnitTypeId::Tempest => vec![UnitTypeId::Corruptor],
            UnitTypeId::Adept => vec![UnitTypeId::Roach],
            UnitTypeId::Disruptor => vec![UnitTypeId::Ultralisk],
            // Race::Terran
            UnitTypeId::Marine => vec![
                // UnitTypeId::Baneling,
                UnitTypeId::Roach,
                UnitTypeId::Ravager,
                UnitTypeId::Ultralisk,
                UnitTypeId::BroodLord,
                // UnitTypeId::LurkerMP,
            ],
            UnitTypeId::Marauder => vec![
                UnitTypeId::Zergling,
                UnitTypeId::Hydralisk,
                // UnitTypeId::Mutalisk,
                UnitTypeId::BroodLord,
            ],
            UnitTypeId::Medivac => vec![UnitTypeId::Hydralisk],
            UnitTypeId::Reaper => vec![UnitTypeId::Ravager],
            UnitTypeId::Ghost => vec![UnitTypeId::Roach, UnitTypeId::Ultralisk],
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
            UnitTypeId::Thor => vec![
                UnitTypeId::Zergling,
                UnitTypeId::Hydralisk,
                UnitTypeId::BroodLord,
            ],
            UnitTypeId::Banshee => vec![
                UnitTypeId::Hydralisk,
                // UnitTypeId::Mutalisk,
                // UnitTypeId::Corruptor
            ],
            UnitTypeId::Viking => vec![UnitTypeId::Hydralisk],
            UnitTypeId::Raven => vec![UnitTypeId::Hydralisk, UnitTypeId::Corruptor],
            UnitTypeId::Battlecruiser => vec![UnitTypeId::Corruptor],
            UnitTypeId::HellionTank => vec![UnitTypeId::Roach],
            UnitTypeId::Liberator => vec![UnitTypeId::Corruptor],
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
                // UnitTypeId::Mutalisk,
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
                UnitTypeId::Hydralisk,
                // UnitTypeId::Mutalisk,
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
    fn produced_on(&self) -> UnitTypeId;
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
    fn building_requirement(&self) -> Option<UnitTypeId>;
}

impl ProducedOn for UnitTypeId {
    fn produced_on(&self) -> UnitTypeId {
        match *self {
            UnitTypeId::Queen | UnitTypeId::Lair => UnitTypeId::Hatchery,
            UnitTypeId::Hive => UnitTypeId::Lair,
            UnitTypeId::Baneling => UnitTypeId::Zergling,
            UnitTypeId::Ravager => UnitTypeId::Roach,
            UnitTypeId::BroodLord => UnitTypeId::Corruptor,
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
            UnitTypeId::Ravager => Some(AbilityId::MorphToRavagerRavager),
            UnitTypeId::Overseer => Some(AbilityId::MorphOverseer),
            UnitTypeId::BroodLord => Some(AbilityId::MorphToBroodLordBroodLord),
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
            UnitTypeId::Ravager => Some(UnitTypeId::RoachWarren),
            UnitTypeId::Hydralisk => Some(UnitTypeId::HydraliskDen),
            UnitTypeId::HydraliskDen => Some(UnitTypeId::Lair),
            UnitTypeId::Mutalisk => Some(UnitTypeId::Spire),
            UnitTypeId::Overseer => Some(UnitTypeId::Lair),
            UnitTypeId::Ultralisk => Some(UnitTypeId::UltraliskCavern),
            UnitTypeId::Corruptor => Some(UnitTypeId::Spire),
            UnitTypeId::BroodLord => Some(UnitTypeId::GreaterSpire),

            // Buildings
            UnitTypeId::Lair => Some(UnitTypeId::SpawningPool),
            UnitTypeId::Hive => Some(UnitTypeId::InfestationPit),
            UnitTypeId::Spire => Some(UnitTypeId::Lair),
            UnitTypeId::UltraliskCavern => Some(UnitTypeId::Hive),
            UnitTypeId::SpineCrawler | UnitTypeId::SporeCrawler => Some(UnitTypeId::SpawningPool),
            _ => None,
        }
    }
}

impl ProducedOn for UpgradeId {
    fn produced_on(&self) -> UnitTypeId {
        match *self {
            UpgradeId::Burrow => UnitTypeId::Hatchery,
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
