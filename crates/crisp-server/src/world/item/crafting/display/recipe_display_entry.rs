use crisp_data::{registry, registry_id};
use crisp_protocol::Writer;

use super::recipe_display::RecipeDisplay;
use super::slot_display::write_holder_set;
use crate::world::item::crafting::ingredient::Ingredient;
use crate::world::item::crafting::recipe_book_categories::RecipeBookCategory;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RecipeDisplayId(pub i32);

#[derive(Clone, Debug)]
pub struct RecipeDisplayEntry {
    pub id: RecipeDisplayId,
    pub display: RecipeDisplay,
    pub group: Option<i32>,
    pub category: RecipeBookCategory,
    pub crafting_requirements: Option<Vec<Ingredient>>,
}

impl RecipeDisplayEntry {
    pub fn write(&self, w: &mut Writer) {
        w.varint(self.id.0);
        self.display.write(w);
        w.varint(self.group.map_or(0, |group| group + 1));
        w.varint(registry_id(registry::RECIPE_BOOK_CATEGORY, self.category.name()));
        w.bool(self.crafting_requirements.is_some());
        if let Some(requirements) = &self.crafting_requirements {
            w.varint(requirements.len() as i32);
            requirements.iter().for_each(|ingredient| write_holder_set(w, ingredient.values()));
        }
    }
}
