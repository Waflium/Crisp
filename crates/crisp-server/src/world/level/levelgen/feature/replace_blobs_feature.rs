use crisp_core::BlockPos;
use crisp_core::random::RandomSource;
use crisp_data::{Block, BlockState};
use serde_json::Value;

use crate::util::valueproviders::IntProvider;
use crate::world::level::levelgen::parse_block_state;
use crate::world::level::world_gen_level::WorldGenLevel;

pub struct ReplaceBlobsFeature {
    target_state: BlockState,
    replace_state: BlockState,
    radius: IntProvider,
}

impl ReplaceBlobsFeature {
    pub fn parse(json: &Value) -> Result<ReplaceBlobsFeature, String> {
        let field = |key: &str| json.get(key).ok_or_else(|| format!("missing {key}"));
        Ok(ReplaceBlobsFeature {
            target_state: parse_block_state(field("target")?)?,
            replace_state: parse_block_state(field("state")?)?,
            radius: IntProvider::parse(field("radius")?)?,
        })
    }

    pub fn place<R: RandomSource>(&self, level: &mut impl WorldGenLevel, random: &mut R, origin: BlockPos) -> bool {
        let target_block = self.target_state.block();
        let cursor = BlockPos::new(origin.x, origin.y.clamp(level.get_min_y() + 1, level.get_max_y()), origin.z);
        let Some(center_pos) = find_target(level, cursor, target_block) else { return false };
        let radius_x = self.radius.sample(random);
        let radius_y = self.radius.sample(random);
        let radius_z = self.radius.sample(random);
        let maximum_radius = radius_x.max(radius_y.max(radius_z));
        let mut replaced_any = false;
        for pos in BlockPos::within_box_by_manhattan_distance(center_pos, radius_x, radius_y, radius_z) {
            if pos.dist_manhattan(center_pos) > maximum_radius {
                break;
            }
            if level.get_block_state(pos).is_block(target_block) {
                level.set_block_and_update(pos, self.replace_state);
                replaced_any = true;
            }
        }
        replaced_any
    }
}

fn find_target(level: &mut impl WorldGenLevel, mut cursor: BlockPos, target: Block) -> Option<BlockPos> {
    while cursor.y > level.get_min_y() + 1 {
        if level.get_block_state(cursor).is_block(target) {
            return Some(cursor);
        }
        cursor = cursor.below();
    }
    None
}
