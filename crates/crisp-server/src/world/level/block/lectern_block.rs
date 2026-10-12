use crisp_core::{BlockPos, Direction};
use crisp_data::properties::{HAS_BOOK, HORIZONTAL_FACING, POWERED};
use crisp_data::{Block, BlockState, item_tags, registry, registry_id};

use super::mirror::Mirror;
use super::rotation::Rotation;
use super::{BlockBehaviour, BlockHitResult};
use crate::server::level::events::SOUND_PAGE_TURN;
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::entity::EntityId;
use crate::world::inventory::menu_provider::MenuProvider;
use crate::world::item::ItemStack;
use crate::world::item::component::{self as dc, values};
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::LevelReader;
use crate::world::level::update_flags::UPDATE_ALL;
use crate::world::{InteractionHand, InteractionResult};

const PAGE_CHANGE_IMPULSE_TICKS: i32 = 2;

pub fn try_place_book(source_entity: Option<EntityId>, level: &mut ServerLevel, pos: BlockPos, state: BlockState, item: &mut ItemStack) -> bool {
    if !state.get(HAS_BOOK) {
        place_book(source_entity, level, pos, state, item);
        true
    } else {
        false
    }
}

fn place_book(source_entity: Option<EntityId>, level: &mut ServerLevel, pos: BlockPos, state: BlockState, book: &mut ItemStack) {
    if level.lectern(pos).is_some() {
        let infinite = source_entity.is_some_and(|source| level.has_infinite_materials(source));
        let consumed = if infinite { book.copy_with_count(1) } else { book.split(1) };
        level.lectern_set_book(pos, consumed, source_entity);
        reset_book_state(source_entity, level, pos, state, true);
        level.play_sound(None, pos, registry_id(registry::SOUND_EVENT, "minecraft:item.book.put"), SoundSource::Blocks, 1.0, 1.0);
    }
}

pub fn reset_book_state(_source_entity: Option<EntityId>, level: &mut ServerLevel, pos: BlockPos, state: BlockState, has_book: bool) {
    let new_state = state.set(POWERED, false).set(HAS_BOOK, has_book);
    level.set_block(pos, new_state, UPDATE_ALL);
    update_below(level, pos, state);
}

pub fn signal_page_change(level: &mut ServerLevel, pos: BlockPos, state: BlockState) {
    change_powered(level, pos, state, true);
    level.schedule_tick(pos, state.block(), PAGE_CHANGE_IMPULSE_TICKS);
    level.level_event(None, SOUND_PAGE_TURN, pos, 0);
}

fn change_powered(level: &mut ServerLevel, pos: BlockPos, state: BlockState, is_powered: bool) {
    level.set_block(pos, state.set(POWERED, is_powered), UPDATE_ALL);
    update_below(level, pos, state);
}

fn update_below(level: &mut ServerLevel, pos: BlockPos, state: BlockState) {
    level.update_neighbors_at(pos.below(), state.block());
}

pub struct LecternBlock;

impl LecternBlock {
    fn open_screen(&self, level: &mut ServerLevel, pos: BlockPos, player: EntityId) {
        if level.lectern(pos).is_some() {
            level.open_menu(player, MenuProvider::BlockEntity(pos));
        }
    }
}

impl BlockBehaviour for LecternBlock {
    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        let mut has_book = false;
        if let Some(player) = &ctx.player
            && level.can_use_game_master_blocks(player.id)
            && let Some(block_entity_data) = ctx.item.get(dc::BLOCK_ENTITY_DATA)
            && values::typed_entity_data(block_entity_data).1.get("Book").is_some()
        {
            has_book = true;
        }
        Some(block.default_state().set(HORIZONTAL_FACING, ctx.horizontal_direction().opposite()).set(HAS_BOOK, has_book))
    }

    fn rotate(&self, state: BlockState, rotation: Rotation) -> BlockState {
        state.set(HORIZONTAL_FACING, rotation.rotate(state.get(HORIZONTAL_FACING)))
    }

    fn mirror(&self, state: BlockState, mirror: Mirror) -> BlockState {
        super::rotate(state, mirror.get_rotation(state.get(HORIZONTAL_FACING)))
    }

    fn tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        change_powered(level, pos, state, false);
    }

    fn affect_neighbors_after_removal(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, _moved_by_piston: bool) {
        if state.get(POWERED) {
            update_below(level, pos, state);
        }
    }

    fn own_signal(&self, state: BlockState, _level: &mut dyn LevelReader, _pos: BlockPos) -> i32 {
        if state.get(POWERED) { 15 } else { 0 }
    }

    fn get_direct_signal(&self, state: BlockState, _level: &mut dyn LevelReader, _pos: BlockPos, direction: Direction) -> i32 {
        if direction == Direction::Up && state.get(POWERED) { 15 } else { 0 }
    }

    fn get_analog_output_signal(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, _direction: Direction) -> i32 {
        if !state.get(HAS_BOOK) {
            return 0;
        }
        level.lectern(pos).map_or(0, |lectern| lectern.get_redstone_signal())
    }

    fn use_item_on(
        &self,
        item_stack: ItemStack,
        state: BlockState,
        level: &mut ServerLevel,
        pos: BlockPos,
        player: EntityId,
        hand: InteractionHand,
        _hit: &BlockHitResult,
    ) -> InteractionResult {
        if state.get(HAS_BOOK) {
            InteractionResult::TryWithEmptyHand
        } else if item_stack.get_item().is(&item_tags::LECTERN_BOOKS) {
            let mut item_stack = item_stack;
            if try_place_book(Some(player), level, pos, state, &mut item_stack) {
                level.player_mut(player).set_item_in_hand(hand, Some(item_stack));
                InteractionResult::SUCCESS
            } else {
                InteractionResult::Pass
            }
        } else if item_stack.is_empty() && hand == InteractionHand::MainHand {
            InteractionResult::Pass
        } else {
            InteractionResult::TryWithEmptyHand
        }
    }

    fn use_without_item(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, player: EntityId, _hit: &BlockHitResult) -> InteractionResult {
        if state.get(HAS_BOOK) {
            self.open_screen(level, pos, player);
            InteractionResult::SUCCESS
        } else {
            InteractionResult::CONSUME
        }
    }
}
