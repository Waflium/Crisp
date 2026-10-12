use crisp_core::BlockPos;
use crisp_protocol::nbt::Nbt;

use super::BlockEntityKind;
use super::base_container_block_entity::{copy_into, from_items, load_all_items, save_all_items};
use crate::server::level::ServerLevel;
use crate::world::item::ItemStack;
use crate::world::item::component::{self as dc, DataComponentType};

pub const MAX_ITEMS: usize = 3;
const ALIGN_ITEMS_TO_BOTTOM_TAG: &str = "align_items_to_bottom";

#[derive(Clone)]
pub struct ShelfBlockEntity {
    items: Vec<ItemStack>,
    align_items_to_bottom: bool,
}

impl Default for ShelfBlockEntity {
    fn default() -> ShelfBlockEntity {
        ShelfBlockEntity { items: vec![ItemStack::empty(); MAX_ITEMS], align_items_to_bottom: false }
    }
}

impl ShelfBlockEntity {
    pub fn load_additional(&mut self, input: &Nbt) {
        self.items = vec![ItemStack::empty(); MAX_ITEMS];
        load_all_items(input, &mut self.items);
        self.align_items_to_bottom = input.get(ALIGN_ITEMS_TO_BOTTOM_TAG).and_then(Nbt::as_i64).is_some_and(|v| v != 0);
    }

    pub fn save_additional(&self, output: &mut Vec<(String, Nbt)>) {
        save_all_items(output, &self.items, true);
        output.push((ALIGN_ITEMS_TO_BOTTOM_TAG.into(), Nbt::Byte(self.align_items_to_bottom as i8)));
    }

    pub fn get_update_tag(&self) -> Nbt {
        let mut output = Vec::new();
        save_all_items(&mut output, &self.items, true);
        output.push((ALIGN_ITEMS_TO_BOTTOM_TAG.into(), Nbt::Byte(self.align_items_to_bottom as i8)));
        Nbt::Compound(output)
    }

    pub fn get_items(&self) -> &[ItemStack] {
        &self.items
    }

    pub fn get_items_mut(&mut self) -> &mut Vec<ItemStack> {
        &mut self.items
    }

    pub fn get_item(&self, slot: usize) -> &ItemStack {
        &self.items[slot]
    }

    pub fn swap_item_no_update(&mut self, slot: usize, mut held_item_stack: ItemStack) -> ItemStack {
        let retrieved_item = match self.items.get_mut(slot) {
            Some(stack) if !stack.is_empty() => stack.split(99),
            _ => ItemStack::empty(),
        };
        held_item_stack.limit_size(held_item_stack.max_stack_size().min(99));
        self.items[slot] = held_item_stack;
        retrieved_item
    }

    pub fn get_align_items_to_bottom(&self) -> bool {
        self.align_items_to_bottom
    }

    pub fn apply_implicit_components<'a>(&mut self, get: &mut impl FnMut(DataComponentType) -> Option<&'a Nbt>) {
        copy_into(get(dc::CONTAINER), &mut self.items);
    }

    pub fn collect_implicit_components(&self, components: &mut Vec<(DataComponentType, Option<Nbt>)>) {
        components.retain(|(t, _)| *t != dc::CONTAINER);
        components.push((dc::CONTAINER, Some(from_items(&self.items))));
    }
}

impl ServerLevel {
    pub fn shelf(&mut self, pos: BlockPos) -> Option<&mut ShelfBlockEntity> {
        match &mut self.get_block_entity(pos)?.kind {
            BlockEntityKind::Shelf(shelf) => Some(shelf),
            _ => None,
        }
    }

    pub fn shelf_set_changed(&mut self, pos: BlockPos) {
        let Some(state) = self.chunks.block_entity_ref(pos).map(|entity| entity.block_state()) else { return };
        self.block_entity_set_changed(pos);
        self.send_block_updated(pos, state, state);
    }
}
