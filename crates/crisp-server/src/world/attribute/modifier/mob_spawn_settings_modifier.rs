use std::sync::Arc;

use crate::world::attribute::Value;
use crate::world::level::biome::mob_spawn_settings::{MobCategory, MobSpawnSettings};

pub fn overlay(first: &Value, second: &Value) -> Value {
    let (Value::MobSpawnSettings(a), Value::MobSpawnSettings(b)) = (first, second) else { panic!("not mob spawn settings: {first:?}, {second:?}") };
    if a.is_empty() {
        return second.clone();
    }
    if b.is_empty() {
        return first.clone();
    }
    let covers_categories = (0..8).all(|i| a.spawns_by_category[i].is_none() || b.spawns_by_category[i].is_some());
    let covers_costs = a.mob_spawn_costs.iter().all(|(e, _)| b.mob_spawn_cost(*e).is_some());
    if covers_categories && covers_costs {
        return second.clone();
    }
    let mut result = MobSpawnSettings::default();
    for category in MobCategory::VALUES {
        let i = category as usize;
        result.spawns_by_category[i] = b.spawns_by_category[i].clone().or_else(|| a.spawns_by_category[i].clone());
    }
    for &(entity_type, cost) in a.mob_spawn_costs.iter().chain(&b.mob_spawn_costs) {
        result.put_cost(entity_type, cost);
    }
    Value::MobSpawnSettings(Arc::new(result))
}
