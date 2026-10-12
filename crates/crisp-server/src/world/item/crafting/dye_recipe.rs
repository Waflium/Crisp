use crisp_protocol::nbt::Nbt;

use super::crafting_input::CraftingInput;
use super::display::recipe_display::RecipeDisplay;
use super::display::slot_display::SlotDisplay;
use super::ingredient::Ingredient;
use super::normal_crafting_recipe::NormalCraftingRecipe;
use super::placement_info::PlacementInfo;
use super::transmute_recipe::create_with_original_components;
use crate::util::codec::{self, DataResult};
use crate::world::item::ItemStack;
use crate::world::item::component::{self as dc, values};
use crate::world::item::component::values::ItemStackTemplate;
use crate::world::item::dye_color::DyeColor;

pub struct DyeRecipe {
    pub base: NormalCraftingRecipe,
    target: Ingredient,
    dye: Ingredient,
    result: ItemStackTemplate,
}

pub fn get_dye(item_stack: &ItemStack) -> DyeColor {
    item_stack.get(dc::DYE).and_then(Nbt::as_str).and_then(DyeColor::by_name).unwrap_or(DyeColor::VALUES[0])
}

impl DyeRecipe {
    pub fn decode(tag: &Nbt) -> DataResult<DyeRecipe> {
        let mut base = NormalCraftingRecipe::decode(tag)?;
        let target = codec::decode_field(tag, "target", Ingredient::decode)?;
        let dye = codec::decode_field(tag, "dye", Ingredient::decode)?;
        let result = codec::decode_field(tag, "result", ItemStackTemplate::decode)?;
        base.placement_info = PlacementInfo::create_list(vec![target.clone(), dye.clone()]);
        Ok(DyeRecipe { base, target, dye, result })
    }

    pub fn matches(&self, input: &CraftingInput) -> bool {
        if input.ingredient_count() < 2 {
            return false;
        }
        let mut has_target = false;
        let mut has_dyes = false;
        for slot in 0..input.size() {
            let item_stack = input.get_item(slot);
            if item_stack.is_empty() {
                continue;
            }
            if self.target.test(item_stack) {
                if has_target {
                    return false;
                }
                has_target = true;
            } else {
                if !self.dye.test(item_stack) || !item_stack.has(dc::DYE) {
                    return false;
                }
                has_dyes = true;
            }
        }
        has_dyes && has_target
    }

    pub fn assemble(&self, input: &CraftingInput) -> ItemStack {
        let mut dyes = Vec::new();
        let mut target_stack: Option<&ItemStack> = None;
        for slot in 0..input.size() {
            let item_stack = input.get_item(slot);
            if item_stack.is_empty() {
                continue;
            }
            if self.target.test(item_stack) {
                if target_stack.is_some() {
                    return ItemStack::empty();
                }
                target_stack = Some(item_stack);
            } else {
                if !self.dye.test(item_stack) {
                    return ItemStack::empty();
                }
                dyes.push(get_dye(item_stack));
            }
        }
        let Some(target_stack) = target_stack.filter(|_| !dyes.is_empty()) else { return ItemStack::empty() };
        let current_dye = target_stack.get(dc::DYED_COLOR).map(values::dyed_color);
        let new_dyed_color = values::apply_dyes(current_dye, &dyes);
        let mut result = create_with_original_components(&self.result, target_stack);
        result.set_value(dc::DYED_COLOR, Nbt::Int(new_dyed_color));
        result
    }

    pub fn display(&self) -> Vec<RecipeDisplay> {
        let dyes_with_dye_component = SlotDisplay::OnlyWithComponent(Box::new(self.dye.display()), dc::DYE);
        let target_display = self.target.display();
        vec![RecipeDisplay::ShapelessCrafting {
            ingredients: vec![target_display.clone(), dyes_with_dye_component.clone()],
            result: SlotDisplay::DyedSlotDemo { dye: Box::new(dyes_with_dye_component), target: Box::new(target_display) },
            crafting_station: SlotDisplay::ItemSlotDisplay(crisp_data::items::CRAFTING_TABLE),
        }]
    }
}
