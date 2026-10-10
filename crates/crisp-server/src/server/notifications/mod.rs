pub mod server_activity_monitor;

use std::sync::Arc;

use serde_json::Value;

use super::jsonrpc::api::PlayerDto;
use super::jsonrpc::management_server::ManagementServer;
use super::jsonrpc::methods::banlist_service::UserBanDto;
use super::jsonrpc::methods::game_rules_service::typed_rule;
use super::jsonrpc::methods::ip_banlist_service;
use super::jsonrpc::methods::operator_service;
use super::jsonrpc::outgoing_rpc_methods::{self as methods, OutgoingRpcMethod};
use super::players::{IpBanEntry, NameAndId, OpEntry, UserBanEntry};

#[derive(Clone, Default)]
pub struct NotificationManager {
    json_rpc: Option<Arc<ManagementServer>>,
}

impl NotificationManager {
    pub fn new(json_rpc: Option<Arc<ManagementServer>>) -> NotificationManager {
        NotificationManager { json_rpc }
    }

    pub fn management_server(&self) -> Option<&Arc<ManagementServer>> {
        self.json_rpc.as_ref()
    }

    fn broadcast(&self, method: &OutgoingRpcMethod, params: impl FnOnce() -> Option<Value>) {
        if let Some(server) = &self.json_rpc {
            server.broadcast(method, params);
        }
    }

    pub fn player_joined(&self, player: &NameAndId) {
        self.broadcast(&methods::PLAYER_JOINED, || Some(PlayerDto::from(player).encode()));
    }

    pub fn player_left(&self, player: &NameAndId) {
        self.broadcast(&methods::PLAYER_LEFT, || Some(PlayerDto::from(player).encode()));
    }

    pub fn server_started(&self) {
        self.broadcast(&methods::SERVER_STARTED, || None);
    }

    pub fn server_shutting_down(&self) {
        self.broadcast(&methods::SERVER_SHUTTING_DOWN, || None);
    }

    pub fn server_save_started(&self) {
        self.broadcast(&methods::SERVER_SAVE_STARTED, || None);
    }

    pub fn server_save_completed(&self) {
        self.broadcast(&methods::SERVER_SAVE_COMPLETED, || None);
    }

    pub fn server_activity_occured(&self) {
        self.broadcast(&methods::SERVER_ACTIVITY_OCCURRED, || None);
    }

    pub fn player_oped(&self, operator: &OpEntry) {
        self.broadcast(&methods::PLAYER_OPED, || Some(operator_service::encode_entry(operator)));
    }

    pub fn player_deoped(&self, operator: &OpEntry) {
        self.broadcast(&methods::PLAYER_DEOPED, || Some(operator_service::encode_entry(operator)));
    }

    pub fn player_added_to_allowlist(&self, player: &NameAndId) {
        self.broadcast(&methods::PLAYER_ADDED_TO_ALLOWLIST, || Some(PlayerDto::from(player).encode()));
    }

    pub fn player_removed_from_allowlist(&self, player: &NameAndId) {
        self.broadcast(&methods::PLAYER_REMOVED_FROM_ALLOWLIST, || Some(PlayerDto::from(player).encode()));
    }

    pub fn ip_banned(&self, ban: &IpBanEntry) {
        self.broadcast(&methods::IP_BANNED, || Some(ip_banlist_service::encode_entry(ban)));
    }

    pub fn ip_unbanned(&self, ip: &str) {
        self.broadcast(&methods::IP_UNBANNED, || Some(Value::String(ip.into())));
    }

    pub fn player_banned(&self, ban: &UserBanEntry) {
        self.broadcast(&methods::PLAYER_BANNED, || Some(UserBanDto::from_entry(ban)));
    }

    pub fn player_unbanned(&self, player: &NameAndId) {
        self.broadcast(&methods::PLAYER_UNBANNED, || Some(PlayerDto::from(player).encode()));
    }

    pub fn on_game_rule_changed(&self, rule: crisp_data::GameRule, value: i32) {
        self.broadcast(&methods::GAMERULE_CHANGED, || Some(typed_rule(rule, value)));
    }
}
