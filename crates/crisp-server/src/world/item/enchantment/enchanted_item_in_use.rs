use crisp_data::EquipmentSlot;

use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;
use crate::world::item::component as dc;

pub struct EnchantedItemInUse {
    pub item_stack: ItemStack,
    pub in_slot: Option<EquipmentSlot>,
    pub owner: Option<EntityId>,
}

impl EnchantedItemInUse {
    pub fn new(item_stack: ItemStack, in_slot: EquipmentSlot, owner: EntityId) -> EnchantedItemInUse {
        EnchantedItemInUse { item_stack, in_slot: Some(in_slot), owner: Some(owner) }
    }

    pub fn hurt_and_break(&self, level: &mut ServerLevel, amount: i32) {
        if !self.item_stack.has(dc::MAX_DAMAGE) || !self.item_stack.has(dc::DAMAGE) {
            return;
        }
        if let (Some(owner), Some(slot)) = (self.owner, self.in_slot) {
            level.hurt_and_break(owner, slot, amount);
        }
    }
}
