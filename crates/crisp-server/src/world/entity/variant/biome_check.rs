use crisp_data::SyncedRegistry;
use serde_json::Value;

pub enum BiomeCheck {
    Tag(String),
    Direct(Vec<String>),
}

impl BiomeCheck {
    pub fn from_json(condition: &Value) -> BiomeCheck {
        match &condition["biomes"] {
            Value::String(biomes) => match biomes.strip_prefix('#') {
                Some(tag) => BiomeCheck::Tag(tag.to_string()),
                None => BiomeCheck::Direct(vec![biomes.clone()]),
            },
            Value::Array(biomes) => BiomeCheck::Direct(biomes.iter().filter_map(Value::as_str).map(str::to_string).collect()),
            _ => BiomeCheck::Direct(Vec::new()),
        }
    }

    pub fn test(&self, biome: i32) -> bool {
        match self {
            BiomeCheck::Tag(tag) => crisp_data::tag("minecraft:worldgen/biome", tag).contains(&biome),
            BiomeCheck::Direct(biomes) => {
                let registry = SyncedRegistry::get("minecraft:worldgen/biome");
                biomes.iter().any(|name| registry.id_of(name) == Some(biome))
            }
        }
    }
}
