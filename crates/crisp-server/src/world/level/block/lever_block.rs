use crisp_core::{BlockPos, Direction};
use crisp_data::properties::POWERED;
use crisp_data::{Block, BlockState, registry, registry_id};

use super::face_attached_horizontal_directional_block as face_attached;
use super::{BlockBehaviour, BlockHitResult};
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::InteractionResult;
use crate::world::entity::EntityId;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::{LevelAccessor, LevelReader};

pub struct LeverBlock {
    pub block: Block,
}

impl LeverBlock {
    pub fn pull(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, player: Option<EntityId>) {
        let state = state.cycle(POWERED);
        level.set_block_simple(pos, state);
        self.update_neighbours(state, level, pos);
        play_sound(player, level, pos, state);
    }

    fn update_neighbours(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        let front = face_attached::get_connected_direction(state).opposite();
        level.update_neighbors_at(pos, self.block);
        level.update_neighbors_at(pos.relative(front), self.block);
    }
}

fn play_sound(player: Option<EntityId>, level: &mut ServerLevel, pos: BlockPos, state_after: BlockState) {
    let pitch = if state_after.get(POWERED) { 0.6 } else { 0.5 };
    let sound = registry_id(registry::SOUND_EVENT, "minecraft:block.lever.click");
    level.play_sound(player, pos, sound, SoundSource::Blocks, 0.3, pitch);
}

impl BlockBehaviour for LeverBlock {
    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        face_attached::get_state_for_placement(block, ctx, level)
    }

    fn can_survive(&self, state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> bool {
        face_attached::can_survive(state, level, pos)
    }

    fn update_shape(
        &self,
        state: BlockState,
        level: &mut dyn LevelAccessor,
        pos: BlockPos,
        direction: Direction,
        _neighbour_pos: BlockPos,
        _neighbour_state: BlockState,
    ) -> BlockState {
        face_attached::update_shape(state, level, pos, direction)
    }

    fn use_without_item(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, _player: EntityId, _hit: &BlockHitResult) -> InteractionResult {
        self.pull(state, level, pos, None);
        InteractionResult::SUCCESS
    }

    fn affect_neighbors_after_removal(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, moved_by_piston: bool) {
        if !moved_by_piston && state.get(POWERED) {
            self.update_neighbours(state, level, pos);
        }
    }

    fn get_direct_signal(&self, state: BlockState, _level: &mut dyn LevelReader, _pos: BlockPos, direction: Direction) -> i32 {
        if state.get(POWERED) && face_attached::get_connected_direction(state) == direction { 15 } else { 0 }
    }

    fn own_signal(&self, state: BlockState, _level: &mut dyn LevelReader, _pos: BlockPos) -> i32 {
        if state.get(POWERED) { 15 } else { 0 }
    }
}
