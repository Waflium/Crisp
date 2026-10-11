use crisp_core::random::RandomSource;
use crisp_protocol::nbt::Nbt;

use crate::util::codec::{self, DataResult};
use crate::world::item::enchantment::enchantment::Enchantment;
use crate::world::item::enchantment::level_based_value::LevelBasedValue;
use crate::world::level::storage::loot::loot_context::LootContext;

pub struct LootItemRandomChanceWithEnchantedBonusCondition {
    unenchanted_chance: f32,
    enchanted_chance: LevelBasedValue,
    enchantment: String,
}

impl LootItemRandomChanceWithEnchantedBonusCondition {
    pub fn decode(tag: &Nbt) -> DataResult<LootItemRandomChanceWithEnchantedBonusCondition> {
        let unenchanted_chance = codec::decode_field(tag, "unenchanted_chance", codec::float)?;
        if !(0.0..=1.0).contains(&unenchanted_chance) {
            return Err(format!("Value {unenchanted_chance} outside of range [0.0:1.0]"));
        }
        Ok(LootItemRandomChanceWithEnchantedBonusCondition {
            unenchanted_chance,
            enchanted_chance: codec::decode_field(tag, "enchanted_chance", LevelBasedValue::decode)?,
            enchantment: codec::decode_field(tag, "enchantment", Enchantment::decode_holder)?,
        })
    }

    pub fn test(&self, context: &mut LootContext) -> bool {
        let enchantment_level = match context.params.attacking_entity {
            Some(killer) if context.params.level.is_living(killer) => context.params.level.get_enchantment_level(&self.enchantment, killer),
            _ => 0,
        };
        let chance = if enchantment_level > 0 { self.enchanted_chance.calculate(enchantment_level) } else { self.unenchanted_chance };
        context.random().next_float() < chance
    }
}
