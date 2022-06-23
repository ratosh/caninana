use std::collections::HashMap;

use log::debug;
use rust_sc2::bot::Bot;
use rust_sc2::prelude::*;
use rust_sc2::Event::UnitDestroyed;

use crate::utils::DetectionCloseBy;
use crate::{AIComponent, BotState};

#[derive(Default)]
pub struct UnitsCache {
    cache: HashMap<u64, CacheEntry>,
    pub units: Units,
}

impl UnitsCache {
    const FOG_AREA_CACHE_TIME: f32 = 60f32;
    const VISIBLE_AREA_CACHE_TIME: f32 = 10f32;
    const ON_DETECTION_CACHE_TIME: f32 = 1f32;
    const TACTICAL_JUMP_CACHE_TIME: f32 = 4f32;

    pub fn destroy_unit(&mut self, tag: u64) {
        if self.cache.contains_key(&tag) {
            debug!("Unit [{tag:?}] destroyed")
        }
        self.units.remove(tag);
        self.cache.remove(&tag);
    }

    fn check_unit_cache(&mut self, bot: &Bot) {
        for unit in bot.units.enemy.all.iter() {
            let to_be_cached = if let Some(cached) = bot.units.cached.all.get(unit.tag()) {
                cached.clone()
            } else {
                unit.clone()
            };
            self.cache
                .insert(unit.tag(), CacheEntry::new(to_be_cached, bot.time));
        }
        self.cache.retain(|_, value| {
            let reference_time = if value.unit.is_using(AbilityId::EffectTacticalJump) {
                value.last_seen + Self::TACTICAL_JUMP_CACHE_TIME
            } else if bot.detection_close_by(&value.unit, 0f32) {
                value.last_seen + Self::ON_DETECTION_CACHE_TIME
            } else if bot.is_visible(value.unit.position()) {
                value.last_seen + Self::VISIBLE_AREA_CACHE_TIME
            } else {
                value.last_seen + Self::FOG_AREA_CACHE_TIME
            };
            reference_time > bot.time
        });
        self.units.clear();
        for unit in self.cache.values() {
            self.units.push(unit.unit.clone());
        }
    }
}

#[derive(Clone)]
pub struct CacheEntry {
    pub unit: Unit,
    pub last_seen: f32,
}

impl CacheEntry {
    fn new(unit: Unit, time: f32) -> Self {
        Self {
            unit,
            last_seen: time,
        }
    }
}

#[derive(Default)]
pub struct CacheManager {}

impl AIComponent for CacheManager {
    fn process(&mut self, bot: &mut Bot, bot_state: &mut BotState) {
        bot_state.enemy_cache.check_unit_cache(bot);
    }

    fn on_event(&mut self, event: &Event, bot_state: &mut BotState) {
        if let UnitDestroyed(tag, _) = event {
            bot_state.enemy_cache.destroy_unit(*tag);
        }
    }
}
