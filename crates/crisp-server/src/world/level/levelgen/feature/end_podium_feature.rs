use crisp_core::{BlockPos, Direction};
use crisp_data::properties::HORIZONTAL_FACING;
use crisp_data::{Block, blocks};
use serde_json::Value;

use crate::world::level::world_gen_level::WorldGenLevel;

pub const PODIUM_RADIUS: i32 = 4;
pub const PODIUM_PILLAR_HEIGHT: i32 = 4;
pub const RIM_RADIUS: i32 = 1;
pub const CORNER_ROUNDING: f32 = 0.5;

pub struct EndPodiumFeature {
    active: bool,
}

impl EndPodiumFeature {
    pub fn new(active: bool) -> EndPodiumFeature {
        EndPodiumFeature { active }
    }

    pub fn parse(json: &Value) -> Result<EndPodiumFeature, String> {
        Ok(EndPodiumFeature { active: json.get("active").and_then(Value::as_bool).unwrap_or(false) })
    }

    pub fn place(&self, level: &mut impl WorldGenLevel, origin: BlockPos) -> bool {
        let from = BlockPos::new(origin.x - 4, origin.y - 1, origin.z - 4);
        let to = BlockPos::new(origin.x + 4, origin.y + 32, origin.z + 4);
        for pos in BlockPos::between_closed(from, to) {
            let inside_rim = pos.closer_than(origin, 2.5);
            if inside_rim || pos.closer_than(origin, 3.5) {
                if pos.y < origin.y {
                    if inside_rim {
                        level.set_block_and_update(pos, blocks::BEDROCK.default_state());
                    } else if self.active {
                        drop_previous_and_set_block(level, pos, blocks::END_STONE);
                    } else {
                        level.set_block_and_update(pos, blocks::END_STONE.default_state());
                    }
                } else if pos.y > origin.y {
                    if self.active {
                        drop_previous_and_set_block(level, pos, blocks::AIR);
                    } else {
                        level.set_block_and_update(pos, blocks::AIR.default_state());
                    }
                } else if !inside_rim {
                    level.set_block_and_update(pos, blocks::BEDROCK.default_state());
                } else if self.active {
                    drop_previous_and_set_block(level, pos, blocks::END_PORTAL);
                } else {
                    level.set_block_and_update(pos, blocks::AIR.default_state());
                }
            }
        }
        for y in 0..PODIUM_PILLAR_HEIGHT {
            level.set_block_and_update(origin.offset(0, y, 0), blocks::BEDROCK.default_state());
        }
        let center_of_pillar = origin.offset(0, 2, 0);
        for face in Direction::PLANE_HORIZONTAL {
            level.set_block_and_update(center_of_pillar.relative(face), blocks::WALL_TORCH.default_state().set(HORIZONTAL_FACING, face));
        }
        true
    }
}

fn drop_previous_and_set_block(level: &mut impl WorldGenLevel, pos: BlockPos, block: Block) {
    if !level.get_block_state(pos).is_block(block) {
        level.destroy_block(pos, true);
        level.set_block_and_update(pos, block.default_state());
    }
}
