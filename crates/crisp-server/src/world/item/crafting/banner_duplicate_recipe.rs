use crisp_protocol::nbt::Nbt;

use super::crafting_input::CraftingInput;
use super::ingredient::Ingredient;
use super::transmute_recipe::create_with_original_components;
use crate::util::codec::{self, DataResult};
use crate::world::item::ItemStack;
use crate::world::item::banner_item;
use crate::world::item::component as dc;
use crate::world::item::component::values::ItemStackTemplate;

pub struct BannerDuplicateRecipe {
    banner: Ingredient,
    result: ItemStackTemplate,
}

pub fn banner_pattern_count(item_stack: &ItemStack) -> usize {
    item_stack.get(dc::BANNER_PATTERNS).map_or(0, |layers| layers.as_list().len())
}

impl BannerDuplicateRecipe {
    pub fn decode(tag: &Nbt) -> DataResult<BannerDuplicateRecipe> {
        Ok(BannerDuplicateRecipe { banner: codec::decode_field(tag, "banner", Ingredient::decode)?, result: codec::decode_field(tag, "result", ItemStackTemplate::decode)? })
    }

    pub fn matches(&self, input: &CraftingInput) -> bool {
        if input.ingredient_count() != 2 {
            return false;
        }
        let mut color = None;
        let mut has_target = false;
        let mut has_source = false;
        for slot in 0..input.size() {
            let item_stack = input.get_item(slot);
            if item_stack.is_empty() {
                continue;
            }
            let Some(banner_color) = banner_item::get_color(item_stack.get_item()).filter(|_| self.banner.test(item_stack)) else { return false };
            match color {
                None => color = Some(banner_color),
                Some(color) if color != banner_color => return false,
                _ => {}
            }
            let pattern_count = banner_pattern_count(item_stack);
            if pattern_count > 6 {
                return false;
            }
            if pattern_count > 0 {
                if has_source {
                    return false;
                }
                has_source = true;
            } else {
                if has_target {
                    return false;
                }
                has_target = true;
            }
        }
        has_source && has_target
    }

    pub fn assemble(&self, input: &CraftingInput) -> ItemStack {
        for slot in 0..input.size() {
            let item_stack = input.get_item(slot);
            if !item_stack.is_empty() {
                let pattern_count = banner_pattern_count(item_stack);
                if pattern_count > 0 && pattern_count <= 6 {
                    return create_with_original_components(&self.result, item_stack);
                }
            }
        }
        ItemStack::empty()
    }

    pub fn get_remaining_items(&self, input: &CraftingInput) -> Vec<ItemStack> {
        let mut result = vec![ItemStack::empty(); input.size()];
        for (slot, remaining) in result.iter_mut().enumerate() {
            let item_stack = input.get_item(slot);
            if !item_stack.is_empty() {
                if let Some(remainder) = item_stack.get_item().crafting_remainder() {
                    *remaining = ItemStack::new(remainder, 1);
                } else if banner_pattern_count(item_stack) > 0 {
                    *remaining = item_stack.copy_with_count(1);
                }
            }
        }
        result
    }
}
