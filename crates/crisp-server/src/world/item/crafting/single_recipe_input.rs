use crate::world::item::ItemStack;

pub struct SingleRecipeInput<'a> {
    pub item: &'a ItemStack,
}

impl SingleRecipeInput<'_> {
    pub fn is_empty(&self) -> bool {
        self.item.is_empty()
    }
}
