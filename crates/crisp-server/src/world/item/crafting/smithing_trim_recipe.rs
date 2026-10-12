use crisp_protocol::nbt::Nbt;

use super::display::recipe_display::RecipeDisplay;
use super::display::slot_display::SlotDisplay;
use super::ingredient::Ingredient;
use super::placement_info::PlacementInfo;
use super::simple_smithing_recipe::SimpleSmithingRecipe;
use super::smithing_recipe_input::SmithingRecipeInput;
use crate::util::codec::{self, DataResult};
use crate::world::item::ItemStack;
use crate::world::item::component as dc;

pub struct SmithingTrimRecipe {
    pub base: SimpleSmithingRecipe,
    pub template: Ingredient,
    pub base_ingredient: Ingredient,
    pub addition: Ingredient,
    pattern: Nbt,
}

fn decode_pattern(tag: &Nbt) -> DataResult<Nbt> {
    match tag {
        Nbt::String(id) => Ok(Nbt::String(codec::identifier(tag).map_err(|_| format!("Not a valid resource location: {id}"))?)),
        other => Ok(other.clone()),
    }
}

pub fn apply_trim(base_item: &ItemStack, material_item: &ItemStack, pattern: &Nbt) -> ItemStack {
    let Some(material) = material_item.get(dc::PROVIDES_TRIM_MATERIAL) else { return ItemStack::empty() };
    let new_trim = Nbt::Compound(vec![("material".into(), material.clone()), ("pattern".into(), pattern.clone())]);
    let new_trim = dc::TRIM.decode(&new_trim).unwrap_or(new_trim);
    if base_item.get(dc::TRIM) == Some(&new_trim) {
        return ItemStack::empty();
    }
    let mut trimmed_item = base_item.copy_with_count(1);
    trimmed_item.set(dc::TRIM, Some(new_trim));
    trimmed_item
}

impl SmithingTrimRecipe {
    pub fn decode(tag: &Nbt) -> DataResult<SmithingTrimRecipe> {
        let mut base = SimpleSmithingRecipe::decode(tag)?;
        let template = codec::decode_field(tag, "template", Ingredient::decode)?;
        let base_ingredient = codec::decode_field(tag, "base", Ingredient::decode)?;
        let addition = codec::decode_field(tag, "addition", Ingredient::decode)?;
        let pattern = codec::decode_field(tag, "pattern", decode_pattern)?;
        base.placement_info = PlacementInfo::create_list(vec![template.clone(), base_ingredient.clone(), addition.clone()]);
        Ok(SmithingTrimRecipe { base, template, base_ingredient, addition, pattern })
    }

    pub fn assemble(&self, input: &SmithingRecipeInput) -> ItemStack {
        apply_trim(&input.base, &input.addition, &self.pattern)
    }

    pub fn display(&self) -> Vec<RecipeDisplay> {
        let base = self.base_ingredient.display();
        let material = self.addition.display();
        vec![RecipeDisplay::Smithing {
            template: self.template.display(),
            base: base.clone(),
            addition: material.clone(),
            result: SlotDisplay::SmithingTrimDemoSlotDisplay { base: Box::new(base), material: Box::new(material), pattern: self.pattern.clone() },
            crafting_station: SlotDisplay::ItemSlotDisplay(crisp_data::items::SMITHING_TABLE),
        }]
    }
}
