use serde_json::Value as Json;

use super::environment_attribute::EnvironmentAttributeInfo;
use super::modifier::{AttributeModifier, OperationId};
use super::{EnvironmentAttribute, Value};
use crate::world::level::levelgen::density_function::resource_name;

#[derive(Clone, PartialEq, Debug)]
pub struct Entry {
    pub argument: Value,
    pub modifier: AttributeModifier,
}

impl Entry {
    pub fn apply_modifier(&self, subject: &Value) -> Value {
        self.modifier.apply(subject, &self.argument)
    }

    fn parse(attribute: &EnvironmentAttributeInfo, json: &Json) -> Result<Entry, String> {
        let value_error = match attribute.parse_value(json) {
            Ok(argument) => return Ok(Entry { argument, modifier: AttributeModifier::Override }),
            Err(e) => e,
        };
        let full = || -> Result<Entry, String> {
            let operation = json.get("modifier").and_then(Json::as_str).ok_or("missing modifier")?;
            let modifier = OperationId::by_name(operation)
                .and_then(|op| attribute.ty.modifier(op))
                .ok_or_else(|| format!("unknown modifier {operation} for {}", attribute.name))?;
            let argument = modifier.parse_argument(attribute, json.get("argument").ok_or("missing argument")?)?;
            Ok(Entry { argument, modifier })
        };
        full().map_err(|e| format!("{}: {value_error}; {e}", attribute.name))
    }
}

#[derive(Clone, PartialEq, Debug, Default)]
pub struct EnvironmentAttributeMap {
    entries: Vec<(usize, Entry)>,
}

impl EnvironmentAttributeMap {
    pub fn parse(json: &Json, only_positional: bool) -> Result<EnvironmentAttributeMap, String> {
        let object = json.as_object().ok_or_else(|| format!("not an attribute map: {json}"))?;
        let mut map = EnvironmentAttributeMap::default();
        let mut illegal = Vec::new();
        for (name, value) in object {
            let name = resource_name(name);
            let attribute = EnvironmentAttributeInfo::by_name(&name).ok_or_else(|| format!("unknown environment attribute {name}"))?;
            if !attribute.is_positional {
                illegal.push(name.clone());
            }
            map.put(attribute.id, Entry::parse(attribute, value)?);
        }
        if only_positional && !illegal.is_empty() {
            return Err(format!("The following attributes cannot be positional: {illegal:?}"));
        }
        Ok(map)
    }

    fn put(&mut self, id: usize, entry: Entry) {
        match self.entries.iter_mut().find(|(i, _)| *i == id) {
            Some(slot) => slot.1 = entry,
            None => self.entries.push((id, entry)),
        }
    }

    pub fn modify<T>(mut self, attribute: EnvironmentAttribute<T>, modifier: AttributeModifier, argument: Value) -> Self {
        self.put(attribute.id(), Entry { argument, modifier });
        self
    }

    pub fn set<T>(self, attribute: EnvironmentAttribute<T>, value: Value) -> Self {
        self.modify(attribute, AttributeModifier::Override, value)
    }

    pub fn get(&self, id: usize) -> Option<&Entry> {
        self.entries.iter().find(|(i, _)| *i == id).map(|(_, e)| e)
    }

    pub fn contains(&self, id: usize) -> bool {
        self.get(id).is_some()
    }

    pub fn key_set(&self) -> impl Iterator<Item = usize> + '_ {
        self.entries.iter().map(|(i, _)| *i)
    }

    pub fn entries(&self) -> &[(usize, Entry)] {
        &self.entries
    }
}
