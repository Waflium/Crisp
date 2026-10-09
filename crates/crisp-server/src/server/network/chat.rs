use std::collections::HashMap;
use std::sync::Arc;

use crisp_data::packets::play::clientbound;

use super::super::{Server, Task};
use crate::ServerConfig;
use crate::commands::brigadier::{self, StringReader};
use crate::commands::source::{CommandSourceStack, SignedArguments};
use crate::network::chat::last_seen_messages::{LAST_SEEN_MESSAGES_MAX_LENGTH, LastSeenMessages, Update};
use crate::network::chat::last_seen_messages_validator::LastSeenMessagesValidator;
use crate::network::chat::message_signature::MessageSignature;
use crate::network::chat::message_signature_cache::MessageSignatureCache;
use crate::network::chat::player_chat_message::PlayerChatMessage;
use crate::network::chat::remote_chat_session::{self, RemoteChatSession};
use crate::network::chat::signable_command;
use crate::network::chat::signed_message_body::SignedMessageBody;
use crate::network::chat::signed_message_chain::{DecodeException, Decoder};
use crate::network::chat::{ChatFormatting, Component};
use crate::network::chat_type::{ChatType, broadcast_chat_message};
use crate::network::configuration::ChatVisibility;
use crate::network::connection::Packet;
use crate::server::network::game::is_illegal_chat_char;
use crate::server::level::player::ServerPlayer;
use crate::util::tick_throttler::TickThrottler;
use crate::world::entity::EntityId;
use crate::world::entity::player::profile_public_key::EXPIRED_PROFILE_PUBLIC_KEY;

const CHAT_VALIDATION_FAILED: &str = "multiplayer.disconnect.chat_validation_failed";
const MAX_ARGUMENT_COUNT: usize = 8;
const MAX_ARGUMENT_NAME_LENGTH: usize = 16;

fn invalid_command_signature() -> Component {
    Component::translatable("chat.disabled.invalid_command_signature").with_color(ChatFormatting::Red)
}

pub struct ChatState {
    pub session: Option<RemoteChatSession>,
    pub decoder: Decoder,
    pub last_seen_messages: LastSeenMessagesValidator,
    pub next_chat_index: i32,
    pub message_signature_cache: MessageSignatureCache,
    pub chat_spam_throttler: TickThrottler,
    pub command_spam_throttler: TickThrottler,
}

impl ChatState {
    pub fn new(profile_id: u128, config: &ServerConfig) -> ChatState {
        ChatState {
            session: None,
            decoder: Decoder::Unsigned { profile_id },
            last_seen_messages: LastSeenMessagesValidator::new(LAST_SEEN_MESSAGES_MAX_LENGTH),
            next_chat_index: 0,
            message_signature_cache: MessageSignatureCache::default(),
            chat_spam_throttler: TickThrottler::new(20, 20 * config.chat_spam_threshold_seconds),
            command_spam_throttler: TickThrottler::new(20, 20 * config.command_spam_threshold_seconds),
        }
    }

    pub fn tick(&mut self) {
        self.chat_spam_throttler.tick();
        self.command_spam_throttler.tick();
    }
}

pub struct ArgumentSignature {
    pub name: String,
    pub signature: MessageSignature,
}

impl ServerPlayer {
    pub fn send_chat_message(&mut self, message: &PlayerChatMessage, chat_type: &crate::network::chat_type::Bound) {
        if self.client.chat_visibility != ChatVisibility::Full {
            return;
        }
        if message.is_system() {
            let content = message.decorated_content();
            self.connection.send(clientbound::DISGUISED_CHAT, |w| {
                content.write(w);
                chat_type.write(w);
            });
        } else {
            self.send_player_chat_message(message, chat_type);
        }
    }

