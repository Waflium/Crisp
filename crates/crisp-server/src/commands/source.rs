use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use crisp_core::Vec3;
use crisp_data::game_rule;

use super::brigadier::{CommandSyntaxException, SimpleCommandExceptionType};
use crate::network::chat::player_chat_message::PlayerChatMessage;
use crate::network::chat::{Arg, ChatFormatting, Component};
use crate::server::Server;
use crate::world::entity::EntityId;

pub static ERROR_NOT_PLAYER: SimpleCommandExceptionType = SimpleCommandExceptionType("permissions.requires.player");
pub static ERROR_NOT_ENTITY: SimpleCommandExceptionType = SimpleCommandExceptionType("permissions.requires.entity");

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CommandSource {
    Server,
    Rcon,
    Player(EntityId),
    Null,
}

impl CommandSource {
    fn accepts_success(self, server: &Server) -> bool {
        match self {
            CommandSource::Server | CommandSource::Rcon => true,
            CommandSource::Player(_) => server.game_rules.get_bool(game_rule::SEND_COMMAND_FEEDBACK),
            CommandSource::Null => false,
        }
    }

    fn accepts_failure(self) -> bool {
        !matches!(self, CommandSource::Null)
    }

    fn should_inform_admins(self, server: &Server) -> bool {
        match self {
            CommandSource::Server => server.config.broadcast_console_to_ops,
            CommandSource::Rcon => server.config.broadcast_rcon_to_ops,
            CommandSource::Player(_) => true,
            CommandSource::Null => false,
        }
    }

    fn send_system_message(self, server: &Server, message: &Component) {
        match self {
            CommandSource::Server => server.send_system_message(message),
            CommandSource::Rcon => server.rcon_buffer.borrow_mut().push_str(&message.get_string()),
            CommandSource::Player(id) => server.send_player_system_message(id, message, false),
            CommandSource::Null => {}
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Anchor {
    Feet,
    Eyes,
}

#[derive(Clone, Debug)]
pub enum Names {
    ForEntity,
    Constant(Component),
}

#[derive(Clone, Default)]
pub struct CommandResultCallback(Vec<Arc<dyn Fn(&mut Server, bool, i32) + Send + Sync>>);

impl CommandResultCallback {
    pub fn on_result(&self, server: &mut Server, success: bool, result: i32) {
        self.0.iter().for_each(|callback| callback(server, success, result));
    }
}

impl fmt::Debug for CommandResultCallback {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "CommandResultCallback({})", self.0.len())
    }
}

#[derive(Debug)]
pub struct SignedArguments {
    pub chain: EntityId,
    pub arguments: HashMap<String, PlayerChatMessage>,
}

#[derive(Clone, Debug)]
pub struct CommandSourceStack {
    pub source: CommandSource,
    pub position: Vec3,
    pub rotation: (f32, f32),
    pub permission: i32,
    pub names: Names,
    pub entity: Option<EntityId>,
    pub silent: bool,
    pub anchor: Anchor,
    pub callback: CommandResultCallback,
    pub signing_context: Option<Arc<SignedArguments>>,
}

impl CommandSourceStack {
    pub fn server(server: &Server) -> CommandSourceStack {
        CommandSourceStack {
            source: CommandSource::Server,
            position: Vec3::at_lower_corner_of(server.respawn_data().pos),
            rotation: (0.0, 0.0),
            permission: 4,
            names: Names::Constant(Component::literal("Server")),
            entity: None,
            silent: false,
            anchor: Anchor::Feet,
            callback: CommandResultCallback::default(),
            signing_context: None,
        }
    }

    pub fn rcon(server: &Server) -> CommandSourceStack {
        CommandSourceStack { source: CommandSource::Rcon, names: Names::Constant(Component::literal("Rcon")), ..CommandSourceStack::server(server) }
    }

    pub fn player(server: &Server, id: EntityId) -> CommandSourceStack {
        let player = server.level.player(id);
        CommandSourceStack {
            source: CommandSource::Player(id),
            position: player.entity.position(),
            rotation: (player.entity.x_rot, player.entity.y_rot),
            permission: server.profile_permission_level(player.profile.uuid),
            names: Names::ForEntity,
            entity: Some(id),
            silent: false,
            anchor: Anchor::Feet,
            callback: CommandResultCallback::default(),
            signing_context: None,
        }
    }

    pub fn for_new_player(server: &Server, player: &crate::server::level::player::ServerPlayer) -> CommandSourceStack {
        CommandSourceStack {
            source: CommandSource::Player(player.entity.id),
            position: player.entity.position(),
            rotation: (player.entity.x_rot, player.entity.y_rot),
            permission: server.profile_permission_level(player.profile.uuid),
            names: Names::ForEntity,
            entity: Some(player.entity.id),
            silent: false,
            anchor: Anchor::Feet,
            callback: CommandResultCallback::default(),
            signing_context: None,
        }
    }

    pub fn has_permission(&self, level: i32) -> bool {
        self.permission >= level
    }

