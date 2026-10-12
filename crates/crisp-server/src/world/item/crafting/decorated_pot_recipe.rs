use crisp_protocol::nbt::Nbt;

use super::crafting_input::CraftingInput;
use super::ingredient::Ingredient;
use crate::util::codec::{self, DataResult};
use crate::world::item::ItemStack;
use crate::world::item::component::values::ItemStackTemplate;
use crate::world::item::component::{self as dc, DataComponentPatch};

pub struct DecoratedPotRecipe {
    back_pattern: Ingredient,
    left_pattern: Ingredient,
    right_pattern: Ingredient,
    front_pattern: Ingredient,
    result: ItemStackTemplate,
}

fn back(input: &CraftingInput) -> &ItemStack {
    input.get_item_at(1, 0)
}

fn left(input: &CraftingInput) -> &ItemStack {
    input.get_item_at(0, 1)
}

fn right(input: &CraftingInput) -> &ItemStack {
    input.get_item_at(2, 1)
}

fn front(input: &CraftingInput) -> &ItemStack {
    input.get_item_at(1, 2)
}

fn convert_to_optional(input: &ItemStack) -> Option<ItemStackTemplate> {
    (!input.is_empty()).then(|| ItemStackTemplate::from_non_empty_stack(input, 1))
}

impl DecoratedPotRecipe {
    pub fn decode(tag: &Nbt) -> DataResult<DecoratedPotRecipe> {
        Ok(DecoratedPotRecipe {
            back_pattern: codec::decode_field(tag, "back", Ingredient::decode)?,
            left_pattern: codec::decode_field(tag, "left", Ingredient::decode)?,
            right_pattern: codec::decode_field(tag, "right", Ingredient::decode)?,
            front_pattern: codec::decode_field(tag, "front", Ingredient::decode)?,
            result: codec::decode_field(tag, "result", ItemStackTemplate::decode)?,
        })
    }

    pub fn matches(&self, input: &CraftingInput) -> bool {
        input.width() == 3
            && input.height() == 3
            && input.ingredient_count() == 4
            && self.back_pattern.test(back(input))
            && self.left_pattern.test(left(input))
            && self.right_pattern.test(right(input))
            && self.front_pattern.test(front(input))
    }

    pub fn assemble(&self, input: &CraftingInput) -> ItemStack {
        let mut decorations = Vec::new();
        for (name, stack) in [("back", back(input)), ("left", left(input)), ("right", right(input)), ("front", front(input))] {
            if let Some(template) = convert_to_optional(stack) {
                decorations.push((name.to_string(), template.to_nbt()));
            }
        }
        let decorations = Nbt::Compound(decorations);
        let mut components = DataComponentPatch::default();
        components.put(dc::POT_DECORATIONS, Some(dc::POT_DECORATIONS.decode(&decorations).unwrap_or(decorations)));
        self.result.apply(self.result.count, &components)
    }
}
