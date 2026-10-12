use crisp_core::BlockPos;
use crisp_data::{BlockState, blocks};

use super::BlockBehaviour;
use crate::server::level::ServerLevel;
use crate::world::level::BlockGetter;
use crate::world::level::update_flags::UPDATE_ALL;

pub struct RootedDirtBlock;

impl BlockBehaviour for RootedDirtBlock {
    fn is_valid_bonemeal_target(&self, level: &mut ServerLevel, pos: BlockPos, _state: BlockState) -> bool {
        level.block_state(pos.below()).is_air() && level.is_inside_build_height(pos.y - 1)
    }

    fn is_bonemeal_success(&self, _level: &mut ServerLevel, _pos: BlockPos, _state: BlockState) -> bool {
        true
    }

    fn perform_bonemeal(&self, level: &mut ServerLevel, pos: BlockPos, _state: BlockState) {
        level.set_block(pos.below(), blocks::HANGING_ROOTS.default_state(), UPDATE_ALL);
    }
}
