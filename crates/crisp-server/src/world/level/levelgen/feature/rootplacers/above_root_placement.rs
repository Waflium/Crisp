use serde_json::Value;

use crate::util::valueproviders::float;
use crate::world::level::levelgen::feature::stateproviders::block_state_provider::BlockStateProviderHolder;

pub struct AboveRootPlacement {
    pub above_root_provider: BlockStateProviderHolder,
    pub above_root_placement_chance: f32,
}

impl AboveRootPlacement {
    pub fn parse(json: &Value) -> Result<AboveRootPlacement, String> {
        Ok(AboveRootPlacement {
            above_root_provider: BlockStateProviderHolder::parse(json.get("above_root_provider").ok_or("missing above_root_provider")?)?,
            above_root_placement_chance: float(json.get("above_root_placement_chance").ok_or("missing above_root_placement_chance")?)?,
        })
    }
}
