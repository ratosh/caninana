use std::collections::HashMap;

use log::debug;
use rust_sc2::bot::Bot;
use rust_sc2::prelude::*;
use rust_sc2::Event::UnitDestroyed;

use crate::{AIComponent, BotState};

#[derive(Default)]
pub struct UnitsCache {
    pub cache: HashMap<u64, CacheEntry>,
}

impl UnitsCache {
    const FOG_AREA_CACHE_TIME: f32 = 120f32;
    const VISIBLE_AREA_CACHE_TIME: f32 = 10f32;

    pub fn units(&self) -> Units {
        let mut result = Units::new();
        for unit in self.cache.values() {
            result.push(unit.clone().unit);
        }
        result
    }

    pub fn destroy_unit(&mut self, tag: u64) {
        if self.cache.contains_key(&tag) {
            debug!("Unit [{tag:?}] destroyed")
        }
        self.cache.remove(&tag);
    }

    fn check_unit_cache(&mut self, bot: &Bot) {
        for unit in bot.units.enemy.all.iter() {
            if let Some(cached) = bot.units.cached.all.get(unit.tag()) {
                self.cache
                    .insert(unit.tag(), CacheEntry::new(cached.clone(), bot.time));
            } else {
                self.cache
                    .insert(unit.tag(), CacheEntry::new(unit.clone(), bot.time));
            }
        }
        self.cache.retain(|_, value| {
            if bot.is_visible(value.unit.position()) && !value.unit.is_burrowed() {
                value.last_seen + Self::VISIBLE_AREA_CACHE_TIME > bot.time
            } else {
                value.last_seen + Self::FOG_AREA_CACHE_TIME > bot.time
            }
        });
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