    pub fn text_name(&self, server: &Server) -> String {
        match &self.names {
            Names::ForEntity => self.entity.map_or_else(String::new, |e| server.entity_name(e)),
            Names::Constant(name) => name.get_string(),
        }
    }

    pub fn display_name(&self, server: &Server) -> Component {
        match &self.names {
            Names::ForEntity => self.entity.map_or_else(Component::empty, |e| server.entity_display_name(e)),
            Names::Constant(name) => name.clone(),
        }
    }

    pub fn entity_or_exception(&self) -> Result<EntityId, CommandSyntaxException> {
        self.entity.ok_or_else(|| ERROR_NOT_ENTITY.create())
    }

    pub fn player_id(&self, server: &Server) -> Option<EntityId> {
        self.entity.filter(|e| server.level.players.contains_key(e))
    }

    pub fn player_or_exception(&self, server: &Server) -> Result<EntityId, CommandSyntaxException> {
        self.player_id(server).ok_or_else(|| ERROR_NOT_PLAYER.create())
    }

    pub fn is_player(&self, server: &Server) -> bool {
        self.player_id(server).is_some()
    }

    pub fn with_entity(&self, entity: EntityId) -> CommandSourceStack {
        CommandSourceStack { entity: Some(entity), names: Names::ForEntity, ..self.clone() }
    }

    pub fn with_position(&self, position: Vec3) -> CommandSourceStack {
        CommandSourceStack { position, ..self.clone() }
    }

    pub fn with_rotation(&self, rotation: (f32, f32)) -> CommandSourceStack {
        CommandSourceStack { rotation, ..self.clone() }
    }

    pub fn facing(&self, server: &Server, pos: Vec3) -> CommandSourceStack {
        use crisp_core::mth;
        let from = crate::commands::arguments::coordinates::anchor_position(server, self.anchor, self);
        let (xd, yd, zd) = (pos.x - from.x, pos.y - from.y, pos.z - from.z);
        let sd = (xd * xd + zd * zd).sqrt();
        let pi = std::f64::consts::PI as f32 as f64;
        let x_rot = mth::wrap_degrees((-(mth::atan2(yd, sd) * 180.0 / pi)) as f32);
        let y_rot = mth::wrap_degrees((mth::atan2(zd, xd) * 180.0 / pi) as f32 - 90.0);
        self.with_rotation((x_rot, y_rot))
    }

    pub fn with_suppressed_output(&self) -> CommandSourceStack {
        CommandSourceStack { silent: true, ..self.clone() }
    }

    pub fn with_anchor(&self, anchor: Anchor) -> CommandSourceStack {
        CommandSourceStack { anchor, ..self.clone() }
    }

    pub fn with_signing_context(&self, signing_context: Arc<SignedArguments>) -> CommandSourceStack {
        CommandSourceStack { signing_context: Some(signing_context), ..self.clone() }
    }

    pub fn with_chained_callback(&self, callback: impl Fn(&mut Server, bool, i32) + Send + Sync + 'static) -> CommandSourceStack {
        let mut source = self.clone();
        source.callback.0.push(Arc::new(callback));
        source
    }

    pub fn send_system_message(&self, server: &Server, message: &Component) {
        if self.silent {
            return;
        }
        match self.player_id(server) {
            Some(player) => server.send_player_system_message(player, message, false),
            None => self.source.send_system_message(server, message),
        }
    }

    pub fn send_success(&self, server: &Server, message: impl FnOnce() -> Component, broadcast: bool) {
        let send = self.source.accepts_success(server) && !self.silent;
        let broadcast = broadcast && self.source.should_inform_admins(server) && !self.silent;
        if send || broadcast {
            let message = message();
            if send {
                self.source.send_system_message(server, &message);
            }
            if broadcast {
                self.broadcast_to_admins(server, message);
            }
        }
    }

    fn broadcast_to_admins(&self, server: &Server, message: Component) {
        let broadcast = Component::translatable_with("chat.type.admin", vec![Arg::Component(self.display_name(server)), Arg::Component(message)])
            .with_color(ChatFormatting::Gray)
            .italic();
        if server.game_rules.get_bool(game_rule::SEND_COMMAND_FEEDBACK) {
            for player in server.level.players.values() {
                if self.source != CommandSource::Player(player.entity.id) && server.is_op(player.profile.uuid) {
                    server.send_player_system_message(player.entity.id, &broadcast, false);
                }
            }
        }
        if self.source != CommandSource::Server && server.game_rules.get_bool(game_rule::LOG_ADMIN_COMMANDS) {
            server.send_system_message(&broadcast);
        }
    }

    pub fn send_failure(&self, server: &Server, message: Component) {
        if self.source.accepts_failure() && !self.silent {
            self.source.send_system_message(server, &Component::empty().append(message).with_color(ChatFormatting::Red));
        }
    }

    pub fn handle_error(&self, server: &Server, e: CommandSyntaxException, forked: bool) {
        if !forked {
            self.send_failure(server, e.message);
        }
    }
}
