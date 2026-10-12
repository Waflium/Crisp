use crisp_core::shapes::{VoxelShape, is_face_full};
use crisp_core::{BlockPos, Direction};
use crisp_data::properties::{ATTACHED, HORIZONTAL_FACING, ROTATION_16, WATERLOGGED};
use crisp_data::{Block, BlockState, Fluid, SupportType, block_tags, blocks};

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

pub struct CeilingHangingSignBlock;

impl CeilingHangingSignBlock {
    fn should_try_to_chain_another_hanging_sign(&self, level: &mut ServerLevel, pos: BlockPos, player: EntityId, hit_result: &BlockHitResult, item_stack: &ItemStack) -> bool {
        let slot = level.sign_get_slot_player_is_facing(pos, player);
        let text_filtering_enabled = level.player(player).client.text_filtering;
        !level.sign(pos).is_some_and(|sign| sign.can_execute_click_commands(slot, text_filtering_enabled))
            && item_stack.item.is_instance_of("HangingSignItem")
            && hit_result.direction == Direction::Down
    }
}

impl SignBlock for CeilingHangingSignBlock {
    fn get_y_rotation_degrees(&self, state: BlockState) -> f32 {
        rotation_segment::convert_to_degrees(state.get(ROTATION_16))
    }
}

impl BlockBehaviour for CeilingHangingSignBlock {
    fn use_item_on(&self, item: ItemStack, _state: BlockState, level: &mut ServerLevel, pos: BlockPos, player: EntityId, hand: InteractionHand, hit: &BlockHitResult) -> InteractionResult {
        if level.sign(pos).is_some() && self.should_try_to_chain_another_hanging_sign(level, pos, player, hit, &item) {
            return InteractionResult::Pass;
        }
        sign_block::use_item_on(item, level, pos, player, hand)
    }

    fn use_without_item(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos, player: EntityId, _hit: &BlockHitResult) -> InteractionResult {
        sign_block::use_without_item(level, pos, player)
    }

    fn set_placed_by(&self, level: &mut ServerLevel, pos: BlockPos, _state: BlockState, placer: Option<EntityId>, _item: ItemStack) {
        sign_block::set_placed_by(level, pos, placer);
    }

    fn can_survive(&self, _state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> bool {
        level.block_state(pos.above()).is_face_sturdy(Direction::Down, SupportType::Center)
    }

    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        let replaced_fluid_state = level.fluid_state(ctx.clicked_pos());
        let above = ctx.clicked_pos().above();
        let state_above = level.block_state(above);
        let is_below_hanging_sign = state_above.is(&block_tags::ALL_HANGING_SIGNS);
        let direction = Direction::from_y_rot(ctx.rotation() as f64);
        let mut attached_to_middle =
            !is_face_full(&VoxelShape::from_boxes(state_above.collision_shape()), Direction::Down) || ctx.is_secondary_use_active();
        if is_below_hanging_sign && !ctx.is_secondary_use_active() {
            if state_above.has(HORIZONTAL_FACING) {
                let above_direction = state_above.get(HORIZONTAL_FACING);
                if above_direction.axis() == direction.axis() {
                    attached_to_middle = false;
                }
            } else if state_above.has(ROTATION_16)
                && let Some(above_direction) = rotation_segment::convert_to_direction(state_above.get(ROTATION_16))
                && above_direction.axis() == direction.axis()
            {
                attached_to_middle = false;
            }
        }
        let rotation_segment = if !attached_to_middle {
            rotation_segment::convert_to_segment_from_direction(direction.opposite())
        } else {
            rotation_segment::convert_to_segment(ctx.rotation() + 180.0)
        };
        Some(
            block
                .default_state()
                .set(ATTACHED, attached_to_middle)
                .set(ROTATION_16, rotation_segment)
                .set(WATERLOGGED, replaced_fluid_state.fluid == Fluid::Water),
        )
    }

    fn update_shape(&self, state: BlockState, level: &mut dyn LevelAccessor, pos: BlockPos, direction: Direction, _: BlockPos, _: BlockState) -> BlockState {
        if direction == Direction::Up && !self.can_survive(state, level, pos) {
            return blocks::AIR.default_state();
        }
        schedule_water_tick(state, level, pos);
        state
    }

    fn rotate(&self, state: BlockState, rotation: Rotation) -> BlockState {
        state.set(ROTATION_16, rotation.rotate_index(state.get(ROTATION_16), 16))
    }

    fn mirror(&self, state: BlockState, mirror: Mirror) -> BlockState {
        state.set(ROTATION_16, mirror.mirror_index(state.get(ROTATION_16), 16))
    }
}
