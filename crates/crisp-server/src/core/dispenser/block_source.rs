use crisp_core::{BlockPos, Direction, Vec3};
use crisp_data::BlockState;
use crisp_data::properties::FACING;

#[derive(Clone, Copy)]
pub struct BlockSource {
    pub pos: BlockPos,
    pub state: BlockState,
}

impl BlockSource {
    pub fn center(&self) -> Vec3 {
        Vec3::at_center_of(self.pos)
    }

    pub fn facing(&self) -> Direction {
        self.state.get(FACING)
    }
}
