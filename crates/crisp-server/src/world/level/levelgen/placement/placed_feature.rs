use std::sync::LazyLock;

use serde_json::Value;

use super::placement_modifier::PlacementModifier;
use crate::world::level::levelgen::blockpredicates::block_predicate::resource;
use crate::world::level::levelgen::feature::feature::FeatureHolder;

pub struct PlacedFeature {
    pub feature: FeatureHolder,
    pub placement: Vec<PlacementModifier>,
}

static PLACED_FEATURES: LazyLock<Vec<PlacedFeature>> = LazyLock::new(|| {
    crisp_data::PLACED_FEATURES
        .iter()
        .map(|(name, json)| {
            let json: Value = serde_json::from_str(json).expect("placed feature json");
            PlacedFeature::parse(&json).unwrap_or_else(|e| panic!("placed feature {name}: {e}"))
        })
        .collect()
});

pub fn placed_feature_index(name: &str) -> Option<u32> {
    let name = resource(name);
    crisp_data::PLACED_FEATURES.iter().position(|(n, _)| *n == name).map(|i| i as u32)
}

pub fn get(index: u32) -> &'static PlacedFeature {
    &PLACED_FEATURES[index as usize]
}

impl PlacedFeature {
    pub fn parse(json: &Value) -> Result<PlacedFeature, String> {
        Ok(PlacedFeature {
            feature: FeatureHolder::parse(json.get("feature").ok_or("missing feature")?)?,
            placement: json.get("placement").and_then(Value::as_array).ok_or("missing placement")?.iter().map(PlacementModifier::parse).collect::<Result<_, _>>()?,
        })
    }
}

pub enum PlacedFeatureHolder {
    Reference(u32),
    Direct(Box<PlacedFeature>),
}

pub fn parse_placed_feature_set(json: &Value) -> Result<Vec<PlacedFeatureHolder>, String> {
    let features = match json {
        Value::Array(list) => list.iter().map(PlacedFeatureHolder::parse).collect::<Result<Vec<_>, _>>()?,
        other => vec![PlacedFeatureHolder::parse(other)?],
    };
    if features.is_empty() {
        return Err("features must not be empty".into());
    }
    Ok(features)
}

impl PlacedFeatureHolder {
    pub fn parse(json: &Value) -> Result<PlacedFeatureHolder, String> {
        match json {
            Value::String(name) => placed_feature_index(name).map(PlacedFeatureHolder::Reference).ok_or_else(|| format!("unknown placed feature {name}")),
            other => Ok(PlacedFeatureHolder::Direct(Box::new(PlacedFeature::parse(other)?))),
        }
    }

    pub fn value(&self) -> &PlacedFeature {
        match self {
            PlacedFeatureHolder::Reference(index) => get(*index),
            PlacedFeatureHolder::Direct(feature) => feature,
        }
    }
}
