use crisp_core::AABB;
use crisp_data::EquipmentSlot;

use super::block_source::BlockSource;
use super::default_dispense_item_behavior;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;
use crate::world::item::component::values::Equippable;
use crate::world::item::component::{self as dc};

pub fn execute(level: &mut ServerLevel, source: &BlockSource, mut dispensed: ItemStack) -> ItemStack {
    if dispense_equipment(level, source, &mut dispensed) { dispensed } else { default_dispense_item_behavior::execute(level, source, dispensed) }
}

fn equippable(item_stack: &ItemStack) -> Option<Equippable> {
    item_stack.get(dc::EQUIPPABLE).map(Equippable::from_nbt)
}

fn can_dispenser_equip_into_slot(level: &ServerLevel, id: EntityId) -> bool {
    level.mob(id).is_none_or(|mob| mob.can_pick_up_loot)
}

fn can_equip_with_dispenser(level: &ServerLevel, id: EntityId, item_stack: &ItemStack) -> bool {
    if !level.is_alive(id) || level.is_spectator(id) {
        return false;
    }
    let Some(equippable) = equippable(item_stack).filter(|equippable| equippable.dispensable) else { return false };
    let Some(slot) = EquipmentSlot::by_name(&equippable.slot) else { return false };
    let entity_type = level.entity_ref(id).map_or(-1, |e| e.entity_type);
    let can_be_equipped_by = equippable.allowed_entities.as_ref().is_none_or(|allowed| entity_type >= 0 && allowed.contains("minecraft:entity_type", crisp_data::registry::ENTITY_TYPE[entity_type as usize], entity_type));
    can_be_equipped_by && level.item_by_slot(id, slot).is_none_or(|s| s.is_empty()) && can_dispenser_equip_into_slot(level, id)
}

fn get_equipment_slot_for_item(item_stack: &ItemStack) -> EquipmentSlot {
    equippable(item_stack).and_then(|equippable| EquipmentSlot::by_name(&equippable.slot)).unwrap_or(EquipmentSlot::Mainhand)
}

pub fn dispense_equipment(level: &mut ServerLevel, source: &BlockSource, dispensed: &mut ItemStack) -> bool {
    let pos = source.pos.relative(source.facing());
    let area = AABB::new(pos.x as f64, pos.y as f64, pos.z as f64, (pos.x + 1) as f64, (pos.y + 1) as f64, (pos.z + 1) as f64);
    let Some(target) = level.get_entities(None, &area).into_iter().find(|&id| level.is_living(id) && can_equip_with_dispenser(level, id, dispensed)) else {
        return false;
    };
    let slot = get_equipment_slot_for_item(dispensed);
    let equip = dispensed.split(1);
    level.living_set_item_slot(target, slot, equip);
    if let Some(mob) = level.mob_mut(target) {
        mob.drop_chances[slot as usize] = 2.0;
        mob.persistence_required = true;
    }
    true
}
