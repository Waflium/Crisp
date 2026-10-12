use crisp_core::{BlockPos, Direction};
use crisp_data::properties::{ROTATION_16, WATERLOGGED};
use crisp_data::{Block, BlockState, Fluid, blocks};

use super::mirror::Mirror;
use super::rotation::Rotation;
use super::rotation_segment;
use super::sign_block::{self, SignBlock};
use super::simple_waterlogged_block::schedule_water_tick;
use super::{BlockBehaviour, BlockHitResult};
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::{BlockGetter, LevelAccessor, LevelReader};
use crate::world::{InteractionHand, InteractionResult};

pub struct StandingSignBlock;

impl SignBlock for StandingSignBlock {
    fn get_y_rotation_degrees(&self, state: BlockState) -> f32 {
        rotation_segment::convert_to_degrees(state.get(ROTATION_16))
    }
}

impl BlockBehaviour for StandingSignBlock {
    fn can_survive(&self, _state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> bool {
        level.block_state(pos.below()).is_solid()
    }

    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        let replaced_fluid_state = level.fluid_state(ctx.clicked_pos());
        Some(
            block
                .default_state()
                .set(ROTATION_16, rotation_segment::convert_to_segment(ctx.rotation() + 180.0))
                .set(WATERLOGGED, replaced_fluid_state.fluid == Fluid::Water),
        )
    }

    fn update_shape(&self, state: BlockState, level: &mut dyn LevelAccessor, pos: BlockPos, direction: Direction, _: BlockPos, _: BlockState) -> BlockState {
        if direction == Direction::Down && !self.can_survive(state, level, pos) {
            return blocks::AIR.default_state();
        }
        schedule_water_tick(state, level, pos);
        state
    }

    fn use_item_on(&self, item: ItemStack, _state: BlockState, level: &mut ServerLevel, pos: BlockPos, player: EntityId, hand: InteractionHand, _hit: &BlockHitResult) -> InteractionResult {
        sign_block::use_item_on(item, level, pos, player, hand)
    }

    fn use_without_item(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos, player: EntityId, _hit: &BlockHitResult) -> InteractionResult {
        sign_block::use_without_item(level, pos, player)
    }

    fn set_placed_by(&self, level: &mut ServerLevel, pos: BlockPos, _state: BlockState, placer: Option<EntityId>, _item: ItemStack) {
        sign_block::set_placed_by(level, pos, placer);
    }

    fn rotate(&self, state: BlockState, rotation: Rotation) -> BlockState {
        state.set(ROTATION_16, rotation.rotate_index(state.get(ROTATION_16), 16))
    }

    fn mirror(&self, state: BlockState, mirror: Mirror) -> BlockState {
        state.set(ROTATION_16, mirror.mirror_index(state.get(ROTATION_16), 16))
    }
}
