use crisp_protocol::nbt::Nbt;

use crate::util::codec::{self, DataResult};
use crate::world::attribute::Value;
use crate::world::attribute::environment_attribute::EnvironmentAttributeInfo;
use crate::world::level::storage::loot::loot_context::LootContext;

pub struct EnvironmentAttributeCheck {
    attribute: &'static EnvironmentAttributeInfo,
    value: Value,
}

impl EnvironmentAttributeCheck {
    pub fn decode(tag: &Nbt) -> DataResult<EnvironmentAttributeCheck> {
        let name = codec::decode_field(tag, "attribute", codec::identifier)?;
        let attribute = EnvironmentAttributeInfo::by_name(&name).ok_or_else(|| format!("Unknown registry key: {name}"))?;
        let value = codec::decode_field(tag, "value", |t| attribute.parse_value(&codec::to_json(t)))?;
        Ok(EnvironmentAttributeCheck { attribute, value })
    }

    pub fn test(&self, context: &mut LootContext) -> bool {
        self.value == context.environment_attribute_value(self.attribute)
    }
}
