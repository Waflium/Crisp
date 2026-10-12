use std::sync::{LazyLock, Mutex};

use serde_json::Value;

use super::placement_modifier::PlacementModifier;
use crate::world::level::levelgen::blockpredicates::block_predicate::resource;
use crate::world::level::levelgen::feature::feature::{Feature, FeatureHolder};

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

static INLINE_PLACED_FEATURES: Mutex<Vec<&'static PlacedFeature>> = Mutex::new(Vec::new());

pub fn inline_placed(feature: Feature) -> u32 {
    let placed: &'static PlacedFeature = Box::leak(Box::new(PlacedFeature { feature: FeatureHolder::Direct(Box::new(feature)), placement: Vec::new() }));
    let mut inline = INLINE_PLACED_FEATURES.lock().unwrap();
    inline.push(placed);
    (PLACED_FEATURES.len() + inline.len() - 1) as u32
}

pub fn get(index: u32) -> &'static PlacedFeature {
    match PLACED_FEATURES.get(index as usize) {
        Some(feature) => feature,
        None => INLINE_PLACED_FEATURES.lock().unwrap()[index as usize - PLACED_FEATURES.len()],
    }
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
