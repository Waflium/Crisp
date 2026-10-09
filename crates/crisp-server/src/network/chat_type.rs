use crisp_data::SyncedRegistry;
use crisp_protocol::Writer;

use crate::commands::source::CommandSourceStack;
use crate::network::chat::player_chat_message::PlayerChatMessage;
use crate::network::chat::{ChatFormatting, Component};
use crate::server::Server;
use crate::world::entity::EntityId;
use crate::world::entity::player::profile_public_key::now_millis;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ChatType {
    Chat,
    SayCommand,
    MsgCommandIncoming,
    MsgCommandOutgoing,
    TeamMsgCommandIncoming,
    TeamMsgCommandOutgoing,
    EmoteCommand,
}

impl ChatType {
    fn name(self) -> &'static str {
        match self {
            ChatType::Chat => "minecraft:chat",
            ChatType::SayCommand => "minecraft:say_command",
            ChatType::MsgCommandIncoming => "minecraft:msg_command_incoming",
            ChatType::MsgCommandOutgoing => "minecraft:msg_command_outgoing",
            ChatType::TeamMsgCommandIncoming => "minecraft:team_msg_command_incoming",
            ChatType::TeamMsgCommandOutgoing => "minecraft:team_msg_command_outgoing",
            ChatType::EmoteCommand => "minecraft:emote_command",
        }
    }

    pub fn bind(self, name: Component) -> Bound {
        Bound { chat_type: self, name, target_name: None }
    }
}

#[derive(Clone, Debug)]
pub struct Bound {
    pub chat_type: ChatType,
    pub name: Component,
    pub target_name: Option<Component>,
}

impl Bound {
    pub fn with_target_name(&self, target_name: Component) -> Bound {
        Bound { target_name: Some(target_name), ..self.clone() }
    }

    pub fn decorate(&self, content: Component) -> Component {
        let target = || self.target_name.clone().unwrap_or_else(Component::empty);
        let direct_message = |key: &str, first: Component| {
            Component::translatable_with(key, vec![first.into(), content.clone().into()]).with_color(ChatFormatting::Gray).italic()
        };
        let team_message = |key: &str| Component::translatable_with(key, vec![target().into(), self.name.clone().into(), content.clone().into()]);
        match self.chat_type {
            ChatType::Chat => Component::translatable_with("chat.type.text", vec![self.name.clone().into(), content.into()]),
            ChatType::SayCommand => Component::translatable_with("chat.type.announcement", vec![self.name.clone().into(), content.into()]),
            ChatType::EmoteCommand => Component::translatable_with("chat.type.emote", vec![self.name.clone().into(), content.into()]),
            ChatType::MsgCommandIncoming => direct_message("commands.message.display.incoming", self.name.clone()),
            ChatType::MsgCommandOutgoing => direct_message("commands.message.display.outgoing", target()),
            ChatType::TeamMsgCommandIncoming => team_message("chat.type.team.text"),
            ChatType::TeamMsgCommandOutgoing => team_message("chat.type.team.sent"),
        }
    }

    pub fn write(&self, w: &mut Writer) {
        let id = SyncedRegistry::get("minecraft:chat_type").id_of(self.chat_type.name()).expect("a chat type");
        w.varint(id + 1);
        self.name.write(w);
        w.bool(self.target_name.is_some());
        if let Some(target) = &self.target_name {
            target.write(w);
        }
    }
}

pub fn broadcast_chat_message(server: &mut Server, message: &PlayerChatMessage, _sender: Option<EntityId>, chat_type: &Bound) {
    let trusted = message.has_signature() && !message.has_expired_server(now_millis());
    let decorated = chat_type.decorate(message.decorated_content()).get_string();
    if trusted {
        info!("{decorated}");
    } else {
        info!("[Not Secure] {decorated}");
    }
    for player in server.level.players.values_mut() {
        player.send_chat_message(message, chat_type);
    }
}

pub fn source_send_chat_message(server: &mut Server, source: &CommandSourceStack, message: &PlayerChatMessage, chat_type: &Bound) {
    if source.silent {
        return;
    }
    match source.player_id(server) {
        Some(player) => server.level.player_mut(player).send_chat_message(message, chat_type),
        None => source.send_system_message(server, &chat_type.decorate(message.decorated_content())),
    }
}
