use crisp_data::{Item, item_tags};
use serde_json::Value;

use super::recipe_property_set::{BREWING_INPUTS, RecipePropertySet};
use crate::core::component::predicates::potions_predicate::PotionsPredicate;
use crate::world::item::ItemStack;
use crate::world::item::component::{self as dc, json_to_nbt};

pub struct PotionIngredient {
    pub ingredient: Vec<Item>,
    potions: Option<PotionsPredicate>,
}

impl PotionIngredient {
    pub fn from_json(json: &Value) -> Option<PotionIngredient> {
        let potions = match json.get("potion_contents") {
            Some(potions) => Some(PotionsPredicate::decode(&json_to_nbt(potions)).ok()?),
            None => None,
        };
        Some(PotionIngredient { ingredient: super::ingredient(&json["item"])?, potions })
    }

    pub fn test(&self, input: &ItemStack) -> bool {
        if input.is_empty() || !self.ingredient.contains(&input.item) {
            return false;
        }
        self.potions.as_ref().is_none_or(|potions| input.get(dc::POTION_CONTENTS).is_some_and(|value| potions.matches(value)))
    }

    pub fn is_potion_input(item_stack: &ItemStack) -> bool {
        RecipePropertySet::get(BREWING_INPUTS).test(item_stack) || item_stack.get_item().is(&item_tags::BREWING_POTION_INPUTS)
    }
}
