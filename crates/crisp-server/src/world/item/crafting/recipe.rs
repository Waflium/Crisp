use crisp_protocol::nbt::Nbt;

use super::abstract_cooking_recipe::AbstractCookingRecipe;
use super::brewing_recipe::BrewingRecipe;
use super::crafting_recipe::CraftingRecipe;
use super::placement_info::PlacementInfo;
use super::recipe_type::RecipeType;
use super::smithing_recipe::SmithingRecipe;
use super::stonecutter_recipe::StonecutterRecipe;
use crate::util::codec::{self, DataResult};

pub static NOT_PLACEABLE: PlacementInfo = PlacementInfo::NOT_PLACEABLE;

#[derive(Clone, Copy, Debug)]
pub struct CommonInfo {
    pub show_notification: bool,
}

impl CommonInfo {
    pub fn decode(tag: &Nbt) -> DataResult<CommonInfo> {
        Ok(CommonInfo { show_notification: codec::decode_or(tag, "show_notification", true, codec::bool_value)? })
    }
}

pub enum Recipe {
    Crafting(CraftingRecipe),
    Cooking(AbstractCookingRecipe),
    Stonecutter(StonecutterRecipe),
    Smithing(SmithingRecipe),
    Brewing(BrewingRecipe),
}

impl Recipe {
    pub fn decode(tag: &Nbt) -> DataResult<Recipe> {
        let ty = codec::decode_field(tag, "type", codec::identifier)?;
        Ok(match ty.as_str() {
            "minecraft:smelting" => Recipe::Cooking(AbstractCookingRecipe::decode(RecipeType::Smelting, tag)?),
            "minecraft:blasting" => Recipe::Cooking(AbstractCookingRecipe::decode(RecipeType::Blasting, tag)?),
            "minecraft:smoking" => Recipe::Cooking(AbstractCookingRecipe::decode(RecipeType::Smoking, tag)?),
            "minecraft:campfire_cooking" => Recipe::Cooking(AbstractCookingRecipe::decode(RecipeType::CampfireCooking, tag)?),
            "minecraft:stonecutting" => Recipe::Stonecutter(StonecutterRecipe::decode(tag)?),
            "minecraft:smithing_transform" | "minecraft:smithing_trim" => Recipe::Smithing(SmithingRecipe::decode(&ty, tag)?),
            "minecraft:brewing" => Recipe::Brewing(BrewingRecipe::decode(tag)?),
            _ => Recipe::Crafting(CraftingRecipe::decode(&ty, tag)?),
        })
    }

    pub fn get_type(&self) -> RecipeType {
        match self {
            Recipe::Crafting(_) => RecipeType::Crafting,
            Recipe::Cooking(recipe) => recipe.get_type(),
            Recipe::Stonecutter(_) => RecipeType::Stonecutting,
            Recipe::Smithing(_) => RecipeType::Smithing,
            Recipe::Brewing(_) => RecipeType::Brewing,
        }
    }

    pub fn is_special(&self) -> bool {
        match self {
            Recipe::Crafting(recipe) => recipe.is_special(),
            Recipe::Brewing(_) => true,
            _ => false,
        }
    }

    pub fn show_notification(&self) -> bool {
        match self {
            Recipe::Crafting(recipe) => recipe.show_notification(),
            Recipe::Cooking(recipe) => recipe.show_notification(),
            Recipe::Stonecutter(recipe) => recipe.show_notification(),
            Recipe::Smithing(recipe) => recipe.show_notification(),
            Recipe::Brewing(_) => false,
        }
    }

    pub fn group(&self) -> &str {
        match self {
            Recipe::Crafting(recipe) => recipe.group(),
            Recipe::Cooking(recipe) => recipe.group(),
            Recipe::Stonecutter(_) | Recipe::Smithing(_) | Recipe::Brewing(_) => "",
        }
    }

    pub fn placement_info(&self) -> &PlacementInfo {
        match self {
            Recipe::Crafting(recipe) => recipe.placement_info(),
            Recipe::Cooking(recipe) => recipe.placement_info(),
            Recipe::Stonecutter(recipe) => recipe.placement_info(),
            Recipe::Smithing(recipe) => recipe.placement_info(),
            Recipe::Brewing(_) => &NOT_PLACEABLE,
        }
    }
}
