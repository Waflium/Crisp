use std::sync::OnceLock;

use crisp_data::{Item, RECIPES, tag};
use serde_json::Value;

use crate::world::item::ItemStack;
use crate::world::item::component::values::ItemStackTemplate;
use crate::world::item::component::json_to_nbt;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CookingRecipeType {
    Smelting,
    Blasting,
    Smoking,
}

impl CookingRecipeType {
    fn by_name(name: &str) -> Option<CookingRecipeType> {
        Some(match name {
            "minecraft:smelting" => CookingRecipeType::Smelting,
            "minecraft:blasting" => CookingRecipeType::Blasting,
            "minecraft:smoking" => CookingRecipeType::Smoking,
            _ => return None,
        })
    }
}

pub struct AbstractCookingRecipe {
    pub id: &'static str,
    pub ty: CookingRecipeType,
    pub input: Vec<Item>,
    pub result: ItemStackTemplate,
    pub experience: f32,
    pub cooking_time: i32,
}

impl AbstractCookingRecipe {
    pub fn matches(&self, input: &ItemStack) -> bool {
        !input.is_empty() && self.input.contains(&input.item)
    }

    pub fn assemble(&self) -> ItemStack {
        self.result.create().unwrap_or_else(ItemStack::empty)
    }
}

fn ingredient(v: &Value) -> Option<Vec<Item>> {
    match v {
        Value::String(s) => match s.strip_prefix('#') {
            Some(t) => Some(tag("minecraft:item", t).iter().map(|&i| Item(i as u16)).collect()),
            None => Item::by_name(s).map(|i| vec![i]),
        },
        Value::Array(list) => list.iter().map(|v| v.as_str().and_then(Item::by_name)).collect(),
        _ => None,
    }
}

pub fn cooking_recipes() -> &'static [AbstractCookingRecipe] {
    static COOKING: OnceLock<Vec<AbstractCookingRecipe>> = OnceLock::new();
    COOKING.get_or_init(|| {
        RECIPES
            .iter()
            .filter_map(|&(id, json)| {
                let r: Value = serde_json::from_str(json).ok()?;
                let ty = CookingRecipeType::by_name(r["type"].as_str()?)?;
                let result = &r["result"];
                let components = result.get("components").map_or(crisp_protocol::nbt::Nbt::Compound(Vec::new()), json_to_nbt);
                let result = ItemStackTemplate { item: result["id"].as_str()?.to_string(), count: result["count"].as_i64().unwrap_or(1) as i32, components };
                Some(AbstractCookingRecipe {
                    id,
                    ty,
                    input: ingredient(&r["ingredient"])?,
                    result,
                    experience: r["experience"].as_f64().unwrap_or(0.0) as f32,
                    cooking_time: r["cookingtime"].as_i64()? as i32,
                })
            })
            .collect()
    })
}

pub fn get_recipe_for(ty: CookingRecipeType, input: &ItemStack, last: Option<usize>) -> Option<usize> {
    let recipes = cooking_recipes();
    if let Some(last) = last
        && recipes[last].ty == ty
        && recipes[last].matches(input)
    {
        return Some(last);
    }
    recipes.iter().position(|r| r.ty == ty && r.matches(input))
}

pub fn by_key(id: &str) -> Option<&'static AbstractCookingRecipe> {
    cooking_recipes().iter().find(|r| r.id == id)
}

pub fn property_set_contains(ty: CookingRecipeType, item: Item) -> bool {
    cooking_recipes().iter().any(|r| r.ty == ty && r.input.contains(&item))
}
