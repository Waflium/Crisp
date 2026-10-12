use crisp_protocol::nbt::Nbt;

use super::ingredient::Ingredient;
use super::placement_info::PlacementInfo;
use super::recipe::CommonInfo;
use super::single_recipe_input::SingleRecipeInput;
use crate::util::codec::{self, DataResult};
use crate::world::item::ItemStack;
use crate::world::item::component::values::ItemStackTemplate;

pub struct SingleItemRecipe {
    pub common_info: CommonInfo,
    input: Ingredient,
    result: ItemStackTemplate,
    placement_info: PlacementInfo,
}

impl SingleItemRecipe {
    pub fn decode(tag: &Nbt) -> DataResult<SingleItemRecipe> {
        let common_info = CommonInfo::decode(tag)?;
        let input = codec::decode_field(tag, "ingredient", Ingredient::decode)?;
        let result = codec::decode_field(tag, "result", ItemStackTemplate::decode)?;
        let placement_info = PlacementInfo::create(&input);
        Ok(SingleItemRecipe { common_info, input, result, placement_info })
    }

    pub fn matches(&self, input: &SingleRecipeInput) -> bool {
        self.input.test(input.item)
    }

    pub fn show_notification(&self) -> bool {
        self.common_info.show_notification
    }

    pub fn input(&self) -> &Ingredient {
        &self.input
    }

    pub fn result(&self) -> &ItemStackTemplate {
        &self.result
    }

    pub fn placement_info(&self) -> &PlacementInfo {
        &self.placement_info
    }

    pub fn assemble(&self) -> ItemStack {
        self.result.create().unwrap_or_else(ItemStack::empty)
    }
}
