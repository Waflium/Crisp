use crisp_protocol::nbt::Nbt;

use super::criterion_trigger::CriterionTrigger;
use super::simple_criterion_trigger;
use crate::advancements::criterion_trigger_instance::CriterionTriggerInstance;
use crate::advancements::predicates::item_predicate::ItemPredicate;
use crate::advancements::predicates::min_max_bounds::Ints;
use crate::core::holder::Holder;
use crate::server::level::ServerLevel;
use crate::util::codec::{self, DataResult};
use crate::world::entity::EntityId;
use crate::world::item::{Inventory, ItemStack, inventory_slot};
use crate::world::level::storage::loot::predicates::loot_item_condition::LootItemCondition;

const CONTAINER_SIZE: usize = 43;
const ITEM_SLOTS: usize = 41;

fn get_item(inventory: &Inventory, slot: usize) -> Option<&ItemStack> {
    if slot < ITEM_SLOTS { inventory.slots[inventory_slot(slot)].as_ref().filter(|stack| !stack.is_empty()) } else { None }
}

#[derive(Clone, Copy)]
pub struct InventoryChangeTrigger(pub CriterionTrigger);

impl InventoryChangeTrigger {
    pub fn trigger(self, level: &mut ServerLevel, player: EntityId, changed_item: &ItemStack) {
        let mut slots_full = 0;
        let mut slots_empty = 0;
        let mut slots_occupied = 0;
        let inventory = &level.player(player).inventory;
        for slot in 0..CONTAINER_SIZE {
            match get_item(inventory, slot) {
                None => slots_empty += 1,
                Some(item_stack) => {
                    slots_occupied += 1;
                    if item_stack.count >= item_stack.max_stack_size() {
                        slots_full += 1;
                    }
                }
            }
        }
        simple_criterion_trigger::trigger(level, player, self.0, |level, instance| {
            let CriterionTriggerInstance::InventoryChange(t) = instance else { return false };
            t.matches(&level.player(player).inventory, changed_item, slots_full, slots_empty, slots_occupied)
        });
    }
}

pub struct Slots {
    occupied: Ints,
    full: Ints,
    empty: Ints,
}

impl Slots {
    const ANY: Slots = Slots { occupied: Ints::ANY, full: Ints::ANY, empty: Ints::ANY };

    fn decode(tag: &Nbt) -> DataResult<Slots> {
        codec::map(tag)?;
        Ok(Slots {
            occupied: codec::decode_or(tag, "occupied", Ints::ANY, Ints::decode)?,
            full: codec::decode_or(tag, "full", Ints::ANY, Ints::decode)?,
            empty: codec::decode_or(tag, "empty", Ints::ANY, Ints::decode)?,
        })
    }

    fn matches(&self, slots_full: i32, slots_empty: i32, slots_occupied: i32) -> bool {
        self.full.matches(slots_full) && self.empty.matches(slots_empty) && self.occupied.matches(slots_occupied)
    }
}

pub struct TriggerInstance {
    pub player: Option<Holder<LootItemCondition>>,
    slots: Slots,
    items: Vec<ItemPredicate>,
}

impl TriggerInstance {
    pub fn decode(tag: &Nbt) -> DataResult<TriggerInstance> {
        Ok(TriggerInstance {
            player: simple_criterion_trigger::decode_player(tag)?,
            slots: codec::decode_or(tag, "slots", Slots::ANY, Slots::decode)?,
            items: codec::decode_or(tag, "items", Vec::new(), |t| codec::list_of(t, ItemPredicate::decode))?,
        })
    }

    pub fn matches(&self, inventory: &Inventory, changed_item: &ItemStack, slots_full: i32, slots_empty: i32, slots_occupied: i32) -> bool {
        if !self.slots.matches(slots_full, slots_empty, slots_occupied) {
            return false;
        }
        if self.items.is_empty() {
            return true;
        }
        if self.items.len() != 1 {
            let mut predicates: Vec<&ItemPredicate> = self.items.iter().collect();
            for slot in 0..CONTAINER_SIZE {
                if predicates.is_empty() {
                    return true;
                }
                if let Some(item_stack) = get_item(inventory, slot) {
                    predicates.retain(|predicate| !predicate.test(item_stack));
                }
            }
            return predicates.is_empty();
        }
        !changed_item.is_empty() && self.items[0].test(changed_item)
    }
}
