use std::fmt;

use serde_json::Value;

use crate::resources::identifier;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TagEntry {
    pub id: String,
    pub tag: bool,
    pub required: bool,
}

pub trait Lookup<T> {
    fn element(&self, key: &str, required: bool) -> Option<T>;

    fn tag(&self, key: &str) -> Option<Vec<T>>;
}

impl TagEntry {
    pub fn parse(value: &Value) -> Result<TagEntry, String> {
        match value {
            Value::String(name) => tag_or_element_id(name).map(|(id, tag)| TagEntry { id, tag, required: true }),
            Value::Object(object) => {
                let id = match object.get("id") {
                    Some(Value::String(name)) => tag_or_element_id(name)?,
                    Some(other) => return Err(format!("Not a string: {other}")),
                    None => return Err(format!("No key id in MapLike[{value}]")),
                };
                let required = match object.get("required") {
                    None => true,
                    Some(Value::Bool(required)) => *required,
                    Some(other) => return Err(format!("Not a boolean: {other}")),
                };
                Ok(TagEntry { id: id.0, tag: id.1, required })
            }
            other => Err(format!("Not a string: {other}")),
        }
    }

    pub fn build<T>(&self, lookup: &dyn Lookup<T>, output: &mut dyn FnMut(T)) -> bool {
        if self.tag {
            let Some(result) = lookup.tag(&self.id) else { return !self.required };
            result.into_iter().for_each(output);
        } else {
            let Some(result) = lookup.element(&self.id, self.required) else { return !self.required };
            output(result);
        }
        true
    }

    pub fn visit_required_dependencies(&self, output: &mut dyn FnMut(&str)) {
        if self.tag && self.required {
            output(&self.id);
        }
    }

    pub fn visit_optional_dependencies(&self, output: &mut dyn FnMut(&str)) {
        if self.tag && !self.required {
            output(&self.id);
        }
    }
}

fn tag_or_element_id(name: &str) -> Result<(String, bool), String> {
    match name.strip_prefix('#') {
        Some(tag) => identifier::read(tag).map(|id| (id, true)),
        None => identifier::read(name).map(|id| (id, false)),
    }
}

impl fmt::Display for TagEntry {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.tag {
            f.write_str("#")?;
        }
        f.write_str(&self.id)?;
        if !self.required {
            f.write_str("?")?;
        }
        Ok(())
    }
}
