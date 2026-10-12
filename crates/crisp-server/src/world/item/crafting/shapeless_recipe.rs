use crisp_data::items;
use crisp_protocol::nbt::Nbt;

use super::crafting_input::CraftingInput;
use super::display::recipe_display::RecipeDisplay;
use super::display::slot_display::SlotDisplay;
use super::ingredient::Ingredient;
use super::normal_crafting_recipe::NormalCraftingRecipe;
use super::placement_info::PlacementInfo;
use crate::util::codec::{self, DataResult};
use crate::world::item::ItemStack;
use crate::world::item::component::values::ItemStackTemplate;

pub struct ShapelessRecipe {
    pub base: NormalCraftingRecipe,
    result: ItemStackTemplate,
    ingredients: Vec<Ingredient>,
}

impl ShapelessRecipe {
    pub fn decode(tag: &Nbt) -> DataResult<ShapelessRecipe> {
        let mut base = NormalCraftingRecipe::decode(tag)?;
        let result = codec::decode_field(tag, "result", ItemStackTemplate::decode)?;
        let ingredients = codec::decode_field(tag, "ingredients", |t| codec::list_of(t, Ingredient::decode))?;
        if ingredients.is_empty() || ingredients.len() > 9 {
            return Err(format!("List must have between 1 and 9 elements: {}", ingredients.len()));
        }
        base.placement_info = PlacementInfo::create_list(ingredients.clone());
        Ok(ShapelessRecipe { base, result, ingredients })
    }

    pub fn matches(&self, input: &mut CraftingInput) -> bool {
        if input.ingredient_count() != self.ingredients.len() {
            return false;
        }
        if input.size() == 1 && self.ingredients.len() == 1 {
            return self.ingredients[0].test(input.get_item(0));
        }
        let placement_info = &self.base.placement_info;
        !placement_info.is_impossible_to_place() && input.stacked_contents().can_craft_ingredients(placement_info.ingredients(), None)
    }

    pub fn assemble(&self) -> ItemStack {
        self.result.create().unwrap_or_else(ItemStack::empty)
    }

    pub fn display(&self) -> Vec<RecipeDisplay> {
        vec![RecipeDisplay::ShapelessCrafting {
            ingredients: self.ingredients.iter().map(Ingredient::display).collect(),
            result: SlotDisplay::ItemStackSlotDisplay(self.result.clone()),
            crafting_station: SlotDisplay::ItemSlotDisplay(items::CRAFTING_TABLE),
        }]
    }
}
