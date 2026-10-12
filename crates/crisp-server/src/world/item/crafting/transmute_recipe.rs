use crisp_data::{Item, items};
use crisp_protocol::nbt::Nbt;

use super::crafting_input::CraftingInput;
use super::display::recipe_display::RecipeDisplay;
use super::display::slot_display::SlotDisplay;
use super::ingredient::Ingredient;
use super::normal_crafting_recipe::NormalCraftingRecipe;
use super::placement_info::PlacementInfo;
use super::transmute_result::TransmuteResult;
use crate::advancements::predicates::min_max_bounds::{Bounds, Ints};
use crate::util::codec::{self, DataResult};
use crate::world::item::ItemStack;
use crate::world::item::component::values::ItemStackTemplate;

const MIN_MATERIAL_COUNT: i32 = 1;
const MAX_MATERIAL_COUNT: i32 = 8;
const DEFAULT_MATERIAL_COUNT: Ints = Ints { bounds: Bounds { min: Some(1), max: Some(1) } };

pub struct TransmuteRecipe {
    pub base: NormalCraftingRecipe,
    input: Ingredient,
    material: Ingredient,
    material_count: Ints,
    result: TransmuteResult,
    add_material_count_to_result: bool,
}

fn decode_material_count(tag: &Nbt) -> DataResult<Ints> {
    let bounds = Ints::decode(tag)?;
    match (bounds.bounds.min, bounds.bounds.max) {
        (Some(min), Some(max)) if min >= MIN_MATERIAL_COUNT && max <= MAX_MATERIAL_COUNT => Ok(bounds),
        _ => Err(format!("Range must be within [{MIN_MATERIAL_COUNT}..{MAX_MATERIAL_COUNT}], but was {:?}", bounds.bounds)),
    }
}

pub fn create_with_original_components(target: &ItemStackTemplate, input: &ItemStack) -> ItemStack {
    create_with_original_components_extra(target, input, 0)
}

pub fn create_with_original_components_extra(target: &ItemStackTemplate, input: &ItemStack, extra_count: i32) -> ItemStack {
    target.apply(target.count + extra_count, input.components_patch())
}

impl TransmuteRecipe {
    pub fn decode(tag: &Nbt) -> DataResult<TransmuteRecipe> {
        let base = NormalCraftingRecipe::decode(tag)?;
        let input = codec::decode_field(tag, "input", Ingredient::decode)?;
        let material = codec::decode_field(tag, "material", Ingredient::decode)?;
        let material_count = codec::decode_or(tag, "material_count", DEFAULT_MATERIAL_COUNT, decode_material_count)?;
        let result = codec::decode_field(tag, "result", TransmuteResult::decode)?;
        let add_material_count_to_result = codec::decode_or(tag, "add_material_count_to_result", false, codec::bool_value)?;
        let mut recipe = TransmuteRecipe { base, input, material, material_count, result, add_material_count_to_result };
        recipe.base.placement_info = recipe.create_placement_info();
        Ok(recipe)
    }

    fn compute_result_size(&self, material_count: i32) -> i32 {
        if self.add_material_count_to_result { material_count + self.result.count } else { self.result.count }
    }

    fn compute_result(&self, input_ingredient: &ItemStack, material_count: i32) -> ItemStack {
        create_with_original_components_extra(&self.result.resolve(input_ingredient.get_item()), input_ingredient, material_count)
    }

    pub fn matches(&self, input: &CraftingInput) -> bool {
        let min_material_count = self.min_material_count();
        let max_material_count = self.max_material_count();
        let ingredient_count = input.ingredient_count() as i32;
        if ingredient_count < min_material_count + 1 || ingredient_count > max_material_count + 1 {
            return false;
        }
        let mut found_input: Option<&ItemStack> = None;
        let mut material_count = 0;
        for slot in 0..input.size() {
            let stack = input.get_item(slot);
            if stack.is_empty() {
                continue;
            }
            if self.input.test(stack) {
                if found_input.is_some() {
                    return false;
                }
                found_input = Some(stack);
            } else {
                if !self.material.test(stack) {
                    return false;
                }
                material_count += 1;
                if material_count > max_material_count {
                    return false;
                }
            }
        }
        let Some(found_input) = found_input.filter(|stack| !stack.is_empty()) else { return false };
        if !self.material_count.matches(material_count) {
            return false;
        }
        let result_count = self.compute_result_size(material_count);
        if result_count != 1 {
            return true;
        }
        let result = self.compute_result(found_input, 0);
        !result.is_empty() && !ItemStack::is_same_item_same_components(found_input, &result)
    }

    pub fn assemble(&self, input: &CraftingInput) -> ItemStack {
        if self.add_material_count_to_result {
            let mut material_count = 0;
            let empty = ItemStack::empty();
            let mut input_ingredient = &empty;
            for slot in 0..input.size() {
                let item_stack = input.get_item(slot);
                if !item_stack.is_empty() {
                    if self.input.test(item_stack) {
                        input_ingredient = item_stack;
                    } else if self.material.test(item_stack) {
                        material_count += 1;
                    }
                }
            }
            return self.compute_result(input_ingredient, material_count);
        }
        for slot in 0..input.size() {
            let item_stack = input.get_item(slot);
            if !item_stack.is_empty() && self.input.test(item_stack) {
                return self.compute_result(item_stack, 0);
            }
        }
        ItemStack::empty()
    }

    pub fn display(&self) -> Vec<RecipeDisplay> {
        let mut displays = Vec::new();
        let mut ingredient_slots = vec![self.input.display()];
        let material_display = self.material.display();
        for material_count in self.min_material_count()..=self.max_material_count() {
            ingredient_slots.push(material_display.clone());
            displays.push(RecipeDisplay::ShapelessCrafting {
                ingredients: ingredient_slots.clone(),
                result: self.result_display(self.compute_result_size(material_count)),
                crafting_station: SlotDisplay::ItemSlotDisplay(items::CRAFTING_TABLE),
            });
        }
        displays
    }

    fn result_display(&self, result_count: i32) -> SlotDisplay {
        match &self.result.item {
            Some(item) => self.stack_display(Item::by_name(item).unwrap_or(items::AIR), result_count),
            None => SlotDisplay::Composite(self.input.items().map(|item| self.stack_display(item, result_count)).collect()),
        }
    }

    fn stack_display(&self, item: Item, result_count: i32) -> SlotDisplay {
        SlotDisplay::ItemStackSlotDisplay(self.result.resolve_count(item, result_count))
    }

    fn min_material_count(&self) -> i32 {
        self.material_count.bounds.min.unwrap_or(MIN_MATERIAL_COUNT)
    }

    fn max_material_count(&self) -> i32 {
        self.material_count.bounds.max.unwrap_or(MAX_MATERIAL_COUNT)
    }

    fn create_placement_info(&self) -> PlacementInfo {
        let max_material_count = self.max_material_count() as usize;
        let mut ingredients = Vec::with_capacity(1 + max_material_count);
        ingredients.push(self.input.clone());
        ingredients.extend(std::iter::repeat_n(self.material.clone(), max_material_count));
        PlacementInfo::create_list(ingredients)
    }
}
