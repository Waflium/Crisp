use crisp_protocol::nbt::Nbt;

use crate::advancements::predicates::enchantment_predicate::EnchantmentPredicate;
use crate::util::codec::{self, DataResult};
use crate::world::item::component::DataComponentType;
use crate::world::item::component::values::ItemEnchantments;

#[derive(Debug)]
pub struct EnchantmentsPredicate {
    component_type: DataComponentType,
    enchantments: Vec<EnchantmentPredicate>,
}

impl EnchantmentsPredicate {
    pub fn decode(component_type: DataComponentType, tag: &Nbt) -> DataResult<EnchantmentsPredicate> {
        Ok(EnchantmentsPredicate { component_type, enchantments: codec::list_of(tag, EnchantmentPredicate::decode)? })
    }

    pub fn component_type(&self) -> DataComponentType {
        self.component_type
    }

    pub fn matches(&self, value: &Nbt) -> bool {
        let applied_enchantments = ItemEnchantments::from_nbt(value);
        self.enchantments.iter().all(|enchantment| enchantment.contained_in(&applied_enchantments))
    }
}
