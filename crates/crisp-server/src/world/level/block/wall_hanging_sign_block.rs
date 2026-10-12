use crisp_core::{BlockPos, Direction};
use crisp_data::properties::{HORIZONTAL_FACING, WATERLOGGED};
use crisp_data::{Block, BlockState, Fluid, SupportType, block_tags, blocks};

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

pub struct WallHangingSignBlock;

impl WallHangingSignBlock {
    fn should_try_to_chain_another_hanging_sign(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, player: EntityId, hit_result: &BlockHitResult, item_stack: &ItemStack) -> bool {
        let slot = level.sign_get_slot_player_is_facing(pos, player);
        let text_filtering_enabled = level.player(player).client.text_filtering;
        !level.sign(pos).is_some_and(|sign| sign.can_execute_click_commands(slot, text_filtering_enabled))
            && item_stack.item.is_instance_of("HangingSignItem")
            && !self.is_hitting_editable_side(hit_result, state)
    }

    fn is_hitting_editable_side(&self, hit_result: &BlockHitResult, state: BlockState) -> bool {
        hit_result.direction.axis() == state.get(HORIZONTAL_FACING).axis()
    }

    pub fn can_place(&self, state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> bool {
        let clockwise = state.get(HORIZONTAL_FACING).clockwise();
        let counter_clockwise = state.get(HORIZONTAL_FACING).counter_clockwise();
        self.can_attach_to(level, state, pos.relative(clockwise), counter_clockwise) || self.can_attach_to(level, state, pos.relative(counter_clockwise), clockwise)
    }

    pub fn can_attach_to(&self, level: &mut dyn LevelReader, state: BlockState, attach_pos: BlockPos, attach_face: Direction) -> bool {
        let attach_state = level.block_state(attach_pos);
        if attach_state.is(&block_tags::WALL_HANGING_SIGNS) {
            attach_state.get(HORIZONTAL_FACING).axis() == state.get(HORIZONTAL_FACING).axis()
        } else {
            attach_state.is_face_sturdy(attach_face, SupportType::Full)
        }
    }
}

impl SignBlock for WallHangingSignBlock {
    fn get_y_rotation_degrees(&self, state: BlockState) -> f32 {
        state.get(HORIZONTAL_FACING).to_y_rot()
    }
}

impl BlockBehaviour for WallHangingSignBlock {
    fn use_item_on(&self, item: ItemStack, state: BlockState, level: &mut ServerLevel, pos: BlockPos, player: EntityId, hand: InteractionHand, hit: &BlockHitResult) -> InteractionResult {
        if level.sign(pos).is_some() && self.should_try_to_chain_another_hanging_sign(state, level, pos, player, hit, &item) {
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

    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        let mut state = block.default_state();
        let replaced_fluid_state = level.fluid_state(ctx.clicked_pos());
        let pos = ctx.clicked_pos();
        for direction in ctx.nearest_looking_directions() {
            if direction.axis().is_horizontal() && direction.axis() != ctx.clicked_face().axis() {
                state = state.set(HORIZONTAL_FACING, direction.opposite());
                if self.can_survive(state, level, pos) && self.can_place(state, level, pos) {
                    return Some(state.set(WATERLOGGED, replaced_fluid_state.fluid == Fluid::Water));
                }
            }
        }
        None
    }

    fn update_shape(&self, state: BlockState, level: &mut dyn LevelAccessor, pos: BlockPos, direction: Direction, _: BlockPos, _: BlockState) -> BlockState {
        if direction.axis() == state.get(HORIZONTAL_FACING).clockwise().axis() && !self.can_survive(state, level, pos) {
            return blocks::AIR.default_state();
        }
        schedule_water_tick(state, level, pos);
        state
    }

    fn rotate(&self, state: BlockState, rotation: Rotation) -> BlockState {
        state.set(HORIZONTAL_FACING, rotation.rotate(state.get(HORIZONTAL_FACING)))
    }

    fn mirror(&self, state: BlockState, mirror: Mirror) -> BlockState {
        super::rotate(state, mirror.get_rotation(state.get(HORIZONTAL_FACING)))
    }
}
