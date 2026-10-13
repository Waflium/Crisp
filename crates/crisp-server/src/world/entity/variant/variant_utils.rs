use std::collections::HashMap;

use crisp_core::BlockPos;
use crisp_core::random::RandomSource;
use crisp_data::SyncedRegistry;
use serde_json::Value;

use super::biome_check::BiomeCheck;
use super::priority_provider::{self, Selector};
use crate::server::level::ServerLevel;

pub struct SpawnPrioritySelectors {
    selectors: Vec<(i32, Selector)>,
}

impl SpawnPrioritySelectors {
    pub fn load(registry: &str, variants: &[(&str, &str)]) -> SpawnPrioritySelectors {
        let json: HashMap<&str, Value> = variants.iter().map(|&(id, json)| (id, serde_json::from_str(json).expect("vanilla JSON"))).collect();
        let mut selectors = Vec::new();
        for (id, name) in SyncedRegistry::get(registry).entries.iter().enumerate() {
            for selector in json[name]["spawn_conditions"].as_array().into_iter().flatten() {
                let condition = selector.get("condition").map(BiomeCheck::from_json);
                selectors.push((id as i32, Selector { condition, priority: selector["priority"].as_i64().unwrap_or(0) as i32 }));
            }
        }
        SpawnPrioritySelectors { selectors }
    }
}

impl ServerLevel {
    pub fn select_variant_to_spawn(&mut self, pos: BlockPos, variants: &SpawnPrioritySelectors) -> Option<i32> {
        let biome = self.get_biome(pos).0 as i32;
        priority_provider::pick(&variants.selectors, &mut self.random, biome)
    }

    pub fn pick_random_sound_variant(&mut self, registry: &str) -> i32 {
        self.random.next_int_bounded(SyncedRegistry::get(registry).entries.len() as i32)
    }
}

pub fn read_variant(registry: &str, name: &str) -> Option<i32> {
    let name = if name.contains(':') { name.to_string() } else { format!("minecraft:{name}") };
    SyncedRegistry::get(registry).id_of(&name)
}

pub fn variant_name(registry: &str, id: i32) -> &'static str {
    SyncedRegistry::get(registry).entries[id as usize]
}
