use crisp_core::{BlockPos, Direction};
use crisp_data::properties::{DELAY, HORIZONTAL_FACING, LOCKED};
use crisp_data::{Block, BlockState, blocks};

use super::diode_block::{self, DiodeBlock};
use super::{BlockBehaviour, BlockHitResult};
use crate::server::level::ServerLevel;
use crate::world::InteractionResult;
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;
use crate::world::item::context::BlockPlaceContext;

pub struct RepeaterBlock {
    pub block: Block,
}

impl DiodeBlock for RepeaterBlock {
    fn block(&self) -> Block {
        self.block
    }

    fn get_delay(&self, state: BlockState) -> i32 {
        state.get(DELAY) * 2
    }

    fn is_locked(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState) -> bool {
        self.get_alternate_signal(level, pos, state) > 0
    }

    fn side_input_diodes_only(&self) -> bool {
        true
    }
}

impl BlockBehaviour for RepeaterBlock {
    fn use_without_item(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, player: EntityId, _hit: &BlockHitResult) -> InteractionResult {
        if !level.may_build(player) {
            return InteractionResult::Pass;
        }
        level.set_block_simple(pos, state.cycle(DELAY));
        InteractionResult::SUCCESS
    }

    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        let state = diode_block::get_state_for_placement(block, ctx);
        Some(state.set(LOCKED, self.is_locked(level, ctx.clicked_pos(), state)))
    }

    fn update_shape(
        &self,
        state: BlockState,
        level: &mut ServerLevel,
        pos: BlockPos,
        direction: Direction,
        _neighbour_pos: BlockPos,
        neighbour_state: BlockState,
    ) -> BlockState {
        if direction == Direction::Down && !diode_block::can_survive_on(neighbour_state) {
            blocks::AIR.default_state()
        } else if direction.axis() != state.get(HORIZONTAL_FACING).axis() {
            state.set(LOCKED, self.is_locked(level, pos, state))
        } else {
            state
        }
    }

    fn should_redstone_wire_connect_to(&self, state: BlockState, _level: &mut ServerLevel, _pos: BlockPos, direction: Option<Direction>) -> bool {
        let repeater_direction = state.get(HORIZONTAL_FACING);
        Some(repeater_direction) == direction || Some(repeater_direction.opposite()) == direction
    }

    fn can_survive(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos) -> bool {
        diode_block::can_survive(level, pos)
    }

    fn tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        diode_block::tick(self, state, level, pos);
    }

    fn neighbor_changed(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, _block: Block, _moved_by_piston: bool) {
        diode_block::neighbor_changed(self, state, level, pos);
    }

    fn own_signal(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) -> i32 {
        diode_block::own_signal(self, state, level, pos)
    }

    fn get_direct_signal(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, direction: Direction) -> i32 {
        super::of(state).get_signal(state, level, pos, direction)
    }

    fn get_signal(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, direction: Direction) -> i32 {
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
