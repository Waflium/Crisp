use std::marker::PhantomData;
use std::sync::LazyLock;

use serde_json::Value as Json;

use super::environment_attributes::ATTRIBUTES;
use super::{AttributeRange, AttributeType, Value};

pub struct EnvironmentAttribute<T> {
    id: usize,
    _value: PhantomData<fn() -> T>,
}

impl<T> Clone for EnvironmentAttribute<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for EnvironmentAttribute<T> {}

impl<T> EnvironmentAttribute<T> {
    pub const fn new(id: usize) -> EnvironmentAttribute<T> {
        EnvironmentAttribute { id, _value: PhantomData }
    }

    pub fn id(self) -> usize {
        self.id
    }

    pub fn info(self) -> &'static EnvironmentAttributeInfo {
        &ATTRIBUTES[self.id]
    }
}

pub struct EnvironmentAttributeInfo {
    pub id: usize,
    pub name: &'static str,
    pub ty: AttributeType,
    default_value: fn() -> Value,
    pub value_range: AttributeRange,
    pub is_syncable: bool,
    pub is_positional: bool,
    pub is_spatially_interpolated: bool,
    pub full_resolution_biomes: bool,
}

static DEFAULTS: LazyLock<Vec<Value>> = LazyLock::new(|| ATTRIBUTES.iter().map(|a| (a.default_value)()).collect());

impl EnvironmentAttributeInfo {
    pub(super) const fn builder(id: usize, name: &'static str, ty: AttributeType, default_value: fn() -> Value) -> EnvironmentAttributeInfo {
        EnvironmentAttributeInfo {
            id,
            name,
            ty,
            default_value,
            value_range: AttributeRange::Any,
            is_syncable: false,
            is_positional: true,
            is_spatially_interpolated: false,
            full_resolution_biomes: false,
        }
    }

    pub(super) const fn value_range(self, value_range: AttributeRange) -> Self {
        EnvironmentAttributeInfo { value_range, ..self }
    }

    pub(super) const fn syncable(self) -> Self {
        EnvironmentAttributeInfo { is_syncable: true, ..self }
    }

    pub(super) const fn not_positional(self) -> Self {
        EnvironmentAttributeInfo { is_positional: false, ..self }
    }

    pub(super) const fn spatially_interpolated(self) -> Self {
        EnvironmentAttributeInfo { is_spatially_interpolated: true, ..self }
    }

    pub(super) const fn full_resolution_biomes(self) -> Self {
        EnvironmentAttributeInfo { full_resolution_biomes: true, ..self }
    }

    pub fn by_name(name: &str) -> Option<&'static EnvironmentAttributeInfo> {
        ATTRIBUTES.iter().find(|a| a.name == name)
    }

    pub fn default_value(&self) -> &'static Value {
        &DEFAULTS[self.id]
    }

    pub fn parse_value(&self, json: &Json) -> Result<Value, String> {
        let value = self.ty.parse_value(json)?;
        self.value_range.validate(&value)?;
        Ok(value)
    }

    pub fn sanitize_value(&self, value: Value) -> Value {
        self.value_range.sanitize(value)
    }
}
