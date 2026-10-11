use crisp_core::{BlockPos, Direction};
use crisp_data::properties::BERRIES;
use crisp_data::{Block, BlockState, blocks};

use super::growing_plant_body_block::{BodyParts, GrowingPlantBodyBlock};
use super::{BlockBehaviour, BlockHitResult, cave_vines};
use crate::server::level::ServerLevel;
use crate::world::InteractionResult;
use crate::world::entity::EntityId;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::update_flags::UPDATE_CLIENTS;

pub struct CaveVinesPlantParts;

impl BodyParts for CaveVinesPlantParts {
    fn head_block(&self) -> Block {
        blocks::CAVE_VINES
    }

    fn head_can_grow_into(&self, state: BlockState) -> bool {
        state.is_air()
    }

    fn update_head_after_converted_from_body(&self, body_state: BlockState, head_state: BlockState) -> BlockState {
        head_state.set(BERRIES, body_state.get(BERRIES))
    }
}

pub struct CaveVinesPlantBlock(pub GrowingPlantBodyBlock<CaveVinesPlantParts>);

impl BlockBehaviour for CaveVinesPlantBlock {
    fn get_clone_item_stack(&self, _level: &ServerLevel, _pos: BlockPos, _state: BlockState, _include_data: bool) -> crate::world::item::ItemStack {
        crate::world::item::ItemStack::new(crisp_data::items::GLOW_BERRIES, 1)
    }

    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        self.0.get_state_for_placement(block, ctx, level)
    }

    fn can_be_replaced(&self, state: BlockState, ctx: &BlockPlaceContext) -> bool {
        self.0.can_be_replaced(state, ctx)
    }

    fn can_survive(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) -> bool {
        self.0.can_survive(state, level, pos)
    }

    fn tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        self.0.tick(state, level, pos);
    }

    fn update_shape(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, direction: Direction, neighbour_pos: BlockPos, neighbour_state: BlockState) -> BlockState {
        self.0.update_shape(state, level, pos, direction, neighbour_pos, neighbour_state)
    }

    fn use_without_item(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, player: EntityId, _hit: &BlockHitResult) -> InteractionResult {
        cave_vines::use_vine(Some(player), state, level, pos)
    }

    fn is_valid_bonemeal_target(&self, _level: &mut ServerLevel, _pos: BlockPos, state: BlockState) -> bool {
        !state.get(BERRIES)
    }

    fn is_bonemeal_success(&self, _level: &mut ServerLevel, _pos: BlockPos, _state: BlockState) -> bool {
        true
    }

    fn perform_bonemeal(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState) {
        level.set_block(pos, state.set(BERRIES, true), UPDATE_CLIENTS);
    }
}
