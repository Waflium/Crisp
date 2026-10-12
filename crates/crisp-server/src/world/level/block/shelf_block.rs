use crisp_core::{BlockPos, Direction};
use crisp_data::properties::{HORIZONTAL_FACING, POWERED, SIDE_CHAIN_PART, SideChainPart, WATERLOGGED};
use crisp_data::{Block, BlockState, Fluid, block_tags, registry, registry_id};

use super::mirror::Mirror;
use super::rotation::Rotation;
use super::selectable_slot_container::SelectableSlotContainer;
use super::side_chain_part_block::SideChainPartBlock;
use super::simple_waterlogged_block::schedule_water_tick;
use super::{BlockBehaviour, BlockHitResult};
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::entity::EntityId;
use crate::world::item::context::BlockPlaceContext;
use crate::world::item::{ItemStack, inventory_slot};
use crate::world::level::update_flags::UPDATE_ALL;
use crate::world::level::{BlockGetter, LevelAccessor};
use crate::world::{InteractionHand, InteractionResult};

fn play_sound(level: &mut ServerLevel, pos: BlockPos, sound: &str) {
    level.play_sound(None, pos, registry_id(registry::SOUND_EVENT, sound), SoundSource::Blocks, 1.0, 1.0);
}

fn swap_single_item(level: &mut ServerLevel, item_stack: ItemStack, player: EntityId, pos: BlockPos, hit_slot: usize) -> bool {
    let infinite = level.has_infinite_materials(player);
    let placed = item_stack.clone();
    let Some(shelf) = level.shelf(pos) else { return false };
    let removed_item = shelf.swap_item_no_update(hit_slot, placed);
    let new_inventory_item = if infinite && removed_item.is_empty() { item_stack.copy() } else { removed_item.clone() };
    let p = level.player_mut(player);
    let selected = p.inventory.selected_slot();
    p.inventory.slots[selected] = (!new_inventory_item.is_empty()).then_some(new_inventory_item);
    p.dirty_slots.push(selected);
    level.shelf_set_changed(pos);
    !removed_item.is_empty()
}

pub struct ShelfBlock;

impl ShelfBlock {
    fn swap_hotbar(&self, level: &mut ServerLevel, pos: BlockPos, player: EntityId) -> bool {
        let connected_blocks = self.get_all_blocks_connected_to(level, pos);
        if connected_blocks.is_empty() {
            return false;
        }
        let mut any_swapped = false;
        for (shelf_part_index, &shelf_pos) in connected_blocks.iter().enumerate() {
            let Some(container_size) = level.shelf(shelf_pos).map(|shelf| shelf.get_items().len()) else { continue };
            for slot in 0..container_size {
                let inventory_slot_index = 9 - (connected_blocks.len() - shelf_part_index) as i32 * container_size as i32 + slot as i32;
                if (0..=41).contains(&inventory_slot_index) {
                    let window_slot = inventory_slot(inventory_slot_index as usize);
                    let p = level.player_mut(player);
                    let placed_inventory_item = p.inventory.slots[window_slot].take().unwrap_or_else(ItemStack::empty);
                    let placed_was_empty = placed_inventory_item.is_empty();
                    let removed_shelf_item = level.shelf(shelf_pos).map_or_else(ItemStack::empty, |shelf| shelf.swap_item_no_update(slot, placed_inventory_item));
                    if !placed_was_empty || !removed_shelf_item.is_empty() {
                        let p = level.player_mut(player);
                        p.inventory.slots[window_slot] = (!removed_shelf_item.is_empty()).then_some(removed_shelf_item);
                        p.dirty_slots.push(window_slot);
                        any_swapped = true;
                    }
                }
            }
            level.shelf_set_changed(shelf_pos);
        }
        any_swapped
    }
}

impl SelectableSlotContainer for ShelfBlock {
    fn get_rows(&self) -> i32 {
        1
    }

    fn get_columns(&self) -> i32 {
        3
    }
}

impl SideChainPartBlock for ShelfBlock {
    fn get_side_chain_part(&self, state: BlockState) -> SideChainPart {
        state.get(SIDE_CHAIN_PART)
    }

    fn set_side_chain_part(&self, state: BlockState, new_part: SideChainPart) -> BlockState {
        state.set(SIDE_CHAIN_PART, new_part)
    }

