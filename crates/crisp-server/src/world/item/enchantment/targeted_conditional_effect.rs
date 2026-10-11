use crisp_protocol::nbt::Nbt;

use crate::core::holder::Holder;
use crate::util::codec::{self, DataResult};
use crate::world::level::storage::loot::loot_context::LootContext;
use crate::world::level::storage::loot::predicates::loot_item_condition::LootItemCondition;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EnchantmentTarget {
    Attacker,
    DamagingEntity,
    Victim,
}

impl EnchantmentTarget {
    fn decode(tag: &Nbt) -> DataResult<EnchantmentTarget> {
        codec::string_enum(tag, &[("attacker", EnchantmentTarget::Attacker), ("damaging_entity", EnchantmentTarget::DamagingEntity), ("victim", EnchantmentTarget::Victim)])
    }

    fn decode_non_damage(tag: &Nbt) -> DataResult<EnchantmentTarget> {
        codec::string_enum(tag, &[("attacker", EnchantmentTarget::Attacker), ("victim", EnchantmentTarget::Victim)])
    }
}

pub struct TargetedConditionalEffect<T> {
    pub enchanted: EnchantmentTarget,
    pub affected: EnchantmentTarget,
    pub effect: T,
    pub requirements: Option<Holder<LootItemCondition>>,
}

impl<T> TargetedConditionalEffect<T> {
    pub fn decode(tag: &Nbt, effect: impl FnOnce(&Nbt) -> DataResult<T>) -> DataResult<TargetedConditionalEffect<T>> {
        Ok(TargetedConditionalEffect {
            enchanted: codec::decode_field(tag, "enchanted", EnchantmentTarget::decode)?,
            affected: codec::decode_field(tag, "affected", EnchantmentTarget::decode)?,
            effect: codec::decode_field(tag, "effect", effect)?,
            requirements: codec::decode_optional(tag, "requirements", LootItemCondition::decode_holder)?,
        })
    }

    pub fn decode_equipment_drops(tag: &Nbt, effect: impl FnOnce(&Nbt) -> DataResult<T>) -> DataResult<TargetedConditionalEffect<T>> {
        Ok(TargetedConditionalEffect {
            enchanted: codec::decode_field(tag, "enchanted", EnchantmentTarget::decode_non_damage)?,
            affected: EnchantmentTarget::Victim,
            effect: codec::decode_field(tag, "effect", effect)?,
            requirements: codec::decode_optional(tag, "requirements", LootItemCondition::decode_holder)?,
        })
    }

    pub fn matches(&self, context: &mut LootContext) -> bool {
        self.requirements.as_ref().is_none_or(|requirements| requirements.value().test(context))
    }
}
