use crisp_core::BlockPos;
use crisp_core::random::RandomSource;
use crisp_data::BlockState;

use super::BlockBehaviour;
use crate::server::level::ServerLevel;
use crate::world::item::ItemStack;

pub struct SpawnerBlock;

impl BlockBehaviour for SpawnerBlock {
    fn spawn_after_break(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos, _tool: Option<ItemStack>, drop_experience: bool) {
        if drop_experience {
            let magic_count = 15 + level.random.next_int_bounded(15) + level.random.next_int_bounded(15);
            level.pop_experience(pos, magic_count);
        }
    }
}