    fn get_facing(&self, state: BlockState) -> Direction {
        state.get(HORIZONTAL_FACING)
    }

    fn is_connectable(&self, state: BlockState) -> bool {
        state.is(&block_tags::WOODEN_SHELVES) && state.has(POWERED) && state.get(POWERED)
    }

    fn get_max_chain_length(&self) -> i32 {
        3
    }
}

impl BlockBehaviour for ShelfBlock {
    fn affect_neighbors_after_removal(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, _moved_by_piston: bool) {
        crate::world::containers::update_neighbours_after_destroy(level, state, pos);
        self.update_neighbors_after_powering_down(level, pos, state);
    }

    fn neighbor_changed(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, _block: Block, _moved_by_piston: bool) {
        let signal = level.has_neighbor_signal(pos);
        if state.get(POWERED) != signal {
            let mut new_state = state.set(POWERED, signal);
            if !signal {
                new_state = new_state.set(SIDE_CHAIN_PART, SideChainPart::Unconnected);
            }
            level.set_block(pos, new_state, UPDATE_ALL);
            play_sound(level, pos, if signal { "minecraft:block.shelf.activate" } else { "minecraft:block.shelf.deactivate" });
        }
    }

    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        let pos = ctx.clicked_pos();
        let replaced_fluid_state = level.fluid_state(pos);
        Some(
            block
                .default_state()
                .set(HORIZONTAL_FACING, ctx.horizontal_direction().opposite())
                .set(POWERED, level.has_neighbor_signal(pos))
                .set(WATERLOGGED, replaced_fluid_state.fluid == Fluid::Water),
        )
    }

    fn rotate(&self, state: BlockState, rotation: Rotation) -> BlockState {
        state.set(HORIZONTAL_FACING, rotation.rotate(state.get(HORIZONTAL_FACING)))
    }

    fn mirror(&self, state: BlockState, mirror: Mirror) -> BlockState {
        super::rotate(state, mirror.get_rotation(state.get(HORIZONTAL_FACING)))
    }

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
        if level.shelf(pos).is_none() || hand == InteractionHand::OffHand {
            return InteractionResult::Pass;
        }
        let Some(hit_slot) = self.get_hit_slot(hit, state.get(HORIZONTAL_FACING)) else { return InteractionResult::Pass };
        if !state.get(POWERED) {
            let item_removed = swap_single_item(level, item_stack.clone(), player, pos, hit_slot as usize);
            if item_removed {
                play_sound(level, pos, if item_stack.is_empty() { "minecraft:block.shelf.take_item" } else { "minecraft:block.shelf.single_swap" });
            } else {
                if item_stack.is_empty() {
                    return InteractionResult::Pass;
                }
                play_sound(level, pos, "minecraft:block.shelf.place_item");
            }
            InteractionResult::SUCCESS_SERVER
        } else {
            let any_swapped = self.swap_hotbar(level, pos, player);
            if !any_swapped {
                return InteractionResult::CONSUME;
            }
            play_sound(level, pos, "minecraft:block.shelf.multi_swap");
            InteractionResult::SUCCESS_SERVER
        }
    }

    fn on_place(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, old_state: BlockState, _moved_by_piston: bool) {
        if state.get(POWERED) {
            self.update_self_and_neighbors_on_powering_up(level, pos, state, old_state);
        } else {
            self.update_neighbors_after_powering_down(level, pos, state);
        }
    }

    fn update_shape(&self, state: BlockState, level: &mut dyn LevelAccessor, pos: BlockPos, _: Direction, _: BlockPos, _: BlockState) -> BlockState {
        schedule_water_tick(state, level, pos);
        state
    }

    fn get_analog_output_signal(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, direction: Direction) -> i32 {
        if direction != state.get(HORIZONTAL_FACING).opposite() {
            return 0;
        }
        let Some(block_entity) = level.shelf(pos) else { return 0 };
        let item1_bit = if block_entity.get_item(0).is_empty() { 0 } else { 1 };
        let item2_bit = if block_entity.get_item(1).is_empty() { 0 } else { 1 };
        let item3_bit = if block_entity.get_item(2).is_empty() { 0 } else { 1 };
        item1_bit | item2_bit << 1 | item3_bit << 2
    }
}
