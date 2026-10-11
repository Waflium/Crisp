use crisp_core::{BlockPos, Direction};
use crisp_data::properties::{DOUBLE_BLOCK_HALF, DoubleBlockHalf};
use crisp_data::{Block, BlockState, fluid_tags};

use super::BlockBehaviour;
use super::double_plant_block;
use super::seagrass_block::may_place_on_seagrass_support;
use super::vegetation_block::{self, Vegetation};
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::{BlockGetter, LevelAccessor, LevelReader};

pub struct TallSeagrassBlock;

impl Vegetation for TallSeagrassBlock {
    fn may_place_on(&self, state: BlockState, _level: &mut dyn LevelReader, _pos: BlockPos) -> bool {
        may_place_on_seagrass_support(state)
    }
}

impl BlockBehaviour for TallSeagrassBlock {
    fn get_clone_item_stack(&self, _level: &ServerLevel, _pos: BlockPos, _state: BlockState, _include_data: bool) -> ItemStack {
        ItemStack::new(crisp_data::blocks::SEAGRASS.item(), 1)
    }

    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        let state = double_plant_block::get_state_for_placement(block, ctx, level)?;
        let fluid = level.fluid_state(ctx.clicked_pos().above());
        (fluid.is(&fluid_tags::WATER) && fluid.amount == 8).then_some(state)
    }

    fn set_placed_by(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState, _placer: Option<EntityId>, _item: ItemStack) {
        double_plant_block::set_placed_by(level, pos, state);
    }

    fn update_shape(&self, state: BlockState, level: &mut dyn LevelAccessor, pos: BlockPos, direction: Direction, _: BlockPos, neighbour_state: BlockState) -> BlockState {
        double_plant_block::update_shape(state, level, pos, direction, neighbour_state)
    }

    fn can_survive(&self, state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> bool {
        if state.get(DOUBLE_BLOCK_HALF) == DoubleBlockHalf::Upper {
            let below = level.block_state(pos.below());
            return below.is_block(state.block()) && below.get(DOUBLE_BLOCK_HALF) == DoubleBlockHalf::Lower;
        }
        let fluid = level.fluid_state(pos);
        vegetation_block::can_survive(self, level, pos) && fluid.is(&fluid_tags::WATER) && fluid.amount == 8
    }

    fn player_will_destroy(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState, player: EntityId) -> BlockState {
        double_plant_block::player_will_destroy(level, pos, state, player)
    }

    fn player_destroy(&self, level: &mut ServerLevel, player: EntityId, pos: BlockPos, _state: BlockState, _block_entity: Option<&crate::world::level::block::entity::BlockEntity>, tool: Option<ItemStack>) {
        double_plant_block::player_destroy(level, player, pos, tool);
    }
}
