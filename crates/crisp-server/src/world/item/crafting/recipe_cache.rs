use super::crafting_input::CraftingInput;
use super::recipe_holder::RecipeHolder;
use super::recipe_manager::RecipeManager;
use crate::server::level::ServerLevel;
use crate::world::item::ItemStack;

struct Entry {
    key: Vec<ItemStack>,
    width: usize,
    height: usize,
    value: Option<&'static RecipeHolder>,
}

impl Entry {
    fn matches(&self, input: &CraftingInput) -> bool {
        self.width == input.width() && self.height == input.height() && self.key.iter().enumerate().all(|(i, key)| ItemStack::is_same_item_same_components(key, input.get_item(i)))
    }
}

pub struct RecipeCache {
    entries: Vec<Option<Entry>>,
}

impl RecipeCache {
    pub fn new(capacity: usize) -> RecipeCache {
        RecipeCache { entries: (0..capacity).map(|_| None).collect() }
    }

    pub fn get(&mut self, level: &mut ServerLevel, input: &mut CraftingInput) -> Option<&'static RecipeHolder> {
        if input.is_empty() {
            return None;
        }
        for i in 0..self.entries.len() {
            if self.entries[i].as_ref().is_some_and(|entry| entry.matches(input)) {
                self.move_entry_to_front(i);
                return self.entries[0].as_ref().and_then(|entry| entry.value);
            }
        }
        self.compute(input, level)
    }

    fn compute(&mut self, input: &mut CraftingInput, level: &mut ServerLevel) -> Option<&'static RecipeHolder> {
        let recipe = RecipeManager::get().get_crafting_recipe_for(input, level, None);
        self.insert(input, recipe);
        recipe
    }

    fn move_entry_to_front(&mut self, index: usize) {
        if index > 0 {
            self.entries[..=index].rotate_right(1);
        }
    }

    fn insert(&mut self, input: &CraftingInput, recipe: Option<&'static RecipeHolder>) {
        let key = input.items().iter().map(|item| if item.is_empty() { ItemStack::empty() } else { item.copy_with_count(1) }).collect();
        self.entries.rotate_right(1);
        self.entries[0] = Some(Entry { key, width: input.width(), height: input.height(), value: recipe });
    }
}
