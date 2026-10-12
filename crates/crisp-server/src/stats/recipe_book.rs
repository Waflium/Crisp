use super::recipe_book_settings::RecipeBookSettings;
use crate::world::inventory::recipe_book_type::RecipeBookType;

#[derive(Clone, Debug, Default)]
pub struct RecipeBook {
    pub book_settings: RecipeBookSettings,
}

impl RecipeBook {
    pub fn is_open(&self, recipe_book_type: RecipeBookType) -> bool {
        self.book_settings.is_open(recipe_book_type)
    }

    pub fn is_filtering(&self, recipe_book_type: RecipeBookType) -> bool {
        self.book_settings.is_filtering(recipe_book_type)
    }

    pub fn set_book_setting(&mut self, book_type: RecipeBookType, open: bool, filtering: bool) {
        self.book_settings.set_open(book_type, open);
        self.book_settings.set_filtering(book_type, filtering);
    }
}
