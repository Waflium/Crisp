use crisp_core::BlockPos;
use crisp_data::BlockState;
use serde_json::Value;

use crate::world::level::world_gen_level::WorldGenLevel;
use crate::world::level::levelgen::parse_block_state;

pub struct FillLayerFeature {
    pub height: i32,
    pub state: BlockState,
}

impl FillLayerFeature {
    pub fn parse(json: &Value) -> Result<FillLayerFeature, String> {
        Ok(FillLayerFeature {
            height: json.get("height").and_then(Value::as_i64).ok_or("missing height")? as i32,
            state: parse_block_state(json.get("state").ok_or("missing state")?)?,
        })
    }

    pub fn place(&self, level: &mut impl WorldGenLevel, origin: BlockPos) -> bool {
        for dx in 0..16 {
            for dz in 0..16 {
                let pos = BlockPos::new(origin.x + dx, level.get_min_y() + self.height, origin.z + dz);
                if level.get_block_state(pos).is_air() {
                    level.set_block(pos, self.state, 2);
                }
            }
        }
        true
    }
}
