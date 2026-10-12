use crisp_protocol::nbt::Nbt;

use super::placement_info::PlacementInfo;
use super::recipe::CommonInfo;
use crate::util::codec::DataResult;

pub struct SimpleSmithingRecipe {
    pub common_info: CommonInfo,
    pub placement_info: PlacementInfo,
}

impl SimpleSmithingRecipe {
    pub fn decode(tag: &Nbt) -> DataResult<SimpleSmithingRecipe> {
        Ok(SimpleSmithingRecipe { common_info: CommonInfo::decode(tag)?, placement_info: PlacementInfo::NOT_PLACEABLE })
    }
}
