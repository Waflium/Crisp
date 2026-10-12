use crisp_data::items;
use crisp_protocol::nbt::Nbt;

use super::crafting_input::CraftingInput;
use super::display::recipe_display::RecipeDisplay;
use super::display::slot_display::SlotDisplay;
use super::ingredient::Ingredient;
use super::normal_crafting_recipe::NormalCraftingRecipe;
use super::placement_info::PlacementInfo;
use super::shaped_recipe_pattern::ShapedRecipePattern;
use crate::util::codec::{self, DataResult};
use crate::world::item::ItemStack;
use crate::world::item::component::values::ItemStackTemplate;

pub struct ShapedRecipe {
    pub base: NormalCraftingRecipe,
    pattern: ShapedRecipePattern,
    result: ItemStackTemplate,
}

impl ShapedRecipe {
    pub fn decode(tag: &Nbt) -> DataResult<ShapedRecipe> {
        let mut base = NormalCraftingRecipe::decode(tag)?;
        let pattern = ShapedRecipePattern::decode(tag)?;
        let result = codec::decode_field(tag, "result", ItemStackTemplate::decode)?;
        base.placement_info = PlacementInfo::create_from_optionals(pattern.ingredients());
        Ok(ShapedRecipe { base, pattern, result })
    }

    pub fn matches(&self, input: &CraftingInput) -> bool {
        self.pattern.matches(input)
    }

    pub fn assemble(&self) -> ItemStack {
        self.result.create().unwrap_or_else(ItemStack::empty)
    }

    pub fn display(&self) -> Vec<RecipeDisplay> {
        vec![RecipeDisplay::ShapedCrafting {
            width: self.pattern.width() as i32,
            height: self.pattern.height() as i32,
            ingredients: self.pattern.ingredients().iter().map(|ingredient| Ingredient::optional_ingredient_to_display(ingredient.as_ref())).collect(),
            result: SlotDisplay::ItemStackSlotDisplay(self.result.clone()),
            crafting_station: SlotDisplay::ItemSlotDisplay(items::CRAFTING_TABLE),
        }]
    }

    pub fn get_width(&self) -> usize {
        self.pattern.width()
    }

    pub fn get_height(&self) -> usize {
        self.pattern.height()
    }
}
