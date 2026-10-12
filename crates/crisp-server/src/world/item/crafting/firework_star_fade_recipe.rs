use crisp_protocol::nbt::Nbt;

use super::crafting_input::CraftingInput;
use super::dye_recipe::get_dye;
use super::firework_star_recipe::explosion;
use super::ingredient::Ingredient;
use super::transmute_recipe::create_with_original_components;
use crate::util::codec::{self, DataResult};
use crate::world::item::ItemStack;
use crate::world::item::component as dc;
use crate::world::item::component::values::{FireworkExplosion, ItemStackTemplate};

pub struct FireworkStarFadeRecipe {
    target: Ingredient,
    dye: Ingredient,
    result: ItemStackTemplate,
}

impl FireworkStarFadeRecipe {
    pub fn decode(tag: &Nbt) -> DataResult<FireworkStarFadeRecipe> {
        Ok(FireworkStarFadeRecipe {
            target: codec::decode_field(tag, "target", Ingredient::decode)?,
            dye: codec::decode_field(tag, "dye", Ingredient::decode)?,
            result: codec::decode_field(tag, "result", ItemStackTemplate::decode)?,
        })
    }

    pub fn matches(&self, input: &CraftingInput) -> bool {
        if input.ingredient_count() < 2 {
            return false;
        }
        let mut has_dye = false;
        let mut has_target = false;
        for slot in 0..input.size() {
            let item_stack = input.get_item(slot);
            if item_stack.is_empty() {
                continue;
            }
            if self.dye.test(item_stack) && item_stack.has(dc::DYE) {
                has_dye = true;
            } else {
                if !self.target.test(item_stack) {
                    return false;
                }
                if has_target {
                    return false;
                }
                has_target = true;
            }
        }
        has_target && has_dye
    }

    pub fn assemble(&self, input: &CraftingInput) -> ItemStack {
        let mut colors = Vec::new();
        let mut target_stack: Option<&ItemStack> = None;
        for slot in 0..input.size() {
            let item_stack = input.get_item(slot);
            if self.dye.test(item_stack) {
                colors.push(get_dye(item_stack).get_firework_color());
            } else if self.target.test(item_stack) {
                target_stack = Some(item_stack);
            }
        }
        let Some(target_stack) = target_stack.filter(|_| !colors.is_empty()) else { return ItemStack::empty() };
        let mut result = create_with_original_components(&self.result, target_stack);
        let current = result.get(dc::FIREWORK_EXPLOSION).map(FireworkExplosion::from_nbt);
        let faded = match current {
            Some(current) => explosion(&current.shape, current.colors, colors, current.has_trail, current.has_twinkle),
            None => explosion("small_ball", Vec::new(), colors, false, false),
        };
        result.set_value(dc::FIREWORK_EXPLOSION, faded);
        result
    }
}
