use crisp_core::{BlockPos, Direction};
use crisp_data::blocks;

use crate::server::level::world_gen_region::WorldGenRegion;
use crate::world::level::block::vine_block;

pub fn place(level: &mut WorldGenRegion, origin: BlockPos) -> bool {
    if !level.get_block_state(origin).is_air() {
        return false;
    }
    for direction in Direction::VALUES {
        if direction != Direction::Down && vine_block::is_acceptable_neighbour(level, origin.relative(direction), direction) {
            level.set_block(origin, blocks::VINE.default_state().set(vine_block::property_for_face(direction), true), 2);
            return true;
        }
    }
    false
}
