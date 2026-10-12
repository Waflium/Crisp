use crisp_core::BlockPos;
use crisp_core::random::RandomSource;
use serde_json::Value;

use super::block_replacement::BlockReplacement;
use crate::world::level::world_gen_level::WorldGenLevel;

pub struct ReplaceBlockFeature {
    replacements: Vec<BlockReplacement>,
}

impl ReplaceBlockFeature {
    pub fn parse(json: &Value) -> Result<ReplaceBlockFeature, String> {
        let targets = json.get("targets").and_then(Value::as_array).ok_or("missing targets")?;
        Ok(ReplaceBlockFeature { replacements: targets.iter().map(BlockReplacement::parse).collect::<Result<_, _>>()? })
    }

    pub fn place<R: RandomSource>(&self, level: &mut impl WorldGenLevel, random: &mut R, origin: BlockPos) -> bool {
        for replacement in &self.replacements {
            let state = level.get_block_state(origin);
            if replacement.target.test(state, origin, random) {
                level.set_block(origin, replacement.state, 2);
                break;
            }
        }
        true
    }
}
