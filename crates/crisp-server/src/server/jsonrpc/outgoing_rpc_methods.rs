use serde_json::{Map, Value};

use super::api::schema::{self, Schema};
use super::incoming_rpc_methods::param_info;

pub struct OutgoingRpcMethod {
    pub name: &'static str,
    pub description: &'static str,
    pub param: Option<(&'static str, fn() -> Schema)>,
    pub allow_pre_server_init: bool,
}

impl OutgoingRpcMethod {
    pub fn info(&self) -> Value {
        let mut object = Map::new();
        object.insert("name".into(), Value::String(self.name.into()));
        object.insert("description".into(), Value::String(self.description.into()));
        let params = self.param.iter().map(|(name, schema)| param_info(name, &schema())).collect();
        object.insert("params".into(), Value::Array(params));
        Value::Object(object)
    }
}

macro_rules! notification {
    ($name:ident, $key:literal, $description:literal) => {
        pub static $name: OutgoingRpcMethod =
            OutgoingRpcMethod { name: concat!("minecraft:notification/", $key), description: $description, param: None, allow_pre_server_init: false };
    };
    ($name:ident, $key:literal, $description:literal, $param:literal, $schema:expr) => {
        pub static $name: OutgoingRpcMethod = OutgoingRpcMethod {
            name: concat!("minecraft:notification/", $key),
            description: $description,
            param: Some(($param, || $schema)),
            allow_pre_server_init: false,
        };
    };
    (pre_init $name:ident, $key:literal, $description:literal $(, $param:literal, $schema:expr)?) => {
        pub static $name: OutgoingRpcMethod = OutgoingRpcMethod {
            name: concat!("minecraft:notification/", $key),
            description: $description,
            param: notification!(@param $($param, $schema)?),
            allow_pre_server_init: true,
        };
    };
    (@param) => { None };
    (@param $param:literal, $schema:expr) => { Some(($param, || $schema)) };
}

notification!(SERVER_STARTED, "server/started", "Server started");
notification!(SERVER_SHUTTING_DOWN, "server/stopping", "Server shutting down");
notification!(SERVER_SAVE_STARTED, "server/saving", "Server save started");
notification!(SERVER_SAVE_COMPLETED, "server/saved", "Server save completed");
notification!(SERVER_ACTIVITY_OCCURRED, "server/activity", "Server activity occurred. Rate limited to 1 notification per 30 seconds");
notification!(pre_init WORLD_UPGRADE_STARTED, "world/upgrade_started", "World upgrade started");
notification!(
    pre_init WORLD_UPGRADE_PROGRESS,
    "world/upgrade_progress",
    "World upgrade progress. Rate limited to 1 notification per second",
    "progress",
    schema::NUMBER_SCHEMA
);
notification!(pre_init WORLD_UPGRADE_FINISHED, "world/upgrade_finished", "World upgrade finished");
notification!(pre_init WORLD_UPGRADE_FAILED, "world/upgrade_failed", "World upgrade failed", "reason", schema::STRING_SCHEMA);
notification!(PLAYER_JOINED, "players/joined", "Player joined", "player", schema::PLAYER_SCHEMA.as_ref());
notification!(PLAYER_LEFT, "players/left", "Player left", "player", schema::PLAYER_SCHEMA.as_ref());
notification!(PLAYER_OPED, "operators/added", "Player was oped", "player", schema::OPERATOR_SCHEMA.as_ref());
notification!(PLAYER_DEOPED, "operators/removed", "Player was deoped", "player", schema::OPERATOR_SCHEMA.as_ref());
notification!(PLAYER_ADDED_TO_ALLOWLIST, "allowlist/added", "Player was added to allowlist", "player", schema::PLAYER_SCHEMA.as_ref());
notification!(PLAYER_REMOVED_FROM_ALLOWLIST, "allowlist/removed", "Player was removed from allowlist", "player", schema::PLAYER_SCHEMA.as_ref());
notification!(IP_BANNED, "ip_bans/added", "Ip was added to ip ban list", "player", schema::IP_BAN_SCHEMA.as_ref());
notification!(IP_UNBANNED, "ip_bans/removed", "Ip was removed from ip ban list", "player", schema::STRING_SCHEMA);
notification!(PLAYER_BANNED, "bans/added", "Player was added to ban list", "player", schema::PLAYER_BAN_SCHEMA.as_ref());
notification!(PLAYER_UNBANNED, "bans/removed", "Player was removed from ban list", "player", schema::PLAYER_SCHEMA.as_ref());
notification!(GAMERULE_CHANGED, "gamerules/updated", "Gamerule was changed", "gamerule", schema::TYPED_GAME_RULE_SCHEMA.as_ref());
notification!(
    pre_init STATUS_HEARTBEAT,
    "server/status",
    "Server status heartbeat, including before the server has spun up",
    "status",
    schema::SERVER_STATE_SCHEMA.as_ref()
);

pub static OUTGOING_RPC_METHODS: [&OutgoingRpcMethod; 21] = [
    &SERVER_STARTED,
    &SERVER_SHUTTING_DOWN,
    &SERVER_SAVE_STARTED,
    &SERVER_SAVE_COMPLETED,
    &SERVER_ACTIVITY_OCCURRED,
    &WORLD_UPGRADE_STARTED,
    &WORLD_UPGRADE_PROGRESS,
    &WORLD_UPGRADE_FINISHED,
    &WORLD_UPGRADE_FAILED,
    &PLAYER_JOINED,
    &PLAYER_LEFT,
    &PLAYER_OPED,
    &PLAYER_DEOPED,
    &PLAYER_ADDED_TO_ALLOWLIST,
    &PLAYER_REMOVED_FROM_ALLOWLIST,
    &IP_BANNED,
    &IP_UNBANNED,
    &PLAYER_BANNED,
    &PLAYER_UNBANNED,
    &GAMERULE_CHANGED,
    &STATUS_HEARTBEAT,
];
