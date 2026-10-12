use crisp_core::{BlockPos, Direction};
use crisp_data::properties::{ENABLED, FACING_HOPPER};
use crisp_data::{Block, BlockState};

use super::mirror::Mirror;
use super::rotation::Rotation;
use super::{BlockBehaviour, BlockHitResult};
use crate::server::level::ServerLevel;
use crate::world::InteractionResult;
use crate::world::entity::EntityId;
use crate::world::entity::inside_block::InsideBlockEffects;
use crate::world::inventory::menu_provider::MenuProvider;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::block::entity::BlockEntityKind;
use crate::world::level::update_flags::UPDATE_CLIENTS;

pub struct HopperBlock;

fn check_powered_state(level: &mut ServerLevel, pos: BlockPos, state: BlockState) {
    let should_be_on = !level.has_neighbor_signal(pos);
    if should_be_on != state.get(ENABLED) {
        level.set_block(pos, state.set(ENABLED, should_be_on), UPDATE_CLIENTS);
    }
}

impl BlockBehaviour for HopperBlock {
    fn rotate(&self, state: BlockState, rotation: Rotation) -> BlockState {
        state.set(FACING_HOPPER, rotation.rotate(state.get(FACING_HOPPER)))
    }

    fn mirror(&self, state: BlockState, mirror: Mirror) -> BlockState {
        super::rotate(state, mirror.get_rotation(state.get(FACING_HOPPER)))
    }

    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, _level: &mut ServerLevel) -> Option<BlockState> {
        let direction = ctx.clicked_face().opposite();
        let facing = if direction.axis() == crisp_core::Axis::Y { Direction::Down } else { direction };
        Some(block.default_state().set(FACING_HOPPER, facing).set(ENABLED, true))
    }

    fn on_place(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, old_state: BlockState, _moved_by_piston: bool) {
        if !old_state.is_block(state.block()) {
            check_powered_state(level, pos, state);
        }
    }

    fn use_without_item(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos, player: EntityId, _hit: &BlockHitResult) -> InteractionResult {
        if matches!(level.get_block_entity(pos).map(|e| &e.kind), Some(BlockEntityKind::Hopper(_))) {
            level.open_menu(player, MenuProvider::BlockEntity(pos));
            level.award_stat(player, crate::stats::stats::INSPECT_HOPPER, 1);
        }
        InteractionResult::SUCCESS
    }

    fn neighbor_changed(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, _block: Block, _moved_by_piston: bool) {
        check_powered_state(level, pos, state);
    }

    fn affect_neighbors_after_removal(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, _moved_by_piston: bool) {
        crate::world::containers::update_neighbours_after_destroy(level, state, pos);
    }

    fn get_analog_output_signal(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos, _direction: Direction) -> i32 {
        level.get_redstone_signal_from_block_entity(pos)
    }

    fn entity_inside(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, entity: EntityId, _effects: &mut InsideBlockEffects, _is_precise: bool) {
        if matches!(level.get_block_entity(pos).map(|e| &e.kind), Some(BlockEntityKind::Hopper(_))) {
            level.hopper_entity_inside(pos, state, entity);
        }
    }
}
