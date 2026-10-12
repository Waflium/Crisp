use crisp_core::{BlockPos, Direction};
use crisp_data::properties::HORIZONTAL_FACING;
use crisp_data::{Block, BlockState, blocks};

use super::BlockBehaviour;
use super::abstract_banner_block;
use super::mirror::Mirror;
use super::rotation::Rotation;
use crate::server::level::ServerLevel;
use crate::world::item::ItemStack;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::{LevelAccessor, LevelReader};

pub struct WallBannerBlock;

impl BlockBehaviour for WallBannerBlock {
    fn can_survive(&self, state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> bool {
        level.block_state(pos.relative(state.get(HORIZONTAL_FACING).opposite())).is_solid()
    }

    fn update_shape(&self, state: BlockState, level: &mut dyn LevelAccessor, pos: BlockPos, direction: Direction, _: BlockPos, _: BlockState) -> BlockState {
        if direction == state.get(HORIZONTAL_FACING).opposite() && !self.can_survive(state, level, pos) { blocks::AIR.default_state() } else { state }
    }

    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        let mut state = block.default_state();
        let pos = ctx.clicked_pos();
        for direction in ctx.nearest_looking_directions() {
            if direction.axis().is_horizontal() {
                state = state.set(HORIZONTAL_FACING, direction.opposite());
                if self.can_survive(state, level, pos) {
                    return Some(state);
                }
            }
        }
        None
    }

    fn rotate(&self, state: BlockState, rotation: Rotation) -> BlockState {
        state.set(HORIZONTAL_FACING, rotation.rotate(state.get(HORIZONTAL_FACING)))
    }

    fn mirror(&self, state: BlockState, mirror: Mirror) -> BlockState {
        super::rotate(state, mirror.get_rotation(state.get(HORIZONTAL_FACING)))
    }

    fn get_clone_item_stack(&self, level: &ServerLevel, pos: BlockPos, state: BlockState, include_data: bool) -> ItemStack {
        abstract_banner_block::get_clone_item_stack(level, pos, state, include_data)
    }
}
