use crisp_core::BlockPos;
use crisp_data::item_tags;
use crisp_protocol::nbt::Nbt;

use super::base_container_block_entity::{copy_into, from_items, load_all_items, save_all_items};
use super::{BlockEntityKind, int_or};
use crate::server::level::ServerLevel;
use crate::world::item::ItemStack;
use crate::world::item::component::{self as dc, DataComponentType};
use crate::world::level::block::chiseled_book_shelf_block::SLOT_OCCUPIED_PROPERTIES;
use crate::world::level::update_flags::UPDATE_ALL;

pub const MAX_BOOKS_IN_STORAGE: usize = 6;
const DEFAULT_LAST_INTERACTED_SLOT: i32 = -1;

#[derive(Clone)]
pub struct ChiseledBookShelfBlockEntity {
    items: Vec<ItemStack>,
    last_interacted_slot: i32,
}

impl Default for ChiseledBookShelfBlockEntity {
    fn default() -> ChiseledBookShelfBlockEntity {
        ChiseledBookShelfBlockEntity { items: vec![ItemStack::empty(); MAX_BOOKS_IN_STORAGE], last_interacted_slot: DEFAULT_LAST_INTERACTED_SLOT }
    }
}

impl ChiseledBookShelfBlockEntity {
    pub fn load_additional(&mut self, input: &Nbt) {
        self.items = vec![ItemStack::empty(); MAX_BOOKS_IN_STORAGE];
        load_all_items(input, &mut self.items);
        self.last_interacted_slot = int_or(input, "last_interacted_slot", DEFAULT_LAST_INTERACTED_SLOT);
    }

    pub fn save_additional(&self, output: &mut Vec<(String, Nbt)>) {
        save_all_items(output, &self.items, true);
        output.push(("last_interacted_slot".into(), Nbt::Int(self.last_interacted_slot)));
    }

    pub fn get_items(&self) -> &[ItemStack] {
        &self.items
    }

    pub fn get_items_mut(&mut self) -> &mut Vec<ItemStack> {
        &mut self.items
    }

    pub fn get_last_interacted_slot(&self) -> i32 {
        self.last_interacted_slot
    }

    pub fn accepts_item_type(item_stack: &ItemStack) -> bool {
        item_stack.get_item().is(&item_tags::BOOKSHELF_BOOKS)
    }

    pub fn can_place_item(&self, slot: usize, item_stack: &ItemStack) -> bool {
        Self::accepts_item_type(item_stack) && (self.items[slot].is_empty() || self.items[slot].count < item_stack.max_stack_size().min(1))
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
    pub fn chiseled_book_shelf(&mut self, pos: BlockPos) -> Option<&mut ChiseledBookShelfBlockEntity> {
        match &mut self.get_block_entity(pos)?.kind {
            BlockEntityKind::ChiseledBookShelf(bookshelf) => Some(bookshelf),
            _ => None,
        }
    }

    fn chiseled_book_shelf_update_state(&mut self, pos: BlockPos, interacted_slot: usize) {
        if interacted_slot >= MAX_BOOKS_IN_STORAGE {
            error!("Expected slot 0-5, got {interacted_slot}");
            return;
        }
        let Some(entity) = self.get_block_entity(pos) else { return };
        let mut updated_state = entity.block_state();
        let BlockEntityKind::ChiseledBookShelf(bookshelf) = &mut entity.kind else { return };
        bookshelf.last_interacted_slot = interacted_slot as i32;
        for (slot, &slot_property) in SLOT_OCCUPIED_PROPERTIES.iter().enumerate() {
            updated_state = updated_state.set(slot_property, !bookshelf.items[slot].is_empty());
        }
        self.set_block(pos, updated_state, UPDATE_ALL);
    }

    pub fn chiseled_book_shelf_remove_item(&mut self, pos: BlockPos, slot: usize) -> ItemStack {
        let Some(bookshelf) = self.chiseled_book_shelf(pos) else { return ItemStack::empty() };
        let retrieved_item = std::mem::replace(&mut bookshelf.items[slot], ItemStack::empty());
        if !retrieved_item.is_empty() {
            self.chiseled_book_shelf_update_state(pos, slot);
        }
        retrieved_item
    }

    pub fn chiseled_book_shelf_set_item(&mut self, pos: BlockPos, slot: usize, item_stack: ItemStack) {
        if ChiseledBookShelfBlockEntity::accepts_item_type(&item_stack) {
            let Some(bookshelf) = self.chiseled_book_shelf(pos) else { return };
            bookshelf.items[slot] = item_stack;
            self.chiseled_book_shelf_update_state(pos, slot);
        } else if item_stack.is_empty() {
            self.chiseled_book_shelf_remove_item(pos, slot);
        }
    }
}
