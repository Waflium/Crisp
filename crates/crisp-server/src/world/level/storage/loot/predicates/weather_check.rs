use crisp_protocol::nbt::Nbt;

use crate::util::codec::{self, DataResult};
use crate::world::level::storage::loot::loot_context::LootContext;

pub struct WeatherCheck {
    is_raining: Option<bool>,
    is_thundering: Option<bool>,
}

impl WeatherCheck {
    pub fn decode(tag: &Nbt) -> DataResult<WeatherCheck> {
        Ok(WeatherCheck {
            is_raining: codec::decode_optional(tag, "raining", codec::bool_value)?,
            is_thundering: codec::decode_optional(tag, "thundering", codec::bool_value)?,
        })
    }

    pub fn test(&self, context: &mut LootContext) -> bool {
        let level = context.level();
        if self.is_raining.is_some_and(|raining| raining != level.is_raining()) {
            return false;
        }
        self.is_thundering.is_none_or(|thundering| thundering == level.is_thundering())
    }
}
