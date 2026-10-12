use std::rc::Rc;

use super::slot_selector::{self, ItemFilter, SlotSelector};
use crate::server::level::ServerLevel;
use crate::world::entity::slot_access::LevelSlot;
use crate::world::item::ItemStack;
use crate::world::item::component::simple_mutable_container::SlotConsumer;
use crate::world::item::item_provider::ItemProvider;
use crate::world::level::storage::loot::container_component_manipulator::ContainerComponentManipulator;

pub enum SlotCollection<'s> {
    Empty,
    Of(Vec<LevelSlot>),
    Concat(Vec<SlotCollection<'s>>),
    Filtered { slots: Box<SlotCollection<'s>>, filter: ItemFilter<'s> },
    FlatMapped { slots: Box<SlotCollection<'s>>, component: ContainerComponentManipulator },
    Limited { slots: Box<SlotCollection<'s>>, limit: i32 },
}

impl<'s> SlotCollection<'s> {
    pub fn of(slots: Vec<LevelSlot>) -> SlotCollection<'s> {
        if slots.is_empty() { SlotCollection::Empty } else { SlotCollection::Of(slots) }
    }

    pub fn concat(mut terms: Vec<SlotCollection<'s>>) -> SlotCollection<'s> {
        match terms.len() {
            0 => SlotCollection::Empty,
            1 => terms.pop().unwrap(),
            _ => SlotCollection::Concat(terms),
        }
    }

    pub fn filter(self, predicate: ItemFilter<'s>) -> SlotCollection<'s> {
        match self {
            SlotCollection::Filtered { slots, filter } => {
                SlotCollection::Filtered { slots, filter: Rc::new(move |t: &ItemStack| filter(t) && predicate(t)) }
            }
            slots => SlotCollection::Filtered { slots: Box::new(slots), filter: predicate },
        }
    }

    pub fn flat_map(self, component: ContainerComponentManipulator) -> SlotCollection<'s> {
        SlotCollection::FlatMapped { slots: Box::new(self), component }
    }

    pub fn limit(self, limit: i32) -> SlotCollection<'s> {
        match self {
            SlotCollection::Limited { slots, limit: own_limit } => SlotCollection::Limited { slots, limit: own_limit.min(limit) },
            slots => SlotCollection::Limited { slots: Box::new(slots), limit },
        }
    }

    pub fn item_copies(&self, level: &mut ServerLevel) -> Vec<ItemStack> {
        match self {
            SlotCollection::Empty => Vec::new(),
            SlotCollection::Of(slots) => slots.iter().map(|slot| level.level_slot_get(slot)).collect(),
            SlotCollection::Concat(terms) => terms.iter().flat_map(|slots| slots.item_copies(level)).collect(),
            SlotCollection::Filtered { slots, filter } => slots.item_copies(level).into_iter().filter(|item| filter(item)).collect(),
            SlotCollection::FlatMapped { slots, component } => {
                slots.item_copies(level).into_iter().flat_map(|mut item| component.get_slots(&mut item).item_copies()).collect()
            }
            SlotCollection::Limited { slots, limit } => slots.item_copies(level).into_iter().take((*limit).max(0) as usize).collect(),
        }
    }

    pub fn size(&self, level: &mut ServerLevel) -> i32 {
        match self {
            SlotCollection::Empty => 0,
            SlotCollection::Of(slots) => slots.len() as i32,
            SlotCollection::Concat(terms) => terms.iter().map(|slots| slots.size(level)).sum(),
            SlotCollection::Filtered { .. } => self.item_copies(level).len() as i32,
            SlotCollection::FlatMapped { slots, component } => slots.item_copies(level).into_iter().map(|mut item| component.get_slots(&mut item).size()).sum(),
            SlotCollection::Limited { slots, limit } => slots.size(level).min(*limit),
        }
    }

    pub fn replace_slot_items(&self, level: &mut ServerLevel, items: &mut dyn ItemProvider, slot_selector: &SlotSelector<'s>) -> i32 {
        match self {
            SlotCollection::Empty => 0,
            SlotCollection::Of(slots) => {
                let mut success_count = 0;
                let mut iterator = slots.iter();
                while items.has_next() {
                    let Some(slot) = iterator.next() else { break };
                    if slot_selector.try_select_slot(&level.level_slot_get(slot)) && level.level_slot_set(slot, items.next()) {
                        success_count += 1;
                    }
                }
                success_count
            }
            SlotCollection::Concat(terms) => terms.iter().map(|slots| slots.replace_slot_items(level, items, slot_selector)).sum(),
            SlotCollection::Filtered { slots, filter } => slots.replace_slot_items(level, items, &slot_selector.filter(filter.clone())),
            SlotCollection::FlatMapped { slots, component } => {
                let mut success_count = 0;
                slots.modify_slots(
                    level,
                    &mut |level, slot| {
                        slot.modify(level, &mut |_, item| success_count += component.get_slots(item).replace_slot_items(&mut *items, slot_selector))
                    },
                    &slot_selector::ANY_SLOT,
                );
                success_count
            }
            SlotCollection::Limited { slots, limit } => slots.replace_slot_items(level, items, &slot_selector.limit(*limit)),
        }
    }

    pub fn modify_slots(&self, level: &mut ServerLevel, consumer: &mut SlotConsumer, slot_selector: &SlotSelector<'s>) {
        match self {
            SlotCollection::Empty => {}
            SlotCollection::Of(slots) => {
                for slot in slots {
                    if slot_selector.try_select_slot(&level.level_slot_get(slot)) {
                        consumer(level, &mut slot.clone());
                    }
                }
            }
            SlotCollection::Concat(terms) => {
                for slots in terms {
                    slots.modify_slots(level, consumer, slot_selector);
                }
            }
            SlotCollection::Filtered { slots, filter } => slots.modify_slots(level, consumer, &slot_selector.filter(filter.clone())),
            SlotCollection::FlatMapped { slots, component } => slots.modify_slots(
                level,
                &mut |level, slot| slot.modify(level, &mut |level, item| component.get_slots(item).modify_slots(level, &mut *consumer, slot_selector)),
                &slot_selector::ANY_SLOT,
            ),
            SlotCollection::Limited { slots, limit } => slots.modify_slots(level, consumer, &slot_selector.limit(*limit)),
        }
    }
}
