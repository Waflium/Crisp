use crisp_core::{Direction, mth};

use super::BlockHitResult;

pub trait SelectableSlotContainer {
    fn get_rows(&self) -> i32;

    fn get_columns(&self) -> i32;

    fn get_hit_slot(&self, hit_result: &BlockHitResult, block_facing: Direction) -> Option<i32> {
        let (x, y) = get_relative_hit_coordinates_for_block_face(hit_result, block_facing)?;
        let row = get_section(1.0 - y, self.get_rows());
        let column = get_section(x, self.get_columns());
        Some(column + row * self.get_columns())
    }
}

fn get_relative_hit_coordinates_for_block_face(hit_result: &BlockHitResult, block_facing: Direction) -> Option<(f32, f32)> {
    let hit_direction = hit_result.direction;
    if block_facing != hit_direction {
        return None;
    }
    let hit_block_pos = hit_result.block_pos.relative(hit_direction);
    let relative_x = hit_result.location.x - hit_block_pos.x as f64;
    let relative_y = hit_result.location.y - hit_block_pos.y as f64;
    let relative_z = hit_result.location.z - hit_block_pos.z as f64;
    match hit_direction {
        Direction::North => Some(((1.0 - relative_x) as f32, relative_y as f32)),
        Direction::South => Some((relative_x as f32, relative_y as f32)),
        Direction::West => Some((relative_z as f32, relative_y as f32)),
        Direction::East => Some(((1.0 - relative_z) as f32, relative_y as f32)),
        Direction::Down | Direction::Up => None,
    }
}

fn get_section(relative_coordinate: f32, max_sections: i32) -> i32 {
    let targeted_pixel = relative_coordinate * 16.0;
    let section_size = 16.0 / max_sections as f32;
    mth::floor((targeted_pixel / section_size) as f64).max(0).min(max_sections - 1)
}
