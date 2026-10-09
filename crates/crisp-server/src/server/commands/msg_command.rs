use crate::commands::brigadier::{CommandContext, CommandSyntaxException};
use crate::commands::source::CommandSourceStack;
use crate::network::chat::Component;
use crate::network::chat::player_chat_message::PlayerChatMessage;
use crate::network::chat_type::{ChatType, source_send_chat_message};
use crate::server::Server;
use crate::world::entity::EntityId;

pub fn msg(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let players = context.get_players(server, source, "targets")?;
    let count = players.len() as i32;
    if !players.is_empty() {
        let targets: Vec<(EntityId, Component)> = players.into_iter().map(|p| (p, server.entity_display_name(p))).collect();
        let sender = source.clone();
        context.resolve_chat_message(server, source, "message", move |server, message| send_message(server, &sender, &targets, &message))?;
    }
    Ok(count)
}

fn send_message(server: &mut Server, source: &CommandSourceStack, targets: &[(EntityId, Component)], message: &PlayerChatMessage) {
    let incoming = ChatType::MsgCommandIncoming.bind(source.display_name(server));
    for (player, name) in targets {
        let outgoing = ChatType::MsgCommandOutgoing.bind(source.display_name(server)).with_target_name(name.clone());
        source_send_chat_message(server, source, message, &outgoing);
        if let Some(player) = server.level.players.get_mut(player) {
            player.send_chat_message(message, &incoming);
        }
    }
}
