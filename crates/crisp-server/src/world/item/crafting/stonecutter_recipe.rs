use std::ops::Deref;

use crisp_data::items;
use crisp_protocol::nbt::Nbt;

use super::display::recipe_display::RecipeDisplay;
use super::display::slot_display::SlotDisplay;
use super::single_item_recipe::SingleItemRecipe;
use crate::util::codec::DataResult;

pub struct StonecutterRecipe {
    base: SingleItemRecipe,
}

impl Deref for StonecutterRecipe {
    type Target = SingleItemRecipe;

    fn deref(&self) -> &SingleItemRecipe {
        &self.base
    }
}

impl StonecutterRecipe {
    pub fn decode(tag: &Nbt) -> DataResult<StonecutterRecipe> {
        Ok(StonecutterRecipe { base: SingleItemRecipe::decode(tag)? })
    }

    pub fn display(&self) -> Vec<RecipeDisplay> {
        vec![RecipeDisplay::Stonecutter { input: self.input().display(), result: self.result_display(), crafting_station: SlotDisplay::ItemSlotDisplay(items::STONECUTTER) }]
    }

    pub fn result_display(&self) -> SlotDisplay {
        SlotDisplay::ItemStackSlotDisplay(self.result().clone())
    }
}
