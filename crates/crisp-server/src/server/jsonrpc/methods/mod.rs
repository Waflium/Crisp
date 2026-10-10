pub mod allowlist_service;
pub mod banlist_service;
pub mod discovery_service;
pub mod game_rules_service;
pub mod ip_banlist_service;
pub mod operator_service;
pub mod player_service;
pub mod server_settings_service;
pub mod server_state_service;

use std::fmt;

use serde_json::Value;

use super::codec::{self, DecodeResult, Fields};
use crate::network::chat::{Arg, Component};
use crate::server::Server;
use crate::server::players::NameAndId;

#[derive(Clone, Copy, Debug)]
pub struct ClientInfo {
    pub connection_id: i32,
}

pub fn log(client: ClientInfo, message: fmt::Arguments) {
    info!("RPC Connection #{}: {message}", client.connection_id);
}

pub fn get_user(server: &Server, id: Option<u128>, name: Option<&str>) -> Option<NameAndId> {
    match (id, name) {
        (Some(id), _) => server.name_to_id_cache.get_by_id(id).or_else(|| server.services.fetch_profile(id)),
        (None, Some(name)) => server.name_to_id_cache.get(&server.services, name),
        (None, None) => None,
    }
}

pub struct Message {
    pub literal: Option<String>,
    pub translatable: Option<String>,
    pub translatable_params: Option<Vec<String>>,
}

impl Message {
    pub fn decode(value: &Value) -> DecodeResult<Message> {
        let mut fields = Fields::of(value)?;
        let literal = fields.optional("literal", codec::string);
        let translatable = fields.optional("translatable", codec::string);
        let translatable_params = fields.lenient("translatableParams", |v| codec::list(v, codec::string));
        fields.finish(|| Message { literal, translatable, translatable_params })
    }

    pub fn as_component(&self) -> Option<Component> {
        match (&self.translatable, &self.translatable_params) {
            (Some(key), Some(params)) => Some(Component::translatable_with(key, params.iter().map(|p| Arg::String(p.clone())).collect())),
            (Some(key), None) => Some(Component::translatable(key)),
            (None, _) => self.literal.as_deref().map(Component::literal),
        }
    }
}

pub fn decode_params<T>(value: &Value, decode: impl Fn(&Value) -> DecodeResult<T>) -> Result<T, String> {
    decode(value).map_err(codec::message)
}
