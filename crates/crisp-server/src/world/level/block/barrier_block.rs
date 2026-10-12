use crisp_core::{BlockPos, Direction};
use crisp_data::properties::WATERLOGGED;
use crisp_data::{Block, BlockState, Fluid};

use super::BlockBehaviour;
use super::simple_waterlogged_block::{self, is_water_source, schedule_water_tick};
use crate::server::level::ServerLevel;
use crate::server::level::game_mode::GameType;
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::LevelAccessor;

pub struct BarrierBlock;

fn is_creative(level: &ServerLevel, user: Option<EntityId>) -> bool {
    user.and_then(|user| level.players.get(&user)).is_some_and(|p| p.game_mode.game_type == GameType::Creative)
}

impl BlockBehaviour for BarrierBlock {
    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        Some(block.default_state().set(WATERLOGGED, is_water_source(level, ctx.clicked_pos())))
    }

    fn update_shape(&self, state: BlockState, level: &mut dyn LevelAccessor, pos: BlockPos, _: Direction, _: BlockPos, _: BlockState) -> BlockState {
        schedule_water_tick(state, level, pos);
        state
    }

    fn pickup_block(&self, user: Option<EntityId>, level: &mut ServerLevel, pos: BlockPos, state: BlockState) -> Option<ItemStack> {
        if is_creative(level, user) { simple_waterlogged_block::pickup_block(level, pos, state) } else { None }
    }

    fn can_place_liquid(&self, user: Option<EntityId>, level: &mut ServerLevel, _pos: BlockPos, _state: BlockState, fluid: Fluid) -> bool {
        is_creative(level, user) && simple_waterlogged_block::can_place_liquid(fluid)
    }
}
