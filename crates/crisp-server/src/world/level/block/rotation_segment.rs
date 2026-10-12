const PRECISION: i32 = 4;
const MASK: i32 = (1 << PRECISION) - 1;
const DEGREE_TO_ANGLE: f32 = (1 << PRECISION) as f32 / 360.0;

pub fn convert_to_segment(rot_degrees: f32) -> i32 {
    ((rot_degrees * DEGREE_TO_ANGLE + 0.5).floor() as i32) & MASK
}
