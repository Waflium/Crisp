use crisp_core::{BlockPos, Direction};
use crisp_data::properties::{COPPER_GOLEM_POSE, CopperGolemPose, HORIZONTAL_FACING, WATERLOGGED};
use crisp_data::{Block, BlockState, Fluid, block_tags, item_tags, items, registry, registry_id};

use super::mirror::Mirror;
use super::rotation::Rotation;
use super::simple_waterlogged_block::schedule_water_tick;
use super::weathering_copper::{self, WeatherState};
use super::{BlockBehaviour, BlockHitResult};
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;
use crate::world::item::component::{self as dc, values::block_state_properties_nbt};
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::update_flags::UPDATE_ALL;
use crate::world::level::{BlockGetter, LevelAccessor};
use crate::world::{InteractionHand, InteractionResult};

fn get_next_pose(pose: CopperGolemPose) -> CopperGolemPose {
    CopperGolemPose::VALUES.get(pose as usize + 1).copied().unwrap_or(CopperGolemPose::Standing)
}

pub struct CopperGolemStatueBlock;

impl CopperGolemStatueBlock {
    pub fn update_pose(&self, level: &mut ServerLevel, state: BlockState, pos: BlockPos) {
        level.play_sound(None, pos, registry_id(registry::SOUND_EVENT, "minecraft:entity.copper_golem_become_statue"), SoundSource::Blocks, 1.0, 1.0);
        level.set_block(pos, state.set(COPPER_GOLEM_POSE, get_next_pose(state.get(COPPER_GOLEM_POSE))), UPDATE_ALL);
    }
}

impl BlockBehaviour for CopperGolemStatueBlock {
    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        let replaced_fluid_state = level.fluid_state(ctx.clicked_pos());
        Some(block.default_state().set(HORIZONTAL_FACING, ctx.horizontal_direction().opposite()).set(WATERLOGGED, replaced_fluid_state.fluid == Fluid::Water))
    }

    fn rotate(&self, state: BlockState, rotation: Rotation) -> BlockState {
        state.set(HORIZONTAL_FACING, rotation.rotate(state.get(HORIZONTAL_FACING)))
    }

    fn mirror(&self, state: BlockState, mirror: Mirror) -> BlockState {
        super::rotate(state, mirror.get_rotation(state.get(HORIZONTAL_FACING)))
    }

    fn use_item_on(&self, item_stack: ItemStack, state: BlockState, level: &mut ServerLevel, pos: BlockPos, _player: EntityId, _hand: InteractionHand, _hit: &BlockHitResult) -> InteractionResult {
        if item_stack.get_item().is(&item_tags::AXES) {
            return InteractionResult::Pass;
        }
        self.update_pose(level, state, pos);
        InteractionResult::SUCCESS
    }

    fn should_changed_state_keep_block_entity(&self, old_state: BlockState) -> bool {
        old_state.is(&block_tags::COPPER_GOLEM_STATUES)
    }

    fn get_analog_output_signal(&self, state: BlockState, _level: &mut ServerLevel, _pos: BlockPos, _direction: Direction) -> i32 {
        state.get(COPPER_GOLEM_POSE) as i32 + 1
    }

    fn get_clone_item_stack(&self, level: &ServerLevel, pos: BlockPos, state: BlockState, _include_data: bool) -> ItemStack {
        let mut item_stack = ItemStack::new(state.block().item(), 1);
        let Some(entity) = level.chunks.block_entity_ref(pos) else { return item_stack };
        for (ty, value) in entity.collect_components() {
            item_stack.set(ty, value);
        }
        item_stack.set(dc::BLOCK_STATE, Some(block_state_properties_nbt(&[("copper_golem_pose", state.get(COPPER_GOLEM_POSE).name())])));
        item_stack
    }

    fn affect_neighbors_after_removal(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, _moved_by_piston: bool) {
        level.update_neighbour_for_output_signal(pos, state.block());
    }

    fn update_shape(&self, state: BlockState, level: &mut dyn LevelAccessor, pos: BlockPos, _: Direction, _: BlockPos, _: BlockState) -> BlockState {
        schedule_water_tick(state, level, pos);
        state
    }
}

pub struct WeatheringCopperGolemStatueBlock;

impl BlockBehaviour for WeatheringCopperGolemStatueBlock {
    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        CopperGolemStatueBlock.get_state_for_placement(block, ctx, level)
    }

    fn rotate(&self, state: BlockState, rotation: Rotation) -> BlockState {
        CopperGolemStatueBlock.rotate(state, rotation)
    }

    fn mirror(&self, state: BlockState, mirror: Mirror) -> BlockState {
        CopperGolemStatueBlock.mirror(state, mirror)
    }

    fn random_tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        weathering_copper::change_over_time(state, level, pos);
    }

    fn use_item_on(&self, item_stack: ItemStack, state: BlockState, level: &mut ServerLevel, pos: BlockPos, player: EntityId, hand: InteractionHand, _hit: &BlockHitResult) -> InteractionResult {
        if level.get_block_entity(pos).is_none() {
            return InteractionResult::Pass;
        }
        if !item_stack.get_item().is(&item_tags::AXES) {
            if item_stack.is(items::HONEYCOMB) {
                return InteractionResult::Pass;
            }
            CopperGolemStatueBlock.update_pose(level, state, pos);
            return InteractionResult::SUCCESS;
        }
        if weathering_copper::age(state.block()) == Some(WeatherState::Unaffected) {
            let slot = if hand == InteractionHand::MainHand { crisp_data::EquipmentSlot::Mainhand } else { crisp_data::EquipmentSlot::Offhand };
            level.hurt_and_break(player, slot, 1);
        }
        InteractionResult::Pass
    }

    fn should_changed_state_keep_block_entity(&self, old_state: BlockState) -> bool {
        CopperGolemStatueBlock.should_changed_state_keep_block_entity(old_state)
    }

    fn get_analog_output_signal(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, direction: Direction) -> i32 {
        CopperGolemStatueBlock.get_analog_output_signal(state, level, pos, direction)
    }

    fn get_clone_item_stack(&self, level: &ServerLevel, pos: BlockPos, state: BlockState, include_data: bool) -> ItemStack {
        CopperGolemStatueBlock.get_clone_item_stack(level, pos, state, include_data)
    }

    fn affect_neighbors_after_removal(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, moved_by_piston: bool) {
        CopperGolemStatueBlock.affect_neighbors_after_removal(state, level, pos, moved_by_piston);
    }

    fn update_shape(&self, state: BlockState, level: &mut dyn LevelAccessor, pos: BlockPos, direction: Direction, neighbour_pos: BlockPos, neighbour_state: BlockState) -> BlockState {
        CopperGolemStatueBlock.update_shape(state, level, pos, direction, neighbour_pos, neighbour_state)
    }
}
