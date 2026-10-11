use crisp_core::BlockPos;
use crisp_core::shapes::BitSetDiscreteVoxelShape;

use crate::server::level::world_gen_region::WorldGenRegion;
use crate::world::level::block;

pub fn update_shape_at_edge(level: &mut WorldGenRegion, update_mode: i32, shape: &BitSetDiscreteVoxelShape, start_x: i32, start_y: i32, start_z: i32) {
    shape.for_all_faces(|direction, x, y, z| {
        let pos = BlockPos::new(start_x + x, start_y + y, start_z + z);
        let neighbor_pos = pos.relative(direction);
        let state = level.get_block_state(pos);
        let neighbor_state = level.get_block_state(neighbor_pos);
        let new_state = block::of(state).update_shape(state, level, pos, direction, neighbor_pos, neighbor_state);
        if state != new_state {
            level.set_block(pos, new_state, update_mode & -2);
        }
        let new_neighbor_state = block::of(neighbor_state).update_shape(neighbor_state, level, neighbor_pos, direction.opposite(), pos, new_state);
        if neighbor_state != new_neighbor_state {
            level.set_block(neighbor_pos, new_neighbor_state, update_mode & -2);
        }
    });
}
