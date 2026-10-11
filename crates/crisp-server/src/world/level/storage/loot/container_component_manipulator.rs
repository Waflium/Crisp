use crisp_protocol::nbt::Nbt;

use crate::world::item::ItemStack;
use crate::world::item::component::values::{self, ItemStackTemplate};
use crate::world::item::component::{self as dc, DataComponentType, bundle_contents};
use crate::world::level::block::entity::base_container_block_entity;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ContainerComponentManipulator {
    Container,
    BundleContents,
    ChargedProjectiles,
}

fn template_of(stack: &ItemStack) -> Nbt {
    ItemStackTemplate { item: stack.get_item().name().to_string(), count: stack.count, components: stack.components_patch().to_nbt() }.to_nbt()
}

impl ContainerComponentManipulator {
    pub fn type_(self) -> DataComponentType {
        match self {
            ContainerComponentManipulator::Container => dc::CONTAINER,
            ContainerComponentManipulator::BundleContents => dc::BUNDLE_CONTENTS,
            ContainerComponentManipulator::ChargedProjectiles => dc::CHARGED_PROJECTILES,
        }
    }

    pub fn item_copies(self, contents: &Nbt) -> Vec<ItemStack> {
        match self {
            ContainerComponentManipulator::Container => {
                let slots = values::container_slots(contents);
                let Some(max_slot_index) = slots.iter().map(|&(index, _)| index).max() else { return Vec::new() };
                let mut items = vec![ItemStack::empty(); max_slot_index as usize + 1];
                for (index, template) in &slots {
                    items[*index as usize] = template.create().unwrap_or_else(ItemStack::empty);
                }
                items
            }
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
            ContainerComponentManipulator::ChargedProjectiles => Nbt::List(new_contents.iter().filter(|stack| !stack.is_empty()).map(template_of).collect()),
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
}
