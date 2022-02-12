use rust_sc2::bot::Bot;
use rust_sc2::prelude::{*};

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
                Some(result.iter().map(|d| d.unwrap_or(100f32)).sum())
            }
        } else {
            None
        }
    }
}

pub trait UnitOrderCheck {
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


// TODO: Check if all this info could prob be retrieved from game_info.
pub trait ProducedOn {
    fn produced_on(&self) -> UnitTypeId;
}

pub trait IsStaticDefense {
    fn is_static_defense(&self) -> bool;
}

impl IsStaticDefense for UnitTypeId {
    fn is_static_defense(&self) -> bool {
        matches!(self, UnitTypeId::SpineCrawler | UnitTypeId::SporeCrawler | UnitTypeId::PhotonCannon | UnitTypeId::Bunker)
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