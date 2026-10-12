use crisp_core::{BlockPos, Direction};
use crisp_data::properties::{ComparatorMode, HORIZONTAL_FACING, MODE_COMPARATOR, POWERED};
use crisp_data::{Block, BlockState, blocks, registry, registry_id};

use super::diode_block::{self, DiodeBlock};
use super::entity::BlockEntityKind;
use super::horizontal_directional_block;
use super::mirror::Mirror;
use super::rotation::Rotation;
use super::{BlockBehaviour, BlockHitResult};
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::InteractionResult;
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::{BlockGetter, LevelAccessor, LevelReader};
use crate::world::level::update_flags::UPDATE_CLIENTS;
use crate::world::ticks::TickPriority;

pub const MODE: crisp_data::properties::EnumProperty<ComparatorMode> = MODE_COMPARATOR;

pub struct ComparatorBlock {
    pub block: Block,
}

fn block_entity_output(level: &mut dyn LevelReader, pos: BlockPos) -> Option<i32> {
    match level.get_block_entity(pos).map(|entity| &entity.kind) {
        Some(BlockEntityKind::Comparator(comparator)) => Some(comparator.get_output_signal()),
        _ => None,
    }
}

impl ComparatorBlock {
    fn calculate_output_signal(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState) -> i32 {
        let input_signal = self.get_input_signal(level, pos, state);
        if input_signal == 0 {
            return 0;
        }
        let alternate_signal = self.get_alternate_signal(level, pos, state);
        if alternate_signal > input_signal {
            0
        } else if state.get(MODE) == ComparatorMode::Subtract {
            input_signal - alternate_signal
        } else {
            input_signal
        }
    }

    fn refresh_output_state(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState) {
        let output_value = self.calculate_output_signal(level, pos, state);
        let mut old_value = 0;
        if let Some(entity) = level.get_block_entity(pos)
            && let BlockEntityKind::Comparator(comparator) = &mut entity.kind
        {
            old_value = comparator.get_output_signal();
            comparator.set_output_signal(output_value);
        }
        if old_value != output_value || state.get(MODE) == ComparatorMode::Compare {
            let source_on = self.should_turn_on(level, pos, state);
            let is_on = state.get(POWERED);
            if is_on && !source_on {
                level.set_block(pos, state.set(POWERED, false), UPDATE_CLIENTS);
            } else if !is_on && source_on {
                level.set_block(pos, state.set(POWERED, true), UPDATE_CLIENTS);
            }
            self.update_neighbors_in_front(level, pos, state);
        }
    }
}

impl DiodeBlock for ComparatorBlock {
    fn block(&self) -> Block {
        self.block
    }

    fn get_delay(&self, _state: BlockState) -> i32 {
        2
    }

    fn get_output_signal(&self, level: &mut dyn LevelReader, pos: BlockPos, _state: BlockState) -> i32 {
        block_entity_output(level, pos).unwrap_or(0)
    }

    fn should_turn_on(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState) -> bool {
        let input = self.get_input_signal(level, pos, state);
        if input == 0 {
            return false;
        }
        let side_input = self.get_alternate_signal(level, pos, state);
        input > side_input || input == side_input && state.get(MODE) == ComparatorMode::Compare
    }

    fn get_input_signal(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState) -> i32 {
        let mut result_signal = diode_block::get_input_signal(level, pos, state);
        let direction = state.get(HORIZONTAL_FACING);
        let mut target_pos = pos.relative(direction);
        let mut target_state = level.block_state(target_pos);
        if target_state.has_analog_output_signal() {
            result_signal = super::of(target_state).get_analog_output_signal(target_state, level, target_pos, direction.opposite());
        } else if result_signal < 15 && target_state.is_redstone_conductor() {
            target_pos = target_pos.relative(direction);
            target_state = level.block_state(target_pos);
            if target_state.has_analog_output_signal() {
                result_signal = super::of(target_state).get_analog_output_signal(target_state, level, target_pos, direction.opposite());
            }
        }
        result_signal
    }

    fn check_tick_on_neighbor(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState) {
        if !level.will_tick_this_tick(pos, self.block) {
            let output_value = self.calculate_output_signal(level, pos, state);
            let old_value = block_entity_output(level, pos).unwrap_or(0);
            if output_value != old_value || state.get(POWERED) != self.should_turn_on(level, pos, state) {
                let priority = if diode_block::should_prioritize(level, pos, state) { TickPriority::HIGH } else { TickPriority::NORMAL };
                level.schedule_tick_with_priority(pos, self.block, 2, priority);
            }
        }
    }
}

impl BlockBehaviour for ComparatorBlock {
    fn rotate(&self, state: BlockState, rotation: Rotation) -> BlockState {
        horizontal_directional_block::rotate(state, rotation)
    }

    fn mirror(&self, state: BlockState, mirror: Mirror) -> BlockState {
        horizontal_directional_block::mirror(state, mirror)
    }

    fn use_without_item(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, player: EntityId, _hit: &BlockHitResult) -> InteractionResult {
        if !level.may_build(player) {
            return InteractionResult::Pass;
        }
        let state = state.cycle(MODE);
        let pitch = if state.get(MODE) == ComparatorMode::Subtract { 0.55 } else { 0.5 };
        level.play_sound(Some(player), pos, registry_id(registry::SOUND_EVENT, "minecraft:block.comparator.click"), SoundSource::Blocks, 0.3, pitch);
        level.set_block(pos, state, UPDATE_CLIENTS);
        if level.block_state(pos).is_block(self.block) {
            self.refresh_output_state(level, pos, state);
        }
        InteractionResult::SUCCESS
    }

    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, _level: &mut ServerLevel) -> Option<BlockState> {
        Some(diode_block::get_state_for_placement(block, ctx))
    }

    fn update_shape(
        &self,
        state: BlockState,
        _level: &mut dyn LevelAccessor,
        _pos: BlockPos,
        direction: Direction,
        _neighbour_pos: BlockPos,
        neighbour_state: BlockState,
    ) -> BlockState {
        if direction == Direction::Down && !diode_block::can_survive_on(neighbour_state) { blocks::AIR.default_state() } else { state }
    }

    fn can_survive(&self, _state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> bool {
        diode_block::can_survive(level, pos)
    }

    fn tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        self.refresh_output_state(level, pos, state);
    }

    fn neighbor_changed(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, _block: Block, _moved_by_piston: bool) {
        diode_block::neighbor_changed(self, state, level, pos);
    }

    fn own_signal(&self, state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> i32 {
        diode_block::own_signal(self, state, level, pos)
    }

    fn get_direct_signal(&self, state: BlockState, level: &mut dyn LevelReader, pos: BlockPos, direction: Direction) -> i32 {
        super::of(state).get_signal(state, level, pos, direction)
    }

    fn get_signal(&self, state: BlockState, level: &mut dyn LevelReader, pos: BlockPos, direction: Direction) -> i32 {
        diode_block::get_signal(state, level, pos, direction)
    }

    fn set_placed_by(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState, _placer: Option<EntityId>, _item: ItemStack) {
        diode_block::set_placed_by(self, level, pos, state);
    }

    fn on_place(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, _old_state: BlockState, _moved_by_piston: bool) {
        self.update_neighbors_in_front(level, pos, state);
    }

    fn affect_neighbors_after_removal(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, moved_by_piston: bool) {
        diode_block::affect_neighbors_after_removal(self, state, level, pos, moved_by_piston);
    }
}
