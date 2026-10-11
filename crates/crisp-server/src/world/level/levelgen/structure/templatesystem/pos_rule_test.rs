use crisp_core::random::RandomSource;
use crisp_core::{Axis, BlockPos, mth};
use serde_json::Value;

pub enum PosRuleTest {
    AlwaysTrue,
    LinearPos { min_chance: f32, max_chance: f32, min_dist: i32, max_dist: i32 },
    AxisAlignedLinearPos { min_chance: f32, max_chance: f32, min_dist: i32, max_dist: i32, axis: Axis },
}

impl PosRuleTest {
    pub fn parse(json: &Value) -> Result<PosRuleTest, String> {
        let ty = json.get("predicate_type").and_then(Value::as_str).ok_or("pos rule test without predicate_type")?;
        let float = |key: &str| json.get(key).and_then(Value::as_f64).unwrap_or(0.0) as f32;
        let int = |key: &str| json.get(key).and_then(Value::as_i64).unwrap_or(0) as i32;
        Ok(match ty.strip_prefix("minecraft:").unwrap_or(ty) {
            "always_true" => PosRuleTest::AlwaysTrue,
            "linear_pos" => PosRuleTest::LinearPos { min_chance: float("min_chance"), max_chance: float("max_chance"), min_dist: int("min_dist"), max_dist: int("max_dist") },
            "axis_aligned_linear_pos" => PosRuleTest::AxisAlignedLinearPos {
                min_chance: float("min_chance"),
                max_chance: float("max_chance"),
                min_dist: int("min_dist"),
                max_dist: int("max_dist"),
                axis: match json.get("axis").and_then(Value::as_str).unwrap_or("y") {
                    "x" => Axis::X,
                    "z" => Axis::Z,
                    _ => Axis::Y,
                },
            },
            other => return Err(format!("unknown pos rule test {other}")),
        })
    }

    pub fn test(&self, _in_template_pos: BlockPos, world_pos: BlockPos, world_reference: BlockPos, random: &mut impl RandomSource) -> bool {
        match *self {
            PosRuleTest::AlwaysTrue => true,
            PosRuleTest::LinearPos { min_chance, max_chance, min_dist, max_dist } => {
                let dist = (world_pos.x - world_reference.x).abs() + (world_pos.y - world_reference.y).abs() + (world_pos.z - world_reference.z).abs();
                let rnd = random.next_float();
                rnd <= mth::clamped_lerp_f(mth::inverse_lerp_f(dist as f32, min_dist as f32, max_dist as f32), min_chance, max_chance)
            }
            PosRuleTest::AxisAlignedLinearPos { min_chance, max_chance, min_dist, max_dist, axis } => {
                let (step_x, step_y, step_z) = match axis {
                    Axis::X => (1, 0, 0),
                    Axis::Y => (0, 1, 0),
                    Axis::Z => (0, 0, 1),
                };
                let xd = ((world_pos.x - world_reference.x) * step_x).abs() as f32;
                let yd = ((world_pos.y - world_reference.y) * step_y).abs() as f32;
                let zd = ((world_pos.z - world_reference.z) * step_z).abs() as f32;
                let dist = (xd + yd + zd) as i32;
                let rnd = random.next_float();
                rnd <= mth::clamped_lerp_f(mth::inverse_lerp_f(dist as f32, min_dist as f32, max_dist as f32), min_chance, max_chance)
            }
        }
    }
}
