use crate::world::item::ItemStack;

pub struct BrewingInput<'a> {
    pub input: &'a ItemStack,
    pub reagent: &'a ItemStack,
}

impl BrewingInput<'_> {
    pub fn is_empty(&self) -> bool {
        self.input.is_empty() && self.reagent.is_empty()
    }
}
