use crisp_protocol::nbt::Nbt;

use super::brewing_input::BrewingInput;
use super::potion_ingredient::PotionIngredient;
use crate::util::codec::{self, DataResult};
use crate::world::item::ItemStack;
use crate::world::item::component::values::ItemStackTemplate;

pub struct BrewingRecipe {
    input: PotionIngredient,
    reagent: PotionIngredient,
    output: ItemStackTemplate,
}

impl BrewingRecipe {
    pub fn decode(tag: &Nbt) -> DataResult<BrewingRecipe> {
        Ok(BrewingRecipe {
            input: codec::decode_field(tag, "input", PotionIngredient::decode)?,
            reagent: codec::decode_field(tag, "reagent", PotionIngredient::decode)?,
            output: codec::decode_field(tag, "output", ItemStackTemplate::decode)?,
        })
    }

    pub fn get_input(&self) -> &PotionIngredient {
        &self.input
    }

    pub fn get_reagent(&self) -> &PotionIngredient {
        &self.reagent
    }

    pub fn matches(&self, brewing_input: &BrewingInput) -> bool {
        self.input.test(brewing_input.input) && self.reagent.test(brewing_input.reagent)
    }

    pub fn assemble(&self) -> ItemStack {
        self.output.create().unwrap_or_else(ItemStack::empty)
    }
}
