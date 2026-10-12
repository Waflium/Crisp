use crate::commands::brigadier::{CommandContext, CommandSyntaxException, DynamicCommandExceptionType, tr};
use crate::commands::shared_suggestion_provider;
use crate::commands::suggestion::SuggestionsBuilder;
use crate::world::item::crafting::recipe_holder::RecipeHolder;
use crate::world::item::crafting::recipe_manager::RecipeManager;

static ERROR_INVALID_RECIPE: DynamicCommandExceptionType = DynamicCommandExceptionType(|value| tr("recipe.notFound", vec![value]));

pub fn get_recipe(context: &CommandContext, name: &str) -> Result<&'static RecipeHolder, CommandSyntaxException> {
    let key = context.get_identifier(name);
    RecipeManager::get().by_key(key).filter(|recipe| !recipe.value.is_special()).ok_or_else(|| ERROR_INVALID_RECIPE.create(key.to_string()))
}

pub fn suggest_recipes(builder: &mut SuggestionsBuilder) {
    shared_suggestion_provider::suggest_resource(RecipeManager::get().get_recipes().iter().filter(|recipe| !recipe.value.is_special()).map(|recipe| recipe.id), builder);
}
