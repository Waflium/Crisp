use crate::commands::brigadier::{CommandContext, CommandSyntaxException};
use crate::commands::source::CommandSourceStack;
use crate::network::chat::component_utils;
use crate::server::Server;

pub fn tell_raw(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let mut result = 0;
    for player in context.get_players(server, source, "targets")? {
        let message = component_utils::update_for_entity(server, source, context.get_raw_component("message"), Some(player))?;
        server.send_player_system_message(player, &message, false);
        result += 1;
    }
    Ok(result)
}
