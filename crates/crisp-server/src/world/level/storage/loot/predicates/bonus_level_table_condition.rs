use crisp_core::random::RandomSource;
use crisp_protocol::nbt::Nbt;

use crate::util::codec::{self, DataResult};
use crate::world::item::enchantment::enchantment::Enchantment;
use crate::world::item::enchantment::enchantment_helper::get_item_enchantment_level;
use crate::world::level::storage::loot::loot_context::LootContext;

pub struct BonusLevelTableCondition {
    enchantment: String,
    values: Vec<f32>,
}

impl BonusLevelTableCondition {
    pub fn decode(tag: &Nbt) -> DataResult<BonusLevelTableCondition> {
        let values = codec::decode_field(tag, "chances", |t| codec::list_of(t, codec::float))?;
        if values.is_empty() {
            return Err("List must have contents".into());
        }
        Ok(BonusLevelTableCondition { enchantment: codec::decode_field(tag, "enchantment", Enchantment::decode_holder)?, values })
    }

    pub fn test(&self, context: &mut LootContext) -> bool {
        let level = context.params.tool.map_or(0, |tool| get_item_enchantment_level(&self.enchantment, tool));
        let chance = self.values[(level.max(0) as usize).min(self.values.len() - 1)];
        context.random().next_float() < chance
    }
}
