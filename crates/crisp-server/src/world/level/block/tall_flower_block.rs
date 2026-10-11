use crisp_core::{BlockPos, Direction};
use crisp_data::{Block, BlockState};

use super::BlockBehaviour;
use super::double_plant_block::{self, DoublePlantBlock};
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::{LevelAccessor, LevelReader};

pub struct TallFlowerBlock;

impl BlockBehaviour for TallFlowerBlock {
    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        double_plant_block::get_state_for_placement(block, ctx, level)
    }

    fn set_placed_by(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState, _placer: Option<EntityId>, _item: ItemStack) {
        double_plant_block::set_placed_by(level, pos, state);
    }

    fn update_shape(&self, state: BlockState, level: &mut dyn LevelAccessor, pos: BlockPos, direction: Direction, _: BlockPos, neighbour_state: BlockState) -> BlockState {
        double_plant_block::update_shape(state, level, pos, direction, neighbour_state)
    }

    fn can_survive(&self, state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> bool {
        double_plant_block::can_survive(&DoublePlantBlock, state, level, pos)
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
        level.pop_resource(pos, ItemStack::new(state.block().item(), 1));
    }
}
