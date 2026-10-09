use crisp_core::{BlockPos, Direction};
use crisp_data::{BlockState, blocks};

use super::{BlockBehaviour, base_fire_block};
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::inside_block::InsideBlockEffects;
use crate::world::level::BlockGetter;

const FIRE_DAMAGE: f32 = 2.0;

pub struct SoulFireBlock;

impl BlockBehaviour for SoulFireBlock {
    fn update_shape(
        &self,
        state: BlockState,
        level: &mut ServerLevel,
        pos: BlockPos,
        _direction: Direction,
        _neighbour_pos: BlockPos,
        _neighbour_state: BlockState,
    ) -> BlockState {
        if self.can_survive(state, level, pos) { state } else { blocks::AIR.default_state() }
    }

    fn can_survive(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos) -> bool {
        base_fire_block::can_survive_on_soul_block(level.block_state(pos.below()))
    }

    fn on_place(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, old_state: BlockState, _moved_by_piston: bool) {
        base_fire_block::on_place(state, level, pos, old_state);
    }

    fn entity_inside(&self, _state: BlockState, _level: &mut ServerLevel, _pos: BlockPos, _entity: EntityId, effects: &mut InsideBlockEffects, _is_precise: bool) {
        base_fire_block::entity_inside(effects, FIRE_DAMAGE);
    }

    fn player_will_destroy(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState, _player: EntityId) -> BlockState {
        base_fire_block::player_will_destroy(level, pos, state)
    }
}
