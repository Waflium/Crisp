use crisp_data::Block;
use crisp_protocol::nbt::Nbt;
use serde_json::Value;

use crate::world::level::chunk::Biome;
use crate::world::level::chunk::FlatLevelSource;

const Y_SIZE: i64 = 4064;

#[derive(Clone, Debug, PartialEq)]
pub enum StructureOverrides {
    Tag(String),
    Direct(Vec<String>),
}

impl StructureOverrides {
    pub fn to_nbt(&self) -> Nbt {
        match self {
            StructureOverrides::Tag(tag) => Nbt::String(format!("#{tag}")),
            StructureOverrides::Direct(ids) if ids.len() == 1 => Nbt::String(ids[0].clone()),
            StructureOverrides::Direct(ids) => Nbt::List(ids.iter().map(|id| Nbt::String(id.clone())).collect()),
        }
    }

    fn from_strings<'a>(single: Option<&'a str>, list: Option<Vec<&'a str>>) -> Option<StructureOverrides> {
        let structure_set = |id: &str| crate::resources::identifier::parse(id).ok().filter(|id| crisp_data::STRUCTURE_SETS.iter().any(|(name, _)| name == id));
        match (single, list) {
            (Some(tag), _) if tag.starts_with('#') => crate::resources::identifier::parse(&tag[1..]).ok().map(StructureOverrides::Tag),
            (Some(id), _) => structure_set(id).map(|id| StructureOverrides::Direct(vec![id])),
            (None, Some(ids)) => ids.into_iter().map(structure_set).collect::<Option<Vec<_>>>().map(StructureOverrides::Direct),
            (None, None) => None,
        }
    }

    pub fn from_nbt(nbt: &Nbt) -> Option<StructureOverrides> {
        match nbt {
            Nbt::String(s) => Self::from_strings(Some(s), None),
            Nbt::List(items) => Self::from_strings(None, items.iter().map(Nbt::as_str).collect()),
            _ => None,
        }
    }

    fn from_json(json: &Value) -> Option<StructureOverrides> {
        match json {
            Value::String(s) => Self::from_strings(Some(s), None),
            Value::Array(items) => Self::from_strings(None, items.iter().map(Value::as_str).collect()),
            _ => None,
        }
    }
}

pub struct Parsed {
    pub value: Option<FlatLevelSource>,
    pub error: Option<String>,
}

pub fn parse_settings(json: &Value) -> Parsed {
    let mut errors = Vec::new();
    let Some(object) = json.as_object() else {
        return Parsed { value: None, error: Some(format!("Not a JSON object: {}", gson(json))) };
    };
    let layers = match object.get("layers").filter(|v| !v.is_null()) {
        None => {
            return Parsed { value: None, error: Some(format!("No key layers in MapLike[{}]", gson(json))) };
        }
        Some(Value::Array(elements)) => elements.iter().filter_map(|element| parse_layer(element, &mut errors)).collect::<Vec<_>>(),
        Some(other) => {
            return Parsed { value: None, error: Some(format!("Not a json array: {}", gson(other))) };
        }
    };
    let lakes = optional_boolean(object.get("lakes"), &mut errors);
    let features = optional_boolean(object.get("features"), &mut errors);
    let biome = object.get("biome").and_then(Value::as_str).and_then(|id| crate::resources::identifier::parse(id).ok()).and_then(|id| Biome::by_name(&id));
    let biome = biome.unwrap_or_else(|| {
        error!("Unknown biome, defaulting to plains");
        Biome::by_name("minecraft:plains").expect("plains biome")
    });
    let structure_overrides = object.get("structure_overrides").and_then(StructureOverrides::from_json);
    let total_height: i64 = layers.iter().map(|&(_, height)| height as i64).sum();
    errors.reverse();
    if total_height > Y_SIZE {
        errors.push(format!("Sum of layer heights is > {Y_SIZE}"));
    }
    let value = FlatLevelSource { layers, biome, structure_overrides, lakes, features, generation: Default::default() };
    Parsed { value: Some(value), error: (!errors.is_empty()).then(|| errors.join("; ")) }
}

fn parse_layer(element: &Value, errors: &mut Vec<String>) -> Option<(crisp_data::BlockState, u32)> {
    let Some(object) = element.as_object() else {
        errors.push(format!("Not a JSON object: {}", gson(element)));
        return None;
    };
    let height = match object.get("height").filter(|v| !v.is_null()) {
        None => Err(format!("No key height in MapLike[{}]", gson(element))),
        Some(value) => number_value(value).and_then(|height| {
            if (0..=Y_SIZE as i32).contains(&height) { Ok(height) } else { Err(format!("Value {height} outside of range [0:{Y_SIZE}]")) }
        }),
    };
    let block = match object.get("block").filter(|v| !v.is_null()) {
        None => Err(format!("No key block in MapLike[{}]", gson(element))),
        Some(Value::String(id)) => crate::resources::identifier::read(id).and_then(|id| {
            Block::by_name(&id).ok_or_else(|| format!("Unknown registry key in ResourceKey[minecraft:root / minecraft:block]: {id}"))
        }),
        Some(other) => Err(format!("Not a string: {}", gson(other))),
    };
    match (height, block) {
        (Ok(height), Ok(block)) => Some((block.default_state(), height as u32)),
        (height, block) => {
            errors.extend(height.err());
            errors.extend(block.err());
            None
        }
    }
}

fn number_value(value: &Value) -> Result<i32, String> {
    let Value::Number(number) = value else { return Err(format!("Not a number: {}", gson(value))) };
    Ok(match number.as_i64() {
        Some(n) => n as i32,
        None => number.as_f64().unwrap_or(0.0).trunc() as i64 as i32,
    })
}

fn optional_boolean(value: Option<&Value>, errors: &mut Vec<String>) -> bool {
    match value.filter(|v| !v.is_null()) {
        None => false,
        Some(Value::Bool(b)) => *b,
        Some(other) => {
            errors.push(format!("Not a boolean: {}", gson(other)));
            false
        }
    }
}

fn gson(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_default()
}
