use crisp_core::{Axis, AxisDirection, BlockPos, Direction};

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

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FoundRectangle {
    pub min_corner: BlockPos,
    pub axis1_size: i32,
    pub axis2_size: i32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct IntBounds {
    pub min: i32,
    pub max: i32,
}

pub fn get_largest_rectangle_around(center: BlockPos, axis1: Axis, limit1: i32, axis2: Axis, limit2: i32, mut test: impl FnMut(BlockPos) -> bool) -> FoundRectangle {
    let negative_direction1 = Direction::from_axis_and_direction(axis1, AxisDirection::Negative);
    let positive_direction1 = negative_direction1.opposite();
    let negative_direction2 = Direction::from_axis_and_direction(axis2, AxisDirection::Negative);
    let positive_direction2 = negative_direction2.opposite();
    let negative_delta1 = get_limit(&mut test, center, negative_direction1, limit1);
    let positive_delta1 = get_limit(&mut test, center, positive_direction1, limit1);
    let center_index1 = negative_delta1;
    let mut bounds_by_axis1 = vec![IntBounds { min: 0, max: 0 }; (center_index1 + 1 + positive_delta1) as usize];
    bounds_by_axis1[center_index1 as usize] =
        IntBounds { min: get_limit(&mut test, center, negative_direction2, limit2), max: get_limit(&mut test, center, positive_direction2, limit2) };
    let center_index2 = bounds_by_axis1[center_index1 as usize].min;
    for i in 1..=negative_delta1 {
        let last_bounds = bounds_by_axis1[(center_index1 - (i - 1)) as usize];
        let pos = center.relative_n(negative_direction1, i);
        bounds_by_axis1[(center_index1 - i) as usize] =
            IntBounds { min: get_limit(&mut test, pos, negative_direction2, last_bounds.min), max: get_limit(&mut test, pos, positive_direction2, last_bounds.max) };
    }
    for i in 1..=positive_delta1 {
        let last_bounds = bounds_by_axis1[(center_index1 + i - 1) as usize];
        let pos = center.relative_n(positive_direction1, i);
        bounds_by_axis1[(center_index1 + i) as usize] =
            IntBounds { min: get_limit(&mut test, pos, negative_direction2, last_bounds.min), max: get_limit(&mut test, pos, positive_direction2, last_bounds.max) };
    }
    let (mut min_axis1, mut min_axis2, mut size_axis1, mut size_axis2) = (0, 0, 0, 0);
    let mut columns = vec![0; bounds_by_axis1.len()];
    for i2 in (0..=center_index2).rev() {
        for (i1, bounds2) in bounds_by_axis1.iter().enumerate() {
            let min2 = center_index2 - bounds2.min;
            let max2 = center_index2 + bounds2.max;
            columns[i1] = if i2 >= min2 && i2 <= max2 { max2 + 1 - i2 } else { 0 };
        }
        let (bounds_axis1, new_size_axis2) = get_max_rectangle_location(&columns);
        let new_size_axis1 = 1 + bounds_axis1.max - bounds_axis1.min;
        if new_size_axis1 * new_size_axis2 > size_axis1 * size_axis2 {
            min_axis1 = bounds_axis1.min;
            min_axis2 = i2;
            size_axis1 = new_size_axis1;
            size_axis2 = new_size_axis2;
        }
    }
    FoundRectangle {
        min_corner: center.relative_axis(axis1, min_axis1 - center_index1).relative_axis(axis2, min_axis2 - center_index2),
        axis1_size: size_axis1,
        axis2_size: size_axis2,
    }
}

fn get_limit(test: &mut impl FnMut(BlockPos) -> bool, mut pos: BlockPos, direction: Direction, limit: i32) -> i32 {
    let mut max = 0;
    while max < limit && {
        pos = pos.relative(direction);
        test(pos)
    } {
        max += 1;
    }
    max
}

pub fn get_max_rectangle_location(columns: &[i32]) -> (IntBounds, i32) {
    let (mut max_start, mut max_end, mut max_height) = (0, 0, 0);
    let mut stack: Vec<usize> = vec![0];
    for column in 1..=columns.len() {
        let height = if column == columns.len() { 0 } else { columns[column] };
        while let Some(&top) = stack.last() {
            let stack_height = columns[top];
            if height >= stack_height {
                stack.push(column);
                break;
            }
            stack.pop();
            let start = stack.last().map_or(0, |&top| top + 1);
            if stack_height * (column - start) as i32 > max_height * (max_end - max_start) as i32 {
                max_end = column;
                max_start = start;
                max_height = stack_height;
            }
        }
        if stack.is_empty() {
            stack.push(column);
        }
    }
    (IntBounds { min: max_start as i32, max: max_end as i32 - 1 }, max_height)
}
