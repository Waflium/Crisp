use crisp_protocol::nbt::Nbt;

use super::crafting_recipe::CraftingBookInfo;
use super::placement_info::PlacementInfo;
use super::recipe::CommonInfo;
use crate::util::codec::DataResult;

pub struct NormalCraftingRecipe {
    pub common_info: CommonInfo,
    pub book_info: CraftingBookInfo,
    pub placement_info: PlacementInfo,
}

impl NormalCraftingRecipe {
    pub fn decode(tag: &Nbt) -> DataResult<NormalCraftingRecipe> {
        Ok(NormalCraftingRecipe { common_info: CommonInfo::decode(tag)?, book_info: CraftingBookInfo::decode(tag)?, placement_info: PlacementInfo::NOT_PLACEABLE })
    }
}
