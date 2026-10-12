use crisp_core::{BlockPos, Direction, Vec3};
use crisp_data::properties::{HORIZONTAL_FACING, WATERLOGGED};
use crisp_data::{Block, BlockState, Fluid, blocks};

use super::mirror::Mirror;
use super::rotation::Rotation;
use super::sign_block::{self, SignBlock};
use super::simple_waterlogged_block::schedule_water_tick;
use super::{BlockBehaviour, BlockHitResult};
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::{BlockGetter, LevelAccessor, LevelReader};
use crate::world::{InteractionHand, InteractionResult};

pub struct WallSignBlock;

impl SignBlock for WallSignBlock {
    fn get_y_rotation_degrees(&self, state: BlockState) -> f32 {
        state.get(HORIZONTAL_FACING).to_y_rot()
    }

    fn get_sign_hitbox_center_position(&self, state: BlockState) -> Vec3 {
        let (y, near, far) = (8.5 / 16.0, 1.0 / 16.0, 15.0 / 16.0);
        match state.get(HORIZONTAL_FACING) {
            Direction::East => Vec3::new(near, y, 0.5),
            Direction::South => Vec3::new(0.5, y, near),
            Direction::West => Vec3::new(far, y, 0.5),
            _ => Vec3::new(0.5, y, far),
        }
    }
}

impl BlockBehaviour for WallSignBlock {
    fn can_survive(&self, state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> bool {
        level.block_state(pos.relative(state.get(HORIZONTAL_FACING).opposite())).is_solid()
    }

    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        let mut state = block.default_state();
        let replaced_fluid_state = level.fluid_state(ctx.clicked_pos());
        let pos = ctx.clicked_pos();
        for direction in ctx.nearest_looking_directions() {
            if direction.axis().is_horizontal() {
                state = state.set(HORIZONTAL_FACING, direction.opposite());
                if self.can_survive(state, level, pos) {
                    return Some(state.set(WATERLOGGED, replaced_fluid_state.fluid == Fluid::Water));
                }
            }
        }
        None
    }

    fn update_shape(&self, state: BlockState, level: &mut dyn LevelAccessor, pos: BlockPos, direction: Direction, _: BlockPos, _: BlockState) -> BlockState {
        if direction.opposite() == state.get(HORIZONTAL_FACING) && !self.can_survive(state, level, pos) {
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
        state.set(HORIZONTAL_FACING, rotation.rotate(state.get(HORIZONTAL_FACING)))
    }

    fn mirror(&self, state: BlockState, mirror: Mirror) -> BlockState {
        super::rotate(state, mirror.get_rotation(state.get(HORIZONTAL_FACING)))
    }
}
