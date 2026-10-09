use crisp_core::{BlockPos, Direction};
use crisp_data::properties::{DOUBLE_BLOCK_HALF, DoubleBlockHalf, HORIZONTAL_FACING, WATERLOGGED};
use crisp_data::{Block, BlockState, Fluid, block_tags};

use super::BlockBehaviour;
use super::big_dripleaf_block;
use super::double_plant_block;
use super::vegetation_block::{self, Vegetation, schedule_water_tick};
use crate::server::level::{ServerLevel, legacy_block};
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::BlockGetter;
use crate::world::level::update_flags::{UPDATE_ALL, UPDATE_CLIENTS, UPDATE_KNOWN_SHAPE};

pub struct SmallDripleafBlock;

impl Vegetation for SmallDripleafBlock {
    fn may_place_on(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) -> bool {
        state.is(&block_tags::SUPPORTS_SMALL_DRIPLEAF) || level.fluid_state(pos.above()).is_source_of_type(Fluid::Water) && state.is(&block_tags::SUPPORTS_VEGETATION)
    }
}

impl BlockBehaviour for SmallDripleafBlock {
    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        let state = double_plant_block::get_state_for_placement(block, ctx, level)?;
        let facing = ctx.horizontal_direction().opposite();
        Some(double_plant_block::copy_waterlogged_from(level, ctx.clicked_pos(), state.set(HORIZONTAL_FACING, facing)))
    }

    fn set_placed_by(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState, _placer: Option<EntityId>, _item: ItemStack) {
        let above_pos = pos.above();
        let upper = state.block().default_state().set(DOUBLE_BLOCK_HALF, DoubleBlockHalf::Upper).set(HORIZONTAL_FACING, state.get(HORIZONTAL_FACING));
        let upper = double_plant_block::copy_waterlogged_from(level, above_pos, upper);
        level.set_block(above_pos, upper, UPDATE_ALL);
    }

    fn can_survive(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) -> bool {
        if state.get(DOUBLE_BLOCK_HALF) == DoubleBlockHalf::Upper {
            return double_plant_block::can_survive(self, state, level, pos);
        }
        vegetation_block::can_survive(self, level, pos)
    }

    fn update_shape(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, direction: Direction, _: BlockPos, neighbour_state: BlockState) -> BlockState {
        if state.get(WATERLOGGED) {
            schedule_water_tick(level, pos);
        }
        double_plant_block::update_shape(state, level, pos, direction, neighbour_state)
    }

    fn player_will_destroy(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState, player: EntityId) -> BlockState {
        double_plant_block::player_will_destroy(level, pos, state, player)
    }

    fn player_destroy(&self, level: &mut ServerLevel, player: EntityId, pos: BlockPos, _state: BlockState, _block_entity: Option<&crate::world::level::block::entity::BlockEntity>, tool: Option<ItemStack>) {
        double_plant_block::player_destroy(level, player, pos, tool);
    }

    fn is_valid_bonemeal_target(&self, _level: &mut ServerLevel, _pos: BlockPos, _state: BlockState) -> bool {
        true
    }

    fn is_bonemeal_success(&self, _level: &mut ServerLevel, _pos: BlockPos, _state: BlockState) -> bool {
        true
    }

    fn perform_bonemeal(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState) {
        if state.get(DOUBLE_BLOCK_HALF) == DoubleBlockHalf::Lower {
            let above = pos.above();
            let above_fluid = level.fluid_state(above);
            level.set_block(above, legacy_block(above_fluid), UPDATE_CLIENTS | UPDATE_KNOWN_SHAPE);
            big_dripleaf_block::place_with_random_height(level, pos, state.get(HORIZONTAL_FACING));
        } else {
            let below_pos = pos.below();
            let below_state = level.block_state(below_pos);
            self.perform_bonemeal(level, below_pos, below_state);
        }
    }
}
