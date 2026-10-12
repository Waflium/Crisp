use crisp_protocol::nbt::Nbt;

use super::ingredient::Ingredient;
use super::placement_info::PlacementInfo;
use super::smithing_recipe_input::SmithingRecipeInput;
use super::smithing_transform_recipe::SmithingTransformRecipe;
use super::smithing_trim_recipe::SmithingTrimRecipe;
use crate::util::codec::DataResult;
use crate::world::item::ItemStack;

pub enum SmithingRecipe {
    Transform(SmithingTransformRecipe),
    Trim(SmithingTrimRecipe),
}

impl SmithingRecipe {
    pub fn decode(ty: &str, tag: &Nbt) -> DataResult<SmithingRecipe> {
        Ok(if ty == "minecraft:smithing_trim" { SmithingRecipe::Trim(SmithingTrimRecipe::decode(tag)?) } else { SmithingRecipe::Transform(SmithingTransformRecipe::decode(tag)?) })
    }

    pub fn matches(&self, input: &SmithingRecipeInput) -> bool {
        Ingredient::test_optional_ingredient(self.template_ingredient(), &input.template)
            && self.base_ingredient().test(&input.base)
            && Ingredient::test_optional_ingredient(self.addition_ingredient(), &input.addition)
    }

    pub fn assemble(&self, input: &SmithingRecipeInput) -> ItemStack {
        match self {
            SmithingRecipe::Transform(recipe) => recipe.assemble(input),
            SmithingRecipe::Trim(recipe) => recipe.assemble(input),
        }
    }

    pub fn template_ingredient(&self) -> Option<&Ingredient> {
        match self {
            SmithingRecipe::Transform(recipe) => recipe.template.as_ref(),
            SmithingRecipe::Trim(recipe) => Some(&recipe.template),
        }
    }

    pub fn base_ingredient(&self) -> &Ingredient {
        match self {
            SmithingRecipe::Transform(recipe) => &recipe.base_ingredient,
            SmithingRecipe::Trim(recipe) => &recipe.base_ingredient,
        }
    }

    pub fn addition_ingredient(&self) -> Option<&Ingredient> {
        match self {
            SmithingRecipe::Transform(recipe) => recipe.addition.as_ref(),
            SmithingRecipe::Trim(recipe) => Some(&recipe.addition),
        }
    }

    pub fn show_notification(&self) -> bool {
        match self {
            SmithingRecipe::Transform(recipe) => recipe.base.common_info.show_notification,
            SmithingRecipe::Trim(recipe) => recipe.base.common_info.show_notification,
        }
    }

    pub fn placement_info(&self) -> &PlacementInfo {
        match self {
            SmithingRecipe::Transform(recipe) => &recipe.base.placement_info,
            SmithingRecipe::Trim(recipe) => &recipe.base.placement_info,
        }
    }
}
