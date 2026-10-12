use crate::commands::brigadier::{CommandContext, CommandSyntaxException, SimpleCommandExceptionType};
use crate::commands::source::CommandSourceStack;
use crate::network::chat::player_chat_message::PlayerChatMessage;
use crate::network::chat::{ClickEvent, Component, HoverEvent};
use crate::network::chat_type::ChatType;
use crate::server::Server;
use crate::world::entity::EntityId;

static ERROR_NOT_ON_TEAM: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.teammsg.failed.noteam");

pub fn team_msg(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let entity = source.entity_or_exception()?;
    let Some((team_name, display_name)) =
        server.entity_level(entity).team_of(entity).map(|team| (team.name().to_string(), team.formatted_display_name()))
    else {
        return Err(ERROR_NOT_ON_TEAM.create());
    };
    let display_name =
        display_name.with_hover(HoverEvent::ShowText(Box::new(Component::translatable("chat.type.team.hover")))).with_click(ClickEvent::SuggestCommand("/teammsg ".into()));
    let receivers: Vec<EntityId> = server
        .player_list
        .iter()
        .copied()
        .filter(|&receiver| receiver == entity || server.entity_level(receiver).team_of(receiver).is_some_and(|t| t.name() == team_name))
        .collect();
    let count = receivers.len() as i32;
    if !receivers.is_empty() {
        let sender = source.clone();
        context.resolve_chat_message(server, source, "message", move |server, message| send_message(server, &sender, entity, display_name, &receivers, &message))?;
    }
    Ok(count)
}

fn send_message(server: &mut Server, source: &CommandSourceStack, entity: EntityId, team_name: Component, receivers: &[EntityId], message: &PlayerChatMessage) {
    let name = source.display_name(server);
    let incoming = ChatType::TeamMsgCommandIncoming.bind(name.clone()).with_target_name(team_name.clone());
    let outgoing = ChatType::TeamMsgCommandOutgoing.bind(name).with_target_name(team_name);
    for &receiver in receivers {
        let chat_type = if receiver == entity { &outgoing } else { &incoming };
        if let Some(player) = server.get_player_mut(receiver) {
            player.send_chat_message(message, chat_type);
        }
    }
}
