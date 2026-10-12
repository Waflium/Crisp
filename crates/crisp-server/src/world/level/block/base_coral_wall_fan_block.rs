use crisp_core::{BlockPos, Direction};
use crisp_data::properties::HORIZONTAL_FACING;
use crisp_data::{Block, BlockState, SupportType, blocks};

use super::BlockBehaviour;
use super::base_coral_plant_type_block;
use super::mirror::Mirror;
use super::rotation::Rotation;
use super::simple_waterlogged_block::schedule_water_tick;
use crate::server::level::ServerLevel;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::{LevelAccessor, LevelReader};

pub struct BaseCoralWallFanBlock;

impl BlockBehaviour for BaseCoralWallFanBlock {
    fn rotate(&self, state: BlockState, rotation: Rotation) -> BlockState {
        state.set(HORIZONTAL_FACING, rotation.rotate(state.get(HORIZONTAL_FACING)))
    }

    fn mirror(&self, state: BlockState, mirror: Mirror) -> BlockState {
        super::rotate(state, mirror.get_rotation(state.get(HORIZONTAL_FACING)))
    }

    fn update_shape(&self, state: BlockState, level: &mut dyn LevelAccessor, pos: BlockPos, direction: Direction, _: BlockPos, _: BlockState) -> BlockState {
        schedule_water_tick(state, level, pos);
        if direction.opposite() == state.get(HORIZONTAL_FACING) && !self.can_survive(state, level, pos) { blocks::AIR.default_state() } else { state }
    }

    fn can_survive(&self, state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> bool {
        let facing: Direction = state.get(HORIZONTAL_FACING);
        let relative_pos = pos.relative(facing.opposite());
        level.block_state(relative_pos).is_face_sturdy(facing, SupportType::Full)
    }

    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        let mut state = base_coral_plant_type_block::get_state_for_placement(block, ctx, level);
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
}
