use crisp_data::packets::play::clientbound;
use crisp_protocol::Writer;
use crisp_protocol::nbt::Nbt;

use super::recipe_book::RecipeBook;
use super::recipe_book_settings::RecipeBookSettings;
use crate::network::connection::Outbound;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::item::crafting::display::recipe_display_entry::RecipeDisplayEntry;
use crate::world::item::crafting::recipe_holder::RecipeHolder;
use crate::world::item::crafting::recipe_manager::RecipeManager;

pub const RECIPE_BOOK_TAG: &str = "recipeBook";
const FLAG_NOTIFICATION: u8 = 1;
const FLAG_HIGHLIGHT: u8 = 2;

#[derive(Clone, Debug, Default)]
pub struct ServerRecipeBook {
    pub base: RecipeBook,
    known: Vec<&'static str>,
    highlight: Vec<&'static str>,
}

fn write_entry(w: &mut Writer, contents: &RecipeDisplayEntry, notification: bool, highlight: bool) {
    contents.write(w);
    w.u8(if notification { FLAG_NOTIFICATION } else { 0 } | if highlight { FLAG_HIGHLIGHT } else { 0 });
}

fn recipe_keys(tag: Option<&Nbt>) -> Vec<String> {
    tag.map(Nbt::as_list).unwrap_or(&[]).iter().filter_map(Nbt::as_str).filter_map(crate::commands::arguments::parse_identifier).collect()
}

impl ServerRecipeBook {
    pub fn add(&mut self, id: &'static str) {
        if !self.known.contains(&id) {
            self.known.push(id);
        }
    }

    pub fn contains(&self, id: &str) -> bool {
        self.known.contains(&id)
    }

    pub fn remove(&mut self, id: &str) {
        self.known.retain(|&known| known != id);
        self.highlight.retain(|&highlight| highlight != id);
    }

    pub fn remove_highlight(&mut self, id: &str) {
        self.highlight.retain(|&highlight| highlight != id);
    }

    fn add_highlight(&mut self, id: &'static str) {
        if !self.highlight.contains(&id) {
            self.highlight.push(id);
        }
    }

    fn load_recipes(recipes: Vec<String>, target: &mut Vec<&'static str>) {
        for recipe in recipes {
            match RecipeManager::get().by_key(&recipe) {
                Some(holder) => {
                    if !target.contains(&holder.id) {
                        target.push(holder.id);
                    }
                }
                None => error!("Tried to load unrecognized recipe: ResourceKey[minecraft:recipe / {recipe}] removed now."),
            }
        }
    }

    pub fn copy_over_data(&mut self, book_to_copy: &ServerRecipeBook) {
        self.base.book_settings = book_to_copy.base.book_settings;
        self.known.clone_from(&book_to_copy.known);
        self.highlight.clone_from(&book_to_copy.highlight);
    }

    pub fn pack(&self) -> Nbt {
        let mut output = Vec::new();
        self.base.book_settings.encode(&mut output);
        output.push(("recipes".into(), Nbt::List(self.known.iter().map(|&id| Nbt::String(id.into())).collect())));
        output.push(("toBeDisplayed".into(), Nbt::List(self.highlight.iter().map(|&id| Nbt::String(id.into())).collect())));
        Nbt::Compound(output)
    }

    pub fn load_untrusted(&mut self, packed: &Nbt) {
        if packed.get("recipes").is_none() || packed.get("toBeDisplayed").is_none() {
            return;
        }
        self.base.book_settings = RecipeBookSettings::decode(packed);
        ServerRecipeBook::load_recipes(recipe_keys(packed.get("recipes")), &mut self.known);
        ServerRecipeBook::load_recipes(recipe_keys(packed.get("toBeDisplayed")), &mut self.highlight);
    }
}

impl ServerLevel {
    pub fn award_recipes(&mut self, player: EntityId, recipes: &[&'static RecipeHolder]) -> i32 {
        let recipe_manager = RecipeManager::get();
        let mut recipes_to_add = Vec::new();
        for recipe in recipes {
            let recipe_book = &mut self.player_mut(player).recipe_book;
            if !recipe_book.contains(recipe.id) && !recipe.value.is_special() {
                recipe_book.add(recipe.id);
                recipe_book.add_highlight(recipe.id);
                for display in recipe_manager.list_displays_for_recipe(recipe.id) {
                    recipes_to_add.push((display, recipe.value.show_notification(), true));
                }
                crate::advancements::triggers::criteria_triggers::RECIPE_UNLOCKED.trigger(self, player, recipe.id);
            }
        }
        if !recipes_to_add.is_empty() {
            self.player(player).connection.send(clientbound::RECIPE_BOOK_ADD, |w| {
                w.varint(recipes_to_add.len() as i32);
                for (display, notification, highlight) in &recipes_to_add {
                    write_entry(w, display, *notification, *highlight);
                }
                w.bool(false);
            });
        }
        recipes_to_add.len() as i32
    }

    pub fn reset_recipes(&mut self, player: EntityId, recipes: &[&'static RecipeHolder]) -> i32 {
        let recipe_manager = RecipeManager::get();
        let mut recipes_to_remove = Vec::new();
        let recipe_book = &mut self.player_mut(player).recipe_book;
        for recipe in recipes {
            if recipe_book.contains(recipe.id) {
                recipe_book.remove(recipe.id);
                recipes_to_remove.extend(recipe_manager.list_displays_for_recipe(recipe.id).map(|display| display.id));
            }
        }
        if !recipes_to_remove.is_empty() {
            self.player(player).connection.send(clientbound::RECIPE_BOOK_REMOVE, |w| {
                w.varint(recipes_to_remove.len() as i32);
                recipes_to_remove.iter().for_each(|id| w.varint(id.0));
            });
        }
        recipes_to_remove.len() as i32
    }

}

pub fn send_initial_recipe_book(connection: &Outbound, recipe_book: &ServerRecipeBook) {
    connection.send(clientbound::RECIPE_BOOK_SETTINGS, |w| recipe_book.base.book_settings.write(w));
    let recipe_manager = RecipeManager::get();
    let recipes_to_send: Vec<(&RecipeDisplayEntry, bool)> =
        recipe_book.known.iter().flat_map(|&id| recipe_manager.list_displays_for_recipe(id).map(move |display| (display, recipe_book.highlight.contains(&id)))).collect();
    connection.send(clientbound::RECIPE_BOOK_ADD, |w| {
        w.varint(recipes_to_send.len() as i32);
        for (display, highlight) in &recipes_to_send {
            write_entry(w, display, false, *highlight);
        }
        w.bool(true);
    });
}
