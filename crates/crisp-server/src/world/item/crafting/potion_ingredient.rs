use crisp_data::item_tags;
use crisp_protocol::nbt::Nbt;

use super::ingredient::Ingredient;
use super::recipe_manager::RecipeManager;
use super::recipe_property_set::BREWING_INPUTS;
use crate::core::component::predicates::potions_predicate::PotionsPredicate;
use crate::util::codec::{self, DataResult};
use crate::world::item::ItemStack;
use crate::world::item::component as dc;

pub struct PotionIngredient {
    pub ingredient: Ingredient,
    potions: Option<PotionsPredicate>,
}

impl PotionIngredient {
    pub fn decode(tag: &Nbt) -> DataResult<PotionIngredient> {
        Ok(PotionIngredient { ingredient: codec::decode_field(tag, "item", Ingredient::decode)?, potions: codec::decode_optional(tag, "potion_contents", PotionsPredicate::decode)? })
    }

    pub fn test(&self, input: &ItemStack) -> bool {
        if !self.ingredient.test(input) {
            return false;
        }
        self.potions.as_ref().is_none_or(|potions| input.get(dc::POTION_CONTENTS).is_some_and(|value| potions.matches(value)))
    }

    pub fn is_potion_input(item_stack: &ItemStack) -> bool {
        RecipeManager::get().property_set(BREWING_INPUTS).test(item_stack) || item_stack.get_item().is(&item_tags::BREWING_POTION_INPUTS)
    }
}
