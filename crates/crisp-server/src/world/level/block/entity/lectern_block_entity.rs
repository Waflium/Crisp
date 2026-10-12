use crisp_core::{BlockPos, Vec3};
use crisp_data::BlockState;
use crisp_data::properties::{HAS_BOOK, HORIZONTAL_FACING};
use crisp_protocol::nbt::Nbt;

use super::{BlockEntityKind, int_or};
use crate::server::level::ServerLevel;
use crate::world::entity::{EntityId, ItemEntity, LevelEntity};
use crate::world::item::ItemStack;
use crate::world::item::component::{self as dc, values};
use crate::world::level::block::lectern_block;

pub const DATA_PAGE: usize = 0;
pub const SLOT_BOOK: usize = 0;

#[derive(Clone)]
pub struct LecternBlockEntity {
    book: ItemStack,
    page: i32,
    page_count: i32,
}

impl Default for LecternBlockEntity {
    fn default() -> LecternBlockEntity {
        LecternBlockEntity { book: ItemStack::empty(), page: 0, page_count: 0 }
    }
}

fn get_page_count(book: &ItemStack) -> i32 {
    if let Some(written_content) = book.get(dc::WRITTEN_BOOK_CONTENT) {
        return values::WrittenBookContent::from_nbt(written_content).pages.len() as i32;
    }
    book.get(dc::WRITABLE_BOOK_CONTENT).map_or(0, |writable_content| values::writable_book_pages(writable_content).len() as i32)
}

fn resolve_book(mut book: ItemStack) -> ItemStack {
    values::WrittenBookContent::resolve_for_item(&mut book);
    book
}

impl LecternBlockEntity {
    pub fn get_book(&self) -> &ItemStack {
        &self.book
    }

    pub fn has_book(&self) -> bool {
        self.book.has(dc::WRITABLE_BOOK_CONTENT) || self.book.has(dc::WRITTEN_BOOK_CONTENT)
    }

    pub fn get_page(&self) -> i32 {
        self.page
    }

    pub fn get_redstone_signal(&self) -> i32 {
        let page_progress = if self.page_count > 1 { self.get_page() as f32 / (self.page_count as f32 - 1.0) } else { 1.0 };
        crisp_core::mth::floor((page_progress * 14.0) as f64) + if self.has_book() { 1 } else { 0 }
    }

    pub fn load_additional(&mut self, input: &Nbt) {
        self.book = input.get("Book").and_then(|book| ItemStack::from_nbt(book).ok()).map(resolve_book).unwrap_or_else(ItemStack::empty);
        self.page_count = get_page_count(&self.book);
        self.page = int_or(input, "Page", 0).max(0).min(self.page_count - 1);
    }

    pub fn save_additional(&self, output: &mut Vec<(String, Nbt)>) {
        if !self.book.is_empty() {
            output.push(("Book".into(), self.book.to_nbt()));
            output.push(("Page".into(), Nbt::Int(self.page)));
        }
    }
}

impl ServerLevel {
    pub fn lectern(&mut self, pos: BlockPos) -> Option<&mut LecternBlockEntity> {
        match &mut self.get_block_entity(pos)?.kind {
            BlockEntityKind::Lectern(lectern) => Some(lectern),
            _ => None,
        }
    }

    fn lectern_block_state(&self, pos: BlockPos) -> Option<BlockState> {
        self.chunks.block_entity_ref(pos).map(|entity| entity.block_state())
    }

    fn lectern_on_book_item_remove(&mut self, pos: BlockPos) {
        let Some(lectern) = self.lectern(pos) else { return };
        lectern.page = 0;
        lectern.page_count = 0;
        let Some(state) = self.lectern_block_state(pos) else { return };
        lectern_block::reset_book_state(None, self, pos, state, false);
    }

    pub fn lectern_set_book(&mut self, pos: BlockPos, book: ItemStack, _resolution_context: Option<EntityId>) {
        let book = resolve_book(book);
        let Some(lectern) = self.lectern(pos) else { return };
        lectern.page_count = get_page_count(&book);
        lectern.book = book;
        lectern.page = 0;
        self.block_entity_set_changed(pos);
    }

    pub fn lectern_set_page(&mut self, pos: BlockPos, page: i32) {
        let Some(lectern) = self.lectern(pos) else { return };
        let new_page = page.max(0).min(lectern.page_count - 1);
        if new_page != lectern.page {
            lectern.page = new_page;
            self.block_entity_set_changed(pos);
            if let Some(state) = self.lectern_block_state(pos) {
                lectern_block::signal_page_change(self, pos, state);
            }
        }
    }

    pub fn lectern_book_access_remove_item(&mut self, pos: BlockPos, slot: usize, count: i32) -> ItemStack {
        if slot != SLOT_BOOK {
            return ItemStack::empty();
        }
        let Some(lectern) = self.lectern(pos) else { return ItemStack::empty() };
        let result = lectern.book.split(count);
        if lectern.book.is_empty() {
            self.lectern_on_book_item_remove(pos);
        }
        result
    }

    pub fn lectern_book_access_remove_item_no_update(&mut self, pos: BlockPos, slot: usize) -> ItemStack {
        if slot != SLOT_BOOK {
            return ItemStack::empty();
        }
        let Some(lectern) = self.lectern(pos) else { return ItemStack::empty() };
        let prev = std::mem::replace(&mut lectern.book, ItemStack::empty());
        self.lectern_on_book_item_remove(pos);
        prev
    }

    pub fn lectern_clear_content(&mut self, pos: BlockPos) {
        self.lectern_set_book(pos, ItemStack::empty(), None);
    }

    pub fn lectern_pre_remove_side_effects(&mut self, pos: BlockPos, state: BlockState) {
        if !state.get(HAS_BOOK) {
            return;
        }
        let Some(lectern) = self.lectern(pos) else { return };
        let direction = state.get(HORIZONTAL_FACING);
        let book = lectern.book.copy();
        let xo = 0.25f32 * direction.step_x() as f32;
        let zo = 0.25f32 * direction.step_z() as f32;
        let mut entity = ItemEntity::new(Vec3::new(pos.x as f64 + 0.5 + xo as f64, (pos.y + 1) as f64, pos.z as f64 + 0.5 + zo as f64), book);
        entity.set_default_pickup_delay();
        self.add_fresh_entity(LevelEntity::Item(entity));
    }
}