    fn send_player_chat_message(&mut self, message: &PlayerChatMessage, chat_type: &crate::network::chat_type::Bound) {
        let chat = &mut self.chat;
        let global_index = chat.next_chat_index;
        chat.next_chat_index += 1;
        self.connection.send(clientbound::PLAYER_CHAT, |w| {
            w.varint(global_index);
            w.uuid(message.link.sender);
            w.varint(message.link.index);
            w.bool(message.signature.is_some());
            if let Some(signature) = &message.signature {
                signature.write(w);
            }
            message.signed_body.write_packed(w, &chat.message_signature_cache);
            w.bool(message.unsigned_content.is_some());
            if let Some(content) = &message.unsigned_content {
                content.write(w);
            }
            w.varint(0);
            chat_type.write(w);
        });
        if let Some(signature) = &message.signature {
            chat.message_signature_cache.push(&message.signed_body, Some(signature));
            chat.last_seen_messages.add_pending(signature);
            if chat.last_seen_messages.tracked_messages_count() > 4096 {
                self.disconnect("multiplayer.disconnect.too_many_pending_chats");
            }
        }
    }

    pub fn get_chat_session(&self) -> Option<&RemoteChatSession> {
        self.chat_session.as_ref().filter(|s| !s.has_expired())
    }
}

impl Server {
    pub(in crate::server) fn handle_chat(&mut self, id: EntityId, packet: &Packet) -> anyhow::Result<()> {
        let mut r = packet.reader();
        let content = r.string(256)?.to_owned();
        let time_stamp = r.i64()?;
        let salt = r.i64()?;
        let signature = MessageSignature::read_optional(&mut r)?;
        let update = Update::read(&mut r)?;
        let Some(last_seen) = self.unpack_and_apply_last_seen(id, &update) else { return Ok(()) };
        if self.try_handle_chat(id, &content, false) {
            let body = SignedMessageBody { content, time_stamp, salt, last_seen };
            self.tasks.push_back(Task::Chat { player: id, body, signature });
        }
        Ok(())
    }

    pub(in crate::server) fn run_chat(&mut self, id: EntityId, body: SignedMessageBody, signature: Option<MessageSignature>) {
        let enforce_secure_chat = self.enforce_secure_profile();
        let message = match self.level.player_mut(id).chat.decoder.unpack(signature, body, enforce_secure_chat) {
            Ok(message) => message,
            Err(e) => return self.handle_message_decode_failure(id, e),
        };
        let decorated = message.decorated_content();
        self.chain(id, move |server| {
            let message = message.with_unsigned_content(decorated);
            let chat_type = ChatType::Chat.bind(server.entity_display_name(id));
            broadcast_chat_message(server, &message, Some(id), &chat_type);
            server.detect_rate_spam(id, false);
        });
    }

    pub(in crate::server) fn handle_chat_command(&mut self, id: EntityId, packet: &Packet) -> anyhow::Result<()> {
        let command = packet.reader().string(32767)?.to_owned();
        if self.try_handle_chat(id, &command, true) {
            self.tasks.push_back(Task::Command { player: id, command });
        }
        Ok(())
    }

    pub(in crate::server) fn run_unsigned_command(&mut self, id: EntityId, command: &str) {
        let source = CommandSourceStack::player(self, id);
        let units: Vec<u16> = command.encode_utf16().collect();
        let parse = brigadier::parse(StringReader::new(&units), &source);
        if self.enforce_secure_profile() && signable_command::has_signable_arguments(&parse) {
            error!("Received unsigned command packet from {}, but the command requires signable arguments: {command}", self.level.player(id).profile.name);
            self.send_player_system_message(id, &invalid_command_signature(), false);
        } else {
            crate::commands::perform_command(self, parse, command, &source);
        }
        self.detect_rate_spam(id, true);
    }

