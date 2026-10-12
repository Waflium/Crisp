use crisp_protocol::nbt::Nbt;

use super::crafting_input::CraftingInput;
use super::normal_crafting_recipe::NormalCraftingRecipe;
use super::placement_info::PlacementInfo;
use super::shaped_recipe_pattern::ShapedRecipePattern;
use crate::util::codec::{self, DataResult};
use crate::world::item::ItemStack;
use crate::world::item::component::values::ItemStackTemplate;

pub struct ShapedRecipe {
    pub base: NormalCraftingRecipe,
    pattern: ShapedRecipePattern,
    result: ItemStackTemplate,
}

impl ShapedRecipe {
    pub fn decode(tag: &Nbt) -> DataResult<ShapedRecipe> {
        let mut base = NormalCraftingRecipe::decode(tag)?;
        let pattern = ShapedRecipePattern::decode(tag)?;
        let result = codec::decode_field(tag, "result", ItemStackTemplate::decode)?;
        base.placement_info = PlacementInfo::create_from_optionals(pattern.ingredients());
        Ok(ShapedRecipe { base, pattern, result })
    }

    pub fn matches(&self, input: &CraftingInput) -> bool {
        self.pattern.matches(input)
    }

    pub fn assemble(&self) -> ItemStack {
        self.result.create().unwrap_or_else(ItemStack::empty)
    }

    pub fn get_width(&self) -> usize {
        self.pattern.width()
    }

    pub fn get_height(&self) -> usize {
        self.pattern.height()
    }
}
