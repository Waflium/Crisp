use crisp_core::random::RandomSource;
use crisp_core::shapes::VoxelShape;
use crisp_core::{BlockPos, Direction};
use crisp_data::properties::{PICKLES, WATERLOGGED};
use crisp_data::{Block, BlockState, Fluid, SupportType, block_tags, blocks};

use super::BlockBehaviour;
use super::vegetation_block::{self, Vegetation, schedule_water_tick};
use crate::server::level::ServerLevel;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::{BlockGetter, LevelAccessor, LevelReader};
use crate::world::level::update_flags::{UPDATE_ALL, UPDATE_CLIENTS};

pub struct SeaPickleBlock;

fn is_dead(state: BlockState) -> bool {
    !state.get(WATERLOGGED)
}

impl Vegetation for SeaPickleBlock {
    fn may_place_on(&self, state: BlockState, _level: &mut dyn LevelReader, _pos: BlockPos) -> bool {
        !VoxelShape::from_boxes(state.collision_shape()).face_shape(Direction::Up).is_empty() || state.is_face_sturdy(Direction::Up, SupportType::Full)
    }
}

impl BlockBehaviour for SeaPickleBlock {
    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        let state = level.block_state(ctx.clicked_pos());
        if state.is_block(block) {
            return Some(state.set(PICKLES, 4.min(state.get(PICKLES) + 1)));
        }
        let replaced_fluid = level.fluid_state(ctx.clicked_pos());
        Some(block.default_state().set(WATERLOGGED, replaced_fluid.fluid == Fluid::Water))
    }

    fn can_be_replaced(&self, state: BlockState, ctx: &BlockPlaceContext) -> bool {
        (!ctx.is_secondary_use_active() && !ctx.item.is_empty() && ctx.item.item == state.block().item() && state.get(PICKLES) < 4)
            || vegetation_block::block_can_be_replaced(state, ctx)
    }

    fn update_shape(&self, state: BlockState, level: &mut dyn LevelAccessor, pos: BlockPos, _: Direction, _: BlockPos, _: BlockState) -> BlockState {
        if !self.can_survive(state, level, pos) {
            return blocks::AIR.default_state();
        }
        if state.get(WATERLOGGED) {
            schedule_water_tick(level, pos);
        }
        vegetation_block::update_shape(state, level, pos)
    }

    fn can_survive(&self, _state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> bool {
        vegetation_block::can_survive(self, level, pos)
    }

    fn is_valid_bonemeal_target(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState) -> bool {
        !is_dead(state) && level.block_state(pos.below()).is(&block_tags::CORAL_BLOCKS)
    }

    fn is_bonemeal_success(&self, _level: &mut ServerLevel, _pos: BlockPos, _state: BlockState) -> bool {
        true
    }

    fn perform_bonemeal(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState) {
        let mut z_span = 1;
        let mut count = 0;
        let x_start = pos.x - 2;
        let mut z_offset = 0;
        for x in 0..5 {
            for z in 0..z_span {
                let end_y = 2 + pos.y - 1;
                for start_y in end_y - 2..end_y {
                    let position = BlockPos::new(x_start + x, start_y, pos.z - z_offset + z);
                    if position != pos && level.random.next_int_bounded(6) == 0 && level.block_state(position).is_block(blocks::WATER) {
                        let below_state = level.block_state(position.below());
                        if below_state.is(&block_tags::CORAL_BLOCKS) {
                            let pickles = level.random.next_int_bounded(4) + 1;
                            level.set_block(position, blocks::SEA_PICKLE.default_state().set(PICKLES, pickles), UPDATE_ALL);
                        }
                    }
                }
            }
            if count < 2 {
                z_span += 2;
                z_offset += 1;
            } else {
                z_span -= 2;
                z_offset -= 1;
            }
            count += 1;
        }
        level.set_block(pos, state.set(PICKLES, 4), UPDATE_CLIENTS);
    }
}
