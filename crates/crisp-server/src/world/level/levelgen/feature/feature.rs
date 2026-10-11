use std::sync::LazyLock;

use crisp_core::BlockPos;
use crisp_core::random::RandomSource;
use serde_json::Value;

use super::ore_feature::OreFeature;
use super::random_boolean_selector_feature::RandomBooleanSelectorFeature;
use super::random_selector_feature::RandomSelectorFeature;
use super::scattered_ore_feature::ScatteredOreFeature;
use super::simple_random_selector_feature::SimpleRandomSelectorFeature;
use super::tree_feature::TreeFeature;
use super::weighted_random_selector_feature::WeightedRandomSelectorFeature;
use crate::server::level::world_gen_region::WorldGenRegion;
use crate::world::level::levelgen::blockpredicates::block_predicate::resource;

pub enum Feature {
    Tree(Box<TreeFeature>),
    Ore(OreFeature),
    ScatteredOre(ScatteredOreFeature),
    RandomSelector(RandomSelectorFeature),
    SimpleRandomSelector(SimpleRandomSelectorFeature),
    RandomBooleanSelector(Box<RandomBooleanSelectorFeature>),
    WeightedRandomSelector(WeightedRandomSelectorFeature),
    NotImplemented,
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
            "tree" => Feature::Tree(Box::new(TreeFeature::parse(json)?)),
            "ore" => Feature::Ore(OreFeature::parse(json)?),
            "scattered_ore" => Feature::ScatteredOre(ScatteredOreFeature::parse(json)?),
            "random_selector" => Feature::RandomSelector(RandomSelectorFeature::parse(json)?),
            "simple_random_selector" => Feature::SimpleRandomSelector(SimpleRandomSelectorFeature::parse(json)?),
            "random_boolean_selector" => Feature::RandomBooleanSelector(Box::new(RandomBooleanSelectorFeature::parse(json)?)),
            "weighted_random_selector" => Feature::WeightedRandomSelector(WeightedRandomSelectorFeature::parse(json)?),
            _ => Feature::NotImplemented,
        })
    }

    pub fn place<R: RandomSource>(&self, level: &mut WorldGenRegion, random: &mut R, origin: BlockPos) -> bool {
        match self {
            Feature::Tree(feature) => feature.place(level, random, origin),
            Feature::Ore(feature) => feature.place(level, random, origin),
            Feature::ScatteredOre(feature) => feature.place(level, random, origin),
            Feature::RandomSelector(feature) => feature.place(level, random, origin),
            Feature::SimpleRandomSelector(feature) => feature.place(level, random, origin),
            Feature::RandomBooleanSelector(feature) => feature.place(level, random, origin),
            Feature::WeightedRandomSelector(feature) => feature.place(level, random, origin),
            Feature::NotImplemented => false,
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
