use crisp_data::BlockState;
use crisp_data::properties::FACING;

use super::BlockBehaviour;
use super::mirror::Mirror;
use super::rotation::Rotation;

pub struct AmethystClusterBlock;

impl BlockBehaviour for AmethystClusterBlock {
    fn rotate(&self, state: BlockState, rotation: Rotation) -> BlockState {
        state.set(FACING, rotation.rotate(state.get(FACING)))
    }

    fn mirror(&self, state: BlockState, mirror: Mirror) -> BlockState {
        super::rotate(state, mirror.get_rotation(state.get(FACING)))
    }

    fn on_projectile_hit(&self, level: &mut crate::server::level::ServerLevel, _state: BlockState, hit_result: &super::BlockHitResult, _projectile: crate::world::entity::EntityId) {
        super::amethyst_block::on_projectile_hit(level, hit_result);
    }
}
