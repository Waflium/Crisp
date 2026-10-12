use crisp_core::BlockPos;

pub fn pack_difference_in_position(pos: BlockPos, test_pos: BlockPos, x_radius: i32, y_radius: i32, z_radius: i32) -> i32 {
    assert!(x_radius <= 127 && y_radius <= 127 && z_radius <= 127, "The radius cannot be larger than 127 when packing the difference between two positions");
    let x_diff = check_difference_in_bounds(test_pos.x, pos.x);
    let y_diff = check_difference_in_bounds(test_pos.y, pos.y);
    let z_diff = check_difference_in_bounds(test_pos.z, pos.z);
    pack_difference_in_position_internal(x_radius, y_radius, z_radius, x_diff, y_diff, z_diff)
}

fn pack_difference_in_position_internal(x_radius: i32, y_radius: i32, z_radius: i32, x_diff: i32, y_diff: i32, z_diff: i32) -> i32 {
    (x_diff + x_radius & 0xFF) << 16 | (y_diff + y_radius & 0xFF) << 8 | z_diff + z_radius & 0xFF
}

fn check_difference_in_bounds(from: i32, to: i32) -> i32 {
    let difference = from - to;
    assert!((-127..=127).contains(&difference), "The difference must be within [127,-127]");
    difference
}
