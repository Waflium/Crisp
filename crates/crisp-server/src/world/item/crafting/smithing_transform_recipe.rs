use crisp_protocol::nbt::Nbt;

use super::ingredient::Ingredient;
use super::placement_info::PlacementInfo;
use super::simple_smithing_recipe::SimpleSmithingRecipe;
use super::smithing_recipe_input::SmithingRecipeInput;
use super::transmute_recipe::create_with_original_components;
use crate::util::codec::{self, DataResult};
use crate::world::item::ItemStack;
use crate::world::item::component::values::ItemStackTemplate;

pub struct SmithingTransformRecipe {
    pub base: SimpleSmithingRecipe,
    pub template: Option<Ingredient>,
    pub base_ingredient: Ingredient,
    pub addition: Option<Ingredient>,
    result: ItemStackTemplate,
}

impl SmithingTransformRecipe {
    pub fn decode(tag: &Nbt) -> DataResult<SmithingTransformRecipe> {
        let mut base = SimpleSmithingRecipe::decode(tag)?;
        let template = codec::decode_optional(tag, "template", Ingredient::decode)?;
        let base_ingredient = codec::decode_field(tag, "base", Ingredient::decode)?;
        let addition = codec::decode_optional(tag, "addition", Ingredient::decode)?;
        let result = codec::decode_field(tag, "result", ItemStackTemplate::decode)?;
        base.placement_info = PlacementInfo::create_from_optionals(&[template.clone(), Some(base_ingredient.clone()), addition.clone()]);
        Ok(SmithingTransformRecipe { base, template, base_ingredient, addition, result })
    }

    pub fn assemble(&self, input: &SmithingRecipeInput) -> ItemStack {
        create_with_original_components(&self.result, &input.base)
    }
}
