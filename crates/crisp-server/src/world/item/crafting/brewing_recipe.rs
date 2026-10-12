use std::sync::OnceLock;

use crisp_data::RECIPES;
use serde_json::Value;

use super::potion_ingredient::PotionIngredient;
use crate::world::item::ItemStack;
use crate::world::item::component::json_to_nbt;
use crate::world::item::component::values::ItemStackTemplate;

pub struct BrewingRecipe {
    input: PotionIngredient,
    reagent: PotionIngredient,
    output: ItemStackTemplate,
}

impl BrewingRecipe {
    pub fn get_input(&self) -> &PotionIngredient {
        &self.input
    }

    pub fn get_reagent(&self) -> &PotionIngredient {
        &self.reagent
    }

    pub fn matches(&self, input: &ItemStack, reagent: &ItemStack) -> bool {
        self.input.test(input) && self.reagent.test(reagent)
    }

    pub fn assemble(&self) -> ItemStack {
        self.output.create().unwrap_or_else(ItemStack::empty)
    }
}

pub fn brewing_recipes() -> &'static [BrewingRecipe] {
    static BREWING: OnceLock<Vec<BrewingRecipe>> = OnceLock::new();
    BREWING.get_or_init(|| {
        RECIPES
            .iter()
            .filter_map(|&(_, json)| {
                let r: Value = serde_json::from_str(json).ok()?;
                if r["type"].as_str()? != "minecraft:brewing" {
                    return None;
                }
                let output = &r["output"];
                let components = output.get("components").map_or(crisp_protocol::nbt::Nbt::Compound(Vec::new()), json_to_nbt);
                let output = ItemStackTemplate { item: output["id"].as_str()?.to_string(), count: output["count"].as_i64().unwrap_or(1) as i32, components };
                Some(BrewingRecipe { input: PotionIngredient::from_json(&r["input"])?, reagent: PotionIngredient::from_json(&r["reagent"])?, output })
            })
            .collect()
    })
}

pub fn get_recipe_for(input: &ItemStack, reagent: &ItemStack, last: Option<usize>) -> Option<usize> {
    if input.is_empty() && reagent.is_empty() {
        return None;
    }
    let recipes = brewing_recipes();
    if let Some(last) = last
        && recipes[last].matches(input, reagent)
    {
        return Some(last);
    }
    recipes.iter().position(|r| r.matches(input, reagent))
}
