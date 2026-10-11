use crisp_data::EquipmentSlot;

use super::{EntityId, LevelEntity};
use crate::server::level::ServerLevel;
use crate::world::item::{ItemStack, inventory_slot};

fn get_equipment_slot(slot: i32) -> Option<EquipmentSlot> {
    Some(match slot {
        103 => EquipmentSlot::Head,
        102 => EquipmentSlot::Chest,
        101 => EquipmentSlot::Legs,
        100 => EquipmentSlot::Feet,
        98 => EquipmentSlot::Mainhand,
        99 => EquipmentSlot::Offhand,
        105 => EquipmentSlot::Body,
        106 => EquipmentSlot::Saddle,
        _ => return None,
    })
}

impl ServerLevel {
    pub fn get_slot(&self, id: EntityId, slot: i32) -> Option<ItemStack> {
        if let Some(player) = self.players.get(&id) {
            if slot == 499 {
                return Some(player.carried.clone().unwrap_or_else(ItemStack::empty));
            }
            let craft_slot = slot - 500;
            if (0..4).contains(&craft_slot) {
                return Some(player.inventory.slots[1 + craft_slot as usize].clone().unwrap_or_else(ItemStack::empty));
            }
            if (0..36).contains(&slot) {
                return Some(player.inventory.slots[inventory_slot(slot as usize)].clone().unwrap_or_else(ItemStack::empty));
            }
            if (0..27).contains(&(slot - 200)) {
                return Some(ItemStack::empty());
            }
        }
        if self.is_living(id) {
            return get_equipment_slot(slot).map(|equipment_slot| self.item_by_slot(id, equipment_slot).unwrap_or_else(ItemStack::empty));
        }
        match self.entities.get(&id) {
            Some(LevelEntity::Item(item)) if slot == 0 => Some(item.item.clone()),
            _ => None,
        }
    }
}
