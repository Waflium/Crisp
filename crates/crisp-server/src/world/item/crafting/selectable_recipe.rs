use super::ingredient::Ingredient;
use super::recipe_holder::RecipeHolder;
use crate::world::item::ItemStack;

#[derive(Clone, Copy)]
pub struct SingleInputEntry {
    pub input: &'static Ingredient,
    pub recipe: &'static RecipeHolder,
}

#[derive(Clone, Default)]
pub struct SingleInputSet {
    entries: Vec<SingleInputEntry>,
}

impl SingleInputSet {
    pub fn new(entries: Vec<SingleInputEntry>) -> SingleInputSet {
        SingleInputSet { entries }
    }

    pub fn empty() -> SingleInputSet {
        SingleInputSet::default()
    }

    pub fn entries(&self) -> &[SingleInputEntry] {
        &self.entries
    }

    pub fn accepts_input(&self, input: &ItemStack) -> bool {
        self.entries.iter().any(|entry| entry.input.test(input))
    }

    pub fn select_by_input(&self, input: &ItemStack) -> SingleInputSet {
        SingleInputSet { entries: self.entries.iter().filter(|entry| entry.input.test(input)).copied().collect() }
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn size(&self) -> usize {
        self.entries.len()
    }
}
