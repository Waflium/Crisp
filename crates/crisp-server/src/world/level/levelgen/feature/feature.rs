use std::sync::LazyLock;

use crisp_core::BlockPos;
use serde_json::Value;

use super::ore_feature::OreFeature;
use super::scattered_ore_feature::ScatteredOreFeature;
use crate::server::level::world_gen_region::WorldGenRegion;
use crate::world::level::levelgen::blockpredicates::block_predicate::resource;
use crate::world::level::levelgen::worldgen_random::WorldgenRandom;

pub enum Feature {
    Ore(OreFeature),
    ScatteredOre(ScatteredOreFeature),
    NotImplemented(String),
}

static FEATURES: LazyLock<Vec<Feature>> = LazyLock::new(|| {
    crisp_data::FEATURES
        .iter()
        .map(|(name, json)| {
            let json: Value = serde_json::from_str(json).expect("feature json");
            Feature::parse(&json).unwrap_or_else(|e| panic!("feature {name}: {e}"))
        })
        .collect()
});

pub fn feature_index(name: &str) -> Option<u32> {
    let name = resource(name);
    crisp_data::FEATURES.iter().position(|(n, _)| *n == name).map(|i| i as u32)
}

impl Feature {
    pub fn parse(json: &Value) -> Result<Feature, String> {
        let ty = json.get("type").and_then(Value::as_str).ok_or("feature without type")?;
        Ok(match ty.strip_prefix("minecraft:").unwrap_or(ty) {
            "ore" => Feature::Ore(OreFeature::parse(json)?),
            "scattered_ore" => Feature::ScatteredOre(ScatteredOreFeature::parse(json)?),
            other => Feature::NotImplemented(other.to_owned()),
        })
    }

    pub fn place(&self, level: &mut WorldGenRegion, random: &mut WorldgenRandom, origin: BlockPos) -> bool {
        match self {
            Feature::Ore(feature) => feature.place(level, random, origin),
            Feature::ScatteredOre(feature) => feature.place(level, random, origin),
            Feature::NotImplemented(_) => false,
        }
    }
}

pub enum FeatureHolder {
    Reference(u32),
    Direct(Box<Feature>),
}

impl FeatureHolder {
    pub fn parse(json: &Value) -> Result<FeatureHolder, String> {
        match json {
            Value::String(name) => feature_index(name).map(FeatureHolder::Reference).ok_or_else(|| format!("unknown feature {name}")),
            other => Ok(FeatureHolder::Direct(Box::new(Feature::parse(other)?))),
        }
    }

    pub fn value(&self) -> &Feature {
        match self {
            FeatureHolder::Reference(index) => &FEATURES[*index as usize],
            FeatureHolder::Direct(feature) => feature,
        }
    }
}
