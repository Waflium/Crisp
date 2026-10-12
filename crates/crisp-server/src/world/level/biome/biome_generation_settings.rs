use std::sync::{LazyLock, OnceLock};

use crisp_core::hash_map::FastSet;
use serde_json::Value;

use crate::world::level::chunk::Biome;
use crate::world::level::levelgen::placement::placed_feature::{self, placed_feature_index};

pub struct BiomeGenerationSettings {
    features: Vec<Vec<u32>>,
    feature_set: FastSet<u32>,
    bone_meal_features: OnceLock<Vec<u32>>,
}

static SETTINGS: LazyLock<Vec<BiomeGenerationSettings>> = LazyLock::new(|| {
    let registry = crisp_data::SyncedRegistry::get("minecraft:worldgen/biome");
    registry
        .entries
        .iter()
        .map(|name| {
            let source = crisp_data::BIOMES.iter().find(|(n, _)| n == name).map(|(_, s)| *s).expect("biome data");
            let json: Value = serde_json::from_str(source).expect("biome json");
            let features: Vec<Vec<u32>> = json["features"]
                .as_array()
                .map(|steps| {
                    steps
                        .iter()
                        .map(|step| {
                            step.as_array()
                                .into_iter()
                                .flatten()
                                .map(|f| f.as_str().and_then(placed_feature_index).unwrap_or_else(|| panic!("{name} has an unknown placed feature {f}")))
                                .collect()
                        })
                        .collect()
                })
                .unwrap_or_default();
            let feature_set = features.iter().flatten().copied().collect();
            BiomeGenerationSettings { features, feature_set, bone_meal_features: OnceLock::new() }
        })
        .collect()
});

pub fn get(biome: Biome) -> &'static BiomeGenerationSettings {
    &SETTINGS[biome.0 as usize]
}

impl BiomeGenerationSettings {
    pub fn new(features: Vec<Vec<u32>>) -> BiomeGenerationSettings {
        let feature_set = features.iter().flatten().copied().collect();
        BiomeGenerationSettings { features, feature_set, bone_meal_features: OnceLock::new() }
    }

    pub fn features(&self) -> &[Vec<u32>] {
        &self.features
    }

    pub fn get_bone_meal_features(&self) -> &[u32] {
        self.bone_meal_features.get_or_init(|| {
            let can_spawn_from_bone_meal = crisp_data::tag("minecraft:worldgen/feature", "minecraft:can_spawn_from_bone_meal");
            let mut features = Vec::new();
            for &feature in self.features.iter().flatten() {
                placed_feature::get(feature).get_features(&mut features);
            }
            features.retain(|&feature| can_spawn_from_bone_meal.contains(&(feature as i32)));
            features
        })
    }

    pub fn has_feature(&self, feature: u32) -> bool {
        self.feature_set.contains(&feature)
    }
}
