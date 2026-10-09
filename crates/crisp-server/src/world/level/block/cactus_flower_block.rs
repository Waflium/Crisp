use crisp_core::{BlockPos, Direction};
use crisp_data::{BlockState, SupportType, block_tags};

use super::BlockBehaviour;
use super::vegetation_block::{self, Vegetation};
use crate::server::level::ServerLevel;
use crate::world::level::BlockGetter;

pub struct CactusFlowerBlock;

impl Vegetation for CactusFlowerBlock {
    fn may_place_on(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos) -> bool {
        let block_below = level.block_state(pos);
        block_below.is(&block_tags::SUPPORT_OVERRIDE_CACTUS_FLOWER) || block_below.is_face_sturdy(Direction::Up, SupportType::Center)
    }
}

impl BlockBehaviour for CactusFlowerBlock {
    fn update_shape(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, _: Direction, _: BlockPos, _: BlockState) -> BlockState {
        vegetation_block::update_shape(state, level, pos)
    }

    fn can_survive(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos) -> bool {
        vegetation_block::can_survive(self, level, pos)
    }
}
