use crate::commands::brigadier::{CommandContext, CommandSyntaxException};
use crate::commands::source::CommandSourceStack;
use crate::network::chat::Component;
use crate::server::Server;

pub fn set_mode(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let game_type = context.get_game_mode("gamemode");
    server.default_game_type = game_type;
    let count = server.enforce_game_type_for_players(server.forced_game_type());
    source.send_success(server, || Component::translatable_with("commands.defaultgamemode.success", vec![game_type.long_display_name().into()]), true);
    Ok(count)
}
