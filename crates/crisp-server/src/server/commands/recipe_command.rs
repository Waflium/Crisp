use super::command_response_tracker::{CommandResponseTracker, ElementType};
use crate::commands::arguments::resource_key_argument;
use crate::commands::brigadier::{CommandContext, CommandSyntaxException, SimpleCommandExceptionType};
use crate::commands::source::CommandSourceStack;
use crate::network::chat::Component;
use crate::server::Server;
use crate::world::entity::EntityId;
use crate::world::item::crafting::recipe_holder::RecipeHolder;
use crate::world::item::crafting::recipe_manager::RecipeManager;

static ERROR_GIVE_FAILED: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.recipe.give.failed");
static ERROR_TAKE_FAILED: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.recipe.take.failed");

type Result<T> = std::result::Result<T, CommandSyntaxException>;

pub fn give_recipe(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    let players = context.get_players(server, source, "targets")?;
    let recipe = resource_key_argument::get_recipe(context, "recipe")?;
    give_recipes(server, source, &players, &[recipe])
}

pub fn give_all_recipes(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    let players = context.get_players(server, source, "targets")?;
    let recipes: Vec<&'static RecipeHolder> = RecipeManager::get().get_learnable_recipes().collect();
    give_recipes(server, source, &players, &recipes)
}

pub fn take_recipe(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    let players = context.get_players(server, source, "targets")?;
    let recipe = resource_key_argument::get_recipe(context, "recipe")?;
    take_recipes(server, source, &players, &[recipe])
}

pub fn take_all_recipes(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    let players = context.get_players(server, source, "targets")?;
    let recipes: Vec<&'static RecipeHolder> = RecipeManager::get().get_learnable_recipes().collect();
    take_recipes(server, source, &players, &recipes)
}

fn give_recipes(server: &mut Server, source: &CommandSourceStack, players: &[EntityId], recipes: &[&'static RecipeHolder]) -> Result<i32> {
    let mut response = CommandResponseTracker::new();
    for &player in players {
        let value = server.entity_level_mut(player).award_recipes(player, recipes);
        response.track_value(player, value);
    }
    response.send_feedback_of(
        server,
        source,
        true,
        ElementType::NonZero,
        Some(|| ERROR_GIVE_FAILED.create()),
        |player, total_value| Component::translatable_with("commands.recipe.give.success.single", vec![total_value.into(), server.entity_display_name(player).into()]),
        |player_count, total_value| Component::translatable_with("commands.recipe.give.success.multiple", vec![total_value.into(), player_count.into()]),
    )
}

fn take_recipes(server: &mut Server, source: &CommandSourceStack, players: &[EntityId], recipes: &[&'static RecipeHolder]) -> Result<i32> {
    let mut response = CommandResponseTracker::new();
    for &player in players {
        let value = server.entity_level_mut(player).reset_recipes(player, recipes);
        response.track_value(player, value);
    }
    response.send_feedback_of(
        server,
        source,
        true,
        ElementType::NonZero,
        Some(|| ERROR_TAKE_FAILED.create()),
        |player, total_value| Component::translatable_with("commands.recipe.take.success.single", vec![total_value.into(), server.entity_display_name(player).into()]),
        |player_count, total_value| Component::translatable_with("commands.recipe.take.success.multiple", vec![total_value.into(), player_count.into()]),
    )
}
