use crisp_core::{BlockPos, Direction};
use crisp_data::properties::{BooleanProperty, HORIZONTAL_FACING, SLOT_0_OCCUPIED, SLOT_1_OCCUPIED, SLOT_2_OCCUPIED, SLOT_3_OCCUPIED, SLOT_4_OCCUPIED, SLOT_5_OCCUPIED};
use crisp_data::{Block, BlockState, item_tags, items, registry, registry_id};

use super::mirror::Mirror;
use super::rotation::Rotation;
use super::selectable_slot_container::SelectableSlotContainer;
use super::{BlockBehaviour, BlockHitResult};
use crate::server::level::{ServerLevel, SoundSource};
use crate::util::Prediction;
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;
use crate::world::item::context::BlockPlaceContext;
use crate::world::{InteractionHand, InteractionResult};

pub const SLOT_OCCUPIED_PROPERTIES: [BooleanProperty; 6] = [SLOT_0_OCCUPIED, SLOT_1_OCCUPIED, SLOT_2_OCCUPIED, SLOT_3_OCCUPIED, SLOT_4_OCCUPIED, SLOT_5_OCCUPIED];

fn sound(name: &str) -> i32 {
    registry_id(registry::SOUND_EVENT, name)
}

fn add_book(level: &mut ServerLevel, pos: BlockPos, player: EntityId, hand: InteractionHand, mut item_stack: ItemStack, slot: usize) {
    let sound_event = if item_stack.is(items::ENCHANTED_BOOK) { "minecraft:block.chiseled_bookshelf.insert.enchanted" } else { "minecraft:block.chiseled_bookshelf.insert" };
    let book = if level.has_infinite_materials(player) { item_stack.copy_with_count(1) } else { item_stack.split(1) };
    level.player_mut(player).set_item_in_hand(hand, Some(item_stack));
    level.chiseled_book_shelf_set_item(pos, slot, book);
    level.play_sound(None, pos, sound(sound_event), SoundSource::Blocks, 1.0, 1.0);
}

fn remove_book(level: &mut ServerLevel, pos: BlockPos, player: EntityId, slot: usize) {
    let retrieved_book = level.chiseled_book_shelf_remove_item(pos, slot);
    let sound_event = if retrieved_book.is(items::ENCHANTED_BOOK) { "minecraft:block.chiseled_bookshelf.pickup.enchanted" } else { "minecraft:block.chiseled_bookshelf.pickup" };
    level.play_sound(None, pos, sound(sound_event), SoundSource::Blocks, 1.0, 1.0);
    if level.player_mut(player).pick_up(retrieved_book.clone()) > 0 {
        level.player_drop(player, retrieved_book, false, Prediction::ServerOnly);
    }
}

pub struct ChiseledBookShelfBlock;

impl SelectableSlotContainer for ChiseledBookShelfBlock {
    fn get_rows(&self) -> i32 {
        2
    }

    fn get_columns(&self) -> i32 {
        3
    }
}

impl BlockBehaviour for ChiseledBookShelfBlock {
    fn use_item_on(
        &self,
        item_stack: ItemStack,
        state: BlockState,
        level: &mut ServerLevel,
        pos: BlockPos,
        player: EntityId,
        hand: InteractionHand,
        hit: &BlockHitResult,
    ) -> InteractionResult {
        if level.chiseled_book_shelf(pos).is_none() {
            return InteractionResult::Pass;
        }
        if !item_stack.get_item().is(&item_tags::BOOKSHELF_BOOKS) {
            return InteractionResult::TryWithEmptyHand;
        }
        let Some(hit_slot) = self.get_hit_slot(hit, state.get(HORIZONTAL_FACING)) else { return InteractionResult::Pass };
        if state.get(SLOT_OCCUPIED_PROPERTIES[hit_slot as usize]) {
            return InteractionResult::TryWithEmptyHand;
        }
        add_book(level, pos, player, hand, item_stack, hit_slot as usize);
        InteractionResult::SUCCESS
    }

    fn use_without_item(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, player: EntityId, hit: &BlockHitResult) -> InteractionResult {
        if level.chiseled_book_shelf(pos).is_none() {
            return InteractionResult::Pass;
        }
        let Some(hit_slot) = self.get_hit_slot(hit, state.get(HORIZONTAL_FACING)) else { return InteractionResult::Pass };
        if !state.get(SLOT_OCCUPIED_PROPERTIES[hit_slot as usize]) {
            return InteractionResult::CONSUME;
        }
        remove_book(level, pos, player, hit_slot as usize);
        InteractionResult::SUCCESS
    }

    fn affect_neighbors_after_removal(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, _moved_by_piston: bool) {
        crate::world::containers::update_neighbours_after_destroy(level, state, pos);
    }

    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, _level: &mut ServerLevel) -> Option<BlockState> {
        Some(block.default_state().set(HORIZONTAL_FACING, ctx.horizontal_direction().opposite()))
    }

    fn rotate(&self, state: BlockState, rotation: Rotation) -> BlockState {
        state.set(HORIZONTAL_FACING, rotation.rotate(state.get(HORIZONTAL_FACING)))
    }

    fn mirror(&self, state: BlockState, mirror: Mirror) -> BlockState {
        super::rotate(state, mirror.get_rotation(state.get(HORIZONTAL_FACING)))
    }

    fn get_analog_output_signal(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos, _direction: Direction) -> i32 {
        level.chiseled_book_shelf(pos).map_or(0, |block_entity| block_entity.get_last_interacted_slot() + 1)
    }
}
