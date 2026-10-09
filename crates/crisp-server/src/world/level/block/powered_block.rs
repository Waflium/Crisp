use crisp_core::BlockPos;
use crisp_data::BlockState;

use super::BlockBehaviour;
use crate::server::level::ServerLevel;

pub struct PoweredBlock;

impl BlockBehaviour for PoweredBlock {
    fn own_signal(&self, _state: BlockState, _level: &mut ServerLevel, _pos: BlockPos) -> i32 {
        15
    }
}