    pub(in crate::server) fn handle_signed_chat_command(&mut self, id: EntityId, packet: &Packet) -> anyhow::Result<()> {
        let mut r = packet.reader();
        let command = r.string(32767)?.to_owned();
        let time_stamp = r.i64()?;
        let salt = r.i64()?;
        let count = r.varint()?;
        if count < 0 || count as usize > MAX_ARGUMENT_COUNT {
            anyhow::bail!("{count} elements exceeded max size of: {MAX_ARGUMENT_COUNT}");
        }
        let mut arguments = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let name = r.string(MAX_ARGUMENT_NAME_LENGTH)?.to_owned();
            arguments.push(ArgumentSignature { name, signature: MessageSignature::read(&mut r)? });
        }
        let update = Update::read(&mut r)?;
        let Some(last_seen) = self.unpack_and_apply_last_seen(id, &update) else { return Ok(()) };
        if self.try_handle_chat(id, &command, true) {
            self.tasks.push_back(Task::SignedCommand { player: id, command, time_stamp, salt, arguments, last_seen });
        }
        Ok(())
    }

    pub(in crate::server) fn run_signed_command(
        &mut self,
        id: EntityId,
        command: &str,
        time_stamp: i64,
        salt: i64,
        argument_signatures: Vec<ArgumentSignature>,
        last_seen: LastSeenMessages,
    ) {
        let source = CommandSourceStack::player(self, id);
        let units: Vec<u16> = command.encode_utf16().collect();
        let parse = brigadier::parse(StringReader::new(&units), &source);
        let signable = signable_command::of(&parse, &units);
        match self.collect_signed_arguments(id, command, time_stamp, salt, &argument_signatures, &signable, last_seen) {
            Ok(arguments) => {
                let source = source.with_signing_context(Arc::new(SignedArguments { chain: id, arguments }));
                crate::commands::perform_command(self, parse, command, &source);
            }
            Err(e) => self.handle_message_decode_failure(id, e),
        }
        self.detect_rate_spam(id, true);
    }

    #[allow(clippy::too_many_arguments)]
    fn collect_signed_arguments(
        &mut self,
        id: EntityId,
        command: &str,
        time_stamp: i64,
        salt: i64,
        argument_signatures: &[ArgumentSignature],
        signable: &[signable_command::Argument],
        last_seen: LastSeenMessages,
    ) -> Result<HashMap<String, PlayerChatMessage>, DecodeException> {
        let enforce_secure_chat = self.enforce_secure_profile();
        let decoder = &mut self.level.player_mut(id).chat.decoder;
        let mut arguments = HashMap::new();
        if argument_signatures.is_empty() {
            for argument in signable {
                let message = decoder.unpack(None, SignedMessageBody::unsigned(&argument.value), enforce_secure_chat)?;
                arguments.insert(argument.name.to_string(), message);
            }
            return Ok(arguments);
        }
        for client_argument in argument_signatures {
            let Some(expected) = signable.iter().find(|a| a.name == client_argument.name) else {
                decoder.set_chain_broken();
                return Err(signed_argument_mismatch(command, argument_signatures, signable));
            };
            let body = SignedMessageBody { content: expected.value.clone(), time_stamp, salt, last_seen: last_seen.clone() };
            let message = decoder.unpack(Some(client_argument.signature.clone()), body, enforce_secure_chat)?;
            arguments.insert(expected.name.to_string(), message);
        }
        if signable.iter().any(|expected| !arguments.contains_key(expected.name)) {
            return Err(signed_argument_mismatch(command, argument_signatures, signable));
        }
        Ok(arguments)
    }

    fn handle_message_decode_failure(&mut self, id: EntityId, e: DecodeException) {
        warn!("Failed to update secure chat state for {}: '{}'", self.level.player(id).profile.name, e.0.get_string());
        self.send_player_system_message(id, &e.0.with_color(ChatFormatting::Red), false);
    }

    pub(in crate::server) fn try_handle_chat(&mut self, id: EntityId, message: &str, is_command: bool) -> bool {
        let player = self.level.player_mut(id);
        if message.chars().any(is_illegal_chat_char) {
            player.disconnect("multiplayer.disconnect.illegal_characters");
            false
        } else if !is_command && player.client.chat_visibility == ChatVisibility::Hidden {
            let disabled = Component::translatable("chat.disabled.options").with_color(ChatFormatting::Red);
            player.connection.send(clientbound::SYSTEM_CHAT, |p| {
                disabled.write(p);
                p.bool(false);
            });
            false
        } else {
            player.reset_last_action_time();
            true
        }
    }

    fn unpack_and_apply_last_seen(&mut self, id: EntityId, update: &Update) -> Option<LastSeenMessages> {
        let player = self.level.player_mut(id);
        match player.chat.last_seen_messages.apply_update(update) {
            Ok(last_seen) => Some(last_seen),
            Err(e) => {
                error!("Failed to validate message acknowledgements from {}: {}", player.profile.name, e.0);
                player.disconnect(CHAT_VALIDATION_FAILED);
                None
            }
        }
    }

    fn detect_rate_spam(&mut self, id: EntityId, command: bool) {
        let uuid = self.level.player(id).profile.uuid;
        let is_op = self.is_op(uuid);
        let player = self.level.player_mut(id);
        let throttler = if command { &mut player.chat.command_spam_throttler } else { &mut player.chat.chat_spam_throttler };
        throttler.increment();
        if !throttler.is_under_threshold() && !is_op {
            player.disconnect("disconnect.spam");
        }
    }

    pub(in crate::server) fn handle_chat_ack(&mut self, id: EntityId, packet: &Packet) -> anyhow::Result<()> {
        let offset = packet.reader().varint()?;
        let player = self.level.player_mut(id);
        if let Err(e) = player.chat.last_seen_messages.apply_offset(offset) {
            error!("Failed to validate message acknowledgement offset from {}: {}", player.profile.name, e.0);
            player.disconnect(CHAT_VALIDATION_FAILED);
        }
        Ok(())
    }

    pub(in crate::server) fn handle_chat_session_update(&mut self, id: EntityId, packet: &Packet) -> anyhow::Result<()> {
        let data = remote_chat_session::Data::read(&mut packet.reader())?;
        let player = self.level.player(id);
        let old_key = player.chat.session.as_ref().map(|s| &s.profile_public_key.data);
        if old_key == Some(&data.profile_public_key) {
            return Ok(());
        }
        if old_key.is_some_and(|old| data.profile_public_key.expires_at < old.expires_at) {
            self.level.player_mut(id).disconnect(EXPIRED_PROFILE_PUBLIC_KEY);
            return Ok(());
        }
        let Some(validator) = self.services.profile_key_signature_validator() else {
            warn!("Ignoring chat session from {} due to missing Services public key", player.profile.name);
            return Ok(());
        };
        match data.validate(player.profile.uuid, &validator) {
            Ok(session) => self.reset_player_chat_state(id, session),
            Err(reason) => {
                error!("Failed to validate profile key");
                self.level.player_mut(id).disconnect_with(reason);
            }
        }
        Ok(())
    }

    fn reset_player_chat_state(&mut self, id: EntityId, session: RemoteChatSession) {
        let player = self.level.player_mut(id);
        player.chat.decoder = Decoder::chain(player.profile.uuid, session.session_id, session.profile_public_key.clone());
        player.chat.session = Some(session.clone());
        self.chain(id, move |server| {
            let player = server.level.player_mut(id);
            player.chat_session = Some(session);
            let player = server.level.player(id);
            let packet = crate::network::connection::encode(clientbound::PLAYER_INFO_UPDATE, |w| {
                w.u8(1 << 1);
                w.varint(1);
                w.uuid(player.profile.uuid);
                player.write_chat_session(w);
            });
            for other in server.level.players.values() {
                other.connection.send_raw(packet.clone());
            }
        });
    }

    pub fn chain(&mut self, player: EntityId, run: impl FnOnce(&mut Server) + 'static) {
        self.tasks.push_back(Task::Chained { player, run: Box::new(run) });
    }
}

fn signed_argument_mismatch(command: &str, client: &[ArgumentSignature], expected: &[signable_command::Argument]) -> DecodeException {
    let client_names: Vec<&str> = client.iter().map(|a| a.name.as_str()).collect();
    let expected_names: Vec<&str> = expected.iter().map(|a| a.name).collect();
    error!(
        "Signed command mismatch between server and client ('{command}'): got [{}] from client, but expected [{}]",
        client_names.join(", "),
        expected_names.join(", ")
    );
    DecodeException(invalid_command_signature())
}
