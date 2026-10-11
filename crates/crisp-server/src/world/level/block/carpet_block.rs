use crisp_core::{BlockPos, Direction};
use crisp_data::{BlockState, blocks};

use super::BlockBehaviour;
use crate::world::level::{LevelAccessor, LevelReader};
use crate::world::level::block;

pub struct CarpetBlock;

impl BlockBehaviour for CarpetBlock {
    fn update_shape(
        &self,
        state: BlockState,
        level: &mut dyn LevelAccessor,
        pos: BlockPos,
        _direction: Direction,
        _neighbour_pos: BlockPos,
        _neighbour_state: BlockState,
    ) -> BlockState {
        if block::of(state).can_survive(state, level, pos) { state } else { blocks::AIR.default_state() }
    }

    fn can_survive(&self, _state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> bool {
        !level.block_state(pos.below()).is_air()
    }
}

#[cfg(test)]
mod tests {
    use crisp_core::BlockPos;
    use crisp_data::blocks;

    use crate::server::level::testing;
    use crate::world::level::BlockGetter;
    use crate::world::level::update_flags::UPDATE_ALL;

    #[test]
    fn needs_a_block_below() {
        let mut level = testing::level();
        let pos = BlockPos::new(0, -59, 0);
        level.set_block(pos.below(), blocks::GLASS.default_state(), UPDATE_ALL);
        level.set_block(pos, blocks::RED_CARPET.default_state(), UPDATE_ALL);
        level.set_block(pos.below(), blocks::AIR.default_state(), UPDATE_ALL);
        assert!(level.block_state(pos).is_air());
    }
}
