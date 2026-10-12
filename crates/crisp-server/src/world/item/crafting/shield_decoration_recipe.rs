use crisp_protocol::nbt::Nbt;

use super::banner_duplicate_recipe::banner_pattern_count;
use super::crafting_input::CraftingInput;
use super::ingredient::Ingredient;
use super::transmute_recipe::create_with_original_components;
use crate::util::codec::{self, DataResult};
use crate::world::item::ItemStack;
use crate::world::item::banner_item;
use crate::world::item::component as dc;
use crate::world::item::component::values::ItemStackTemplate;
use crate::world::item::dye_color::DyeColor;

pub struct ShieldDecorationRecipe {
    banner: Ingredient,
    target: Ingredient,
    result: ItemStackTemplate,
}

impl ShieldDecorationRecipe {
    pub fn decode(tag: &Nbt) -> DataResult<ShieldDecorationRecipe> {
        Ok(ShieldDecorationRecipe {
            banner: codec::decode_field(tag, "banner", Ingredient::decode)?,
            target: codec::decode_field(tag, "target", Ingredient::decode)?,
            result: codec::decode_field(tag, "result", ItemStackTemplate::decode)?,
        })
    }

    pub fn matches(&self, input: &CraftingInput) -> bool {
        if input.ingredient_count() != 2 {
            return false;
        }
        let mut has_clear_target = false;
        let mut has_pattern_banner = false;
        for slot in 0..input.size() {
            let item_stack = input.get_item(slot);
            if item_stack.is_empty() {
                continue;
            }
            if self.banner.test(item_stack) && banner_item::is_banner_item(item_stack.get_item()) {
                if has_pattern_banner {
                    return false;
                }
                has_pattern_banner = true;
            } else {
                if !self.target.test(item_stack) {
                    return false;
                }
                if has_clear_target {
                    return false;
                }
                if banner_pattern_count(item_stack) > 0 {
                    return false;
                }
                has_clear_target = true;
            }
        }
        has_clear_target && has_pattern_banner
    }

    pub fn assemble(&self, input: &CraftingInput) -> ItemStack {
        let mut patterns = None;
        let mut base_color = DyeColor::VALUES[0];
        let empty = ItemStack::empty();
        let mut target = &empty;
        for slot in 0..input.size() {
            let item_stack = input.get_item(slot);
            if item_stack.is_empty() {
                continue;
            }
            if self.banner.test(item_stack)
                && let Some(color) = banner_item::get_color(item_stack.get_item())
            {
                patterns = item_stack.get(dc::BANNER_PATTERNS).cloned();
                base_color = color;
            } else if self.target.test(item_stack) {
                target = item_stack;
            }
        }
        let mut result = create_with_original_components(&self.result, target);
        result.set(dc::BANNER_PATTERNS, patterns);
        result.set_value(dc::BASE_COLOR, Nbt::String(base_color.name.into()));
        result
    }
}
