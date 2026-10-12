use crisp_protocol::nbt::Nbt;

use crate::server::level::ServerLevel;
use crate::world::item::ItemStack;
use crate::world::item::component::growable_mutable_container;
use crate::world::item::component::simple_mutable_container::{self, SlotConsumer};
use crate::world::item::component::{self as dc, DataComponentType, bundle_contents, charged_projectiles, item_container_contents};
use crate::world::item::item_provider::ItemProvider;
use crate::world::item::slot::slot_selector::SlotSelector;
use crate::world::level::block::entity::base_container_block_entity;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ContainerComponentManipulator {
    Container,
    BundleContents,
    ChargedProjectiles,
}

impl ContainerComponentManipulator {
    pub fn type_(self) -> DataComponentType {
        match self {
            ContainerComponentManipulator::Container => dc::CONTAINER,
            ContainerComponentManipulator::BundleContents => dc::BUNDLE_CONTENTS,
            ContainerComponentManipulator::ChargedProjectiles => dc::CHARGED_PROJECTILES,
        }
    }

    fn empty(self) -> Nbt {
        Nbt::List(Vec::new())
    }

    pub fn item_copies(self, contents: &Nbt) -> Vec<ItemStack> {
        match self {
            ContainerComponentManipulator::Container => item_container_contents::item_copies(contents),
            ContainerComponentManipulator::BundleContents | ContainerComponentManipulator::ChargedProjectiles => bundle_contents::item_copies(contents),
        }
    }

    fn copy_with_contents(self, new_contents: Vec<ItemStack>) -> Nbt {
        match self {
            ContainerComponentManipulator::Container => base_container_block_entity::from_items(&new_contents),
            ContainerComponentManipulator::BundleContents => {
                let mut builder = bundle_contents::Mutable::new();
                for mut stack in new_contents {
                    builder.try_insert(&mut stack);
                }
                builder.to_immutable()
            }
            ContainerComponentManipulator::ChargedProjectiles => Nbt::List(new_contents.iter().filter(|stack| !stack.is_empty()).map(bundle_contents::template_of).collect()),
        }
    }

    pub fn set_contents(self, item_stack: &mut ItemStack, new_contents: Vec<ItemStack>) {
        let new_value = self.copy_with_contents(new_contents);
        item_stack.set(self.type_(), Some(new_value));
    }

    pub fn modify_items(self, item_stack: &mut ItemStack, mut modifier: impl FnMut(ItemStack) -> ItemStack) {
        let Some(contents) = item_stack.get(self.type_()) else { return };
        let items: Vec<ItemStack> = self
            .item_copies(contents)
            .into_iter()
            .map(|current_item_stack| {
                if current_item_stack.is_empty() {
                    return current_item_stack;
                }
                let mut new_item_stack = modifier(current_item_stack);
                new_item_stack.limit_size(new_item_stack.max_stack_size());
                new_item_stack
            })
            .collect();
        self.set_contents(item_stack, items);
    }

    pub fn get_slots(self, item_stack: &mut ItemStack) -> ComponentSlotCollection<'_> {
        ComponentSlotCollection { item_stack, component: self }
    }
}

pub struct ComponentSlotCollection<'i> {
    item_stack: &'i mut ItemStack,
    component: ContainerComponentManipulator,
}

impl ComponentSlotCollection<'_> {
    pub fn item_copies(&self) -> Vec<ItemStack> {
        self.item_stack.get(self.component.type_()).map(|contents| self.component.item_copies(contents)).unwrap_or_default()
    }

    pub fn size(&self) -> i32 {
        self.item_copies().len() as i32
    }

    fn current_value(&self) -> Nbt {
        self.item_stack.get(self.component.type_()).cloned().unwrap_or_else(|| self.component.empty())
    }

    pub fn replace_slot_items(&mut self, items: &mut dyn ItemProvider, slot_selector: &SlotSelector) -> i32 {
        let current_value = self.current_value();
        let (slots_replaced, new_value) = match self.component {
            ContainerComponentManipulator::Container => {
                let mut mutable = item_container_contents::as_mutable(&current_value);
                (growable_mutable_container::growable_replace_slot_items(&mut mutable, items, slot_selector), mutable.to_immutable())
            }
            ContainerComponentManipulator::BundleContents => {
                let mut mutable = bundle_contents::as_mutable(&current_value);
                (mutable.replace_slot_items(items, slot_selector), mutable.to_immutable())
            }
            ContainerComponentManipulator::ChargedProjectiles => {
                let mut mutable = charged_projectiles::as_mutable(&current_value);
                (growable_mutable_container::growable_replace_slot_items(&mut mutable, items, slot_selector), mutable.to_immutable())
            }
        };
        self.item_stack.set(self.component.type_(), Some(new_value));
        slots_replaced
    }

    pub fn modify_slots(&mut self, level: &mut ServerLevel, consumer: &mut SlotConsumer, slot_selector: &SlotSelector) {
        let current_value = self.current_value();
        let new_value = match self.component {
            ContainerComponentManipulator::Container => {
                let mut mutable = item_container_contents::as_mutable(&current_value);
                simple_mutable_container::simple_modify_slots(&mut mutable, level, consumer, slot_selector);
                mutable.to_immutable()
            }
            ContainerComponentManipulator::BundleContents => {
                let mut mutable = bundle_contents::as_mutable(&current_value);
                mutable.modify_slots(level, consumer, slot_selector);
                mutable.to_immutable()
            }
            ContainerComponentManipulator::ChargedProjectiles => {
                let mut mutable = charged_projectiles::as_mutable(&current_value);
                simple_mutable_container::simple_modify_slots(&mut mutable, level, consumer, slot_selector);
                mutable.to_immutable()
            }
        };
        self.item_stack.set(self.component.type_(), Some(new_value));
    }
}
