use crate::world::item::ItemStack;

pub struct SmithingRecipeInput {
    pub template: ItemStack,
    pub base: ItemStack,
    pub addition: ItemStack,
}

impl SmithingRecipeInput {
    pub fn is_empty(&self) -> bool {
        self.template.is_empty() && self.base.is_empty() && self.addition.is_empty()
    }
}
