use crisp_data::game_rule;

use crate::commands::brigadier::{CommandContext, CommandSyntaxException};
use crate::commands::source::CommandSourceStack;
use crate::network::chat::Component;
use crate::server::Server;
use crate::server::level::game_mode::GameType;
use crate::world::entity::EntityId;

pub fn set_mode_self(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let player = source.player_or_exception(server)?;
    Ok(set_mode(server, source, &[player], context.get_game_mode("gamemode")))
}

pub fn set_mode_targets(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let players = context.get_players(server, source, "target")?;
    Ok(set_mode(server, source, &players, context.get_game_mode("gamemode")))
}

fn set_mode(server: &mut Server, source: &CommandSourceStack, players: &[EntityId], game_type: GameType) -> i32 {
    players.iter().filter(|&&player| set_game_mode_as(server, source, player, game_type)).count() as i32
}

pub fn set_game_mode(server: &mut Server, player: EntityId, game_type: GameType) {
    let source = CommandSourceStack::player(server, player);
    set_game_mode_as(server, &source, player, game_type);
}

fn set_game_mode_as(server: &mut Server, source: &CommandSourceStack, player: EntityId, game_type: GameType) -> bool {
    if !server.entity_level_mut(player).set_game_mode(player, game_type) {
        return false;
    }
    log_gamemode_change(server, source, player, game_type);
    true
}

fn log_gamemode_change(server: &Server, source: &CommandSourceStack, target: EntityId, new_type: GameType) {
    let mode = Component::translatable(&format!("gameMode.{}", new_type.name()));
    if source.entity == Some(target) {
        source.send_success(server, || Component::translatable_with("commands.gamemode.success.self", vec![mode.into()]), true);
    } else {
        if server.game_rules.get_bool(game_rule::SEND_COMMAND_FEEDBACK) {
            server.send_player_system_message(target, &Component::translatable_with("gameMode.changed", vec![mode.clone().into()]), false);
        }
        source.send_success(
            server,
            || Component::translatable_with("commands.gamemode.success.other", vec![server.entity_display_name(target).into(), mode.into()]),
            true,
        );
    }
}
