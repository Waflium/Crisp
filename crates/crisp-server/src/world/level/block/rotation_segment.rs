use crisp_core::Direction;

const PRECISION: i32 = 4;
const MASK: i32 = (1 << PRECISION) - 1;
const DEGREE_TO_ANGLE: f32 = (1 << PRECISION) as f32 / 360.0;
const ANGLE_TO_DEGREE: f32 = 360.0 / (1 << PRECISION) as f32;

pub fn convert_to_segment(rot_degrees: f32) -> i32 {
    ((rot_degrees * DEGREE_TO_ANGLE + 0.5).floor() as i32) & MASK
}

pub fn convert_to_segment_from_direction(direction: Direction) -> i32 {
    if direction.axis().is_vertical() { 0 } else { direction.get_2d_data_value() << (PRECISION - 2) }
}

pub fn convert_to_direction(segment: i32) -> Option<Direction> {
    match segment {
        0 => Some(Direction::North),
        4 => Some(Direction::East),
        8 => Some(Direction::South),
        12 => Some(Direction::West),
        _ => None,
    }
}

pub fn convert_to_degrees(segment: i32) -> f32 {
    let degrees = (segment & MASK) as f32 * ANGLE_TO_DEGREE;
    if degrees >= 180.0 { degrees - 360.0 } else { degrees }
}
