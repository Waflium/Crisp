use std::sync::LazyLock;

use serde_json::{Map, Value};

use super::api::schema::{self, Schema};
use super::methods::{
    ClientInfo, allowlist_service, banlist_service, discovery_service, game_rules_service, ip_banlist_service, operator_service, player_service,
    server_settings_service, server_state_service,
};
use crate::server::Server;

pub enum RpcFailure {
    InvalidParams(String),
    InvalidRequest(String),
    MethodNotFound(String),
}

pub type ParameterlessFunction = fn(&mut Server, ClientInfo) -> Value;
pub type ParameterFunction = fn(&mut Server, &Value, ClientInfo) -> Result<Value, String>;

pub enum Function {
    Parameterless(ParameterlessFunction),
    Parameter(ParameterFunction),
    Discover,
}

pub struct IncomingRpcMethod {
    pub name: String,
    pub description: &'static str,
    pub param: Option<(&'static str, Schema)>,
    pub result: (&'static str, Schema),
    pub run_on_main_thread: bool,
    pub discoverable: bool,
    pub allow_pre_server_init: bool,
    pub function: Function,
}

impl IncomingRpcMethod {
    pub fn apply(&self, server: Option<&mut Server>, params: Option<&Value>, client: ClientInfo) -> Result<Value, RpcFailure> {
        match &self.function {
            Function::Discover | Function::Parameterless(_) if params.is_some_and(|p| !p.as_array().is_some_and(Vec::is_empty)) => {
                Err(RpcFailure::InvalidParams("Expected no params, or an empty array".into()))
            }
            Function::Discover => Ok(discovery_service::discover()),
            Function::Parameterless(function) => Ok(function(server.unwrap(), client)),
            Function::Parameter(function) => {
                let element = match params {
                    Some(Value::Object(object)) => {
                        let name = self.param.as_ref().unwrap().0;
                        object
                            .get(name)
                            .ok_or_else(|| RpcFailure::InvalidParams(format!("Params passed by-name, but expected param [{name}] does not exist")))?
                    }
                    Some(Value::Array(array)) if array.len() == 1 => &array[0],
                    Some(Value::Array(_)) => return Err(RpcFailure::InvalidParams("Expected exactly one element in the params array".into())),
                    _ => return Err(RpcFailure::InvalidParams("Expected params as array or named".into())),
                };
                function(server.unwrap(), element, client).map_err(RpcFailure::InvalidParams)
            }
        }
    }

    pub fn info(&self) -> Value {
        let mut object = Map::new();
        object.insert("name".into(), Value::String(self.name.clone()));
        object.insert("description".into(), Value::String(self.description.into()));
        let params = self.param.iter().map(|(name, schema)| param_info(name, schema)).collect();
        object.insert("params".into(), Value::Array(params));
        object.insert("result".into(), result_info(self.result.0, &self.result.1));
        Value::Object(object)
    }
}

pub fn param_info(name: &str, schema: &Schema) -> Value {
    let mut object = Map::new();
    object.insert("name".into(), Value::String(name.into()));
    object.insert("schema".into(), schema.to_json());
    object.insert("required".into(), Value::Bool(true));
    Value::Object(object)
}

pub fn result_info(name: &str, schema: &Schema) -> Value {
    let mut object = Map::new();
    object.insert("name".into(), Value::String(name.into()));
    object.insert("schema".into(), schema.to_json());
    Value::Object(object)
}

struct Builder {
    description: &'static str,
    param: Option<(&'static str, Schema)>,
    result: Option<(&'static str, Schema)>,
    run_on_main_thread: bool,
    discoverable: bool,
    allow_pre_server_init: bool,
    function: Function,
}

fn method(function: Function) -> Builder {
    Builder { description: "", param: None, result: None, run_on_main_thread: true, discoverable: true, allow_pre_server_init: false, function }
}

fn parameterless(function: ParameterlessFunction) -> Builder {
    method(Function::Parameterless(function))
}

fn with_param(function: ParameterFunction) -> Builder {
    method(Function::Parameter(function))
}

impl Builder {
    fn description(mut self, description: &'static str) -> Builder {
        self.description = description;
        self
    }

    fn param(mut self, name: &'static str, schema: Schema) -> Builder {
        self.param = Some((name, schema));
        self
    }

    fn response(mut self, name: &'static str, schema: Schema) -> Builder {
        self.result = Some((name, schema));
        self
    }

    fn register(self, registry: &mut Vec<IncomingRpcMethod>, key: &str) {
        registry.push(IncomingRpcMethod {
            name: format!("minecraft:{key}"),
            description: self.description,
            param: self.param,
            result: self.result.expect("No response defined"),
            run_on_main_thread: self.run_on_main_thread,
            discoverable: self.discoverable,
            allow_pre_server_init: self.allow_pre_server_init,
            function: self.function,
        });
    }
}

pub static INCOMING_RPC_METHODS: LazyLock<Vec<IncomingRpcMethod>> = LazyLock::new(bootstrap);

pub fn get(name: &str) -> Option<&'static IncomingRpcMethod> {
    INCOMING_RPC_METHODS.iter().find(|m| m.name == name)
}

fn bootstrap() -> Vec<IncomingRpcMethod> {
    let mut registry = Vec::new();
    register_allow_list_service(&mut registry);
    register_banlist_service(&mut registry);
    register_ip_banlist_service(&mut registry);
    register_player_service(&mut registry);
    register_operator_service(&mut registry);
    register_server_state_service(&mut registry);
    register_server_settings_service(&mut registry);
    register_game_rule_service(&mut registry);
    let mut discover = method(Function::Discover).response("result", schema::DISCOVERY_SCHEMA);
    discover.discoverable = false;
    discover.allow_pre_server_init = true;
    discover.run_on_main_thread = false;
    discover.register(&mut registry, "rpc.discover");
    registry
}

fn register_allow_list_service(r: &mut Vec<IncomingRpcMethod>) {
    use allowlist_service as s;
    let players = || schema::PLAYER_SCHEMA.as_array();
    parameterless(s::get).description("Get the allowlist").response("allowlist", players()).register(r, "allowlist");
    with_param(s::set).description("Set the allowlist").param("players", players()).response("allowlist", players()).register(r, "allowlist/set");
    with_param(s::add).description("Add players to allowlist").param("add", players()).response("allowlist", players()).register(r, "allowlist/add");
    with_param(s::remove)
        .description("Remove players from allowlist")
        .param("remove", players())
        .response("allowlist", players())
        .register(r, "allowlist/remove");
    parameterless(s::clear).description("Clear all players in allowlist").response("allowlist", players()).register(r, "allowlist/clear");
}

fn register_banlist_service(r: &mut Vec<IncomingRpcMethod>) {
    use banlist_service as s;
    let bans = || schema::PLAYER_BAN_SCHEMA.as_array();
    parameterless(s::get).description("Get the ban list").response("banlist", bans()).register(r, "bans");
    with_param(s::set).description("Set the banlist").param("bans", bans()).response("banlist", bans()).register(r, "bans/set");
    with_param(s::add).description("Add players to ban list").param("add", bans()).response("banlist", bans()).register(r, "bans/add");
    with_param(s::remove)
        .description("Remove players from ban list")
        .param("remove", schema::PLAYER_SCHEMA.as_array())
        .response("banlist", bans())
        .register(r, "bans/remove");
    parameterless(s::clear).description("Clear all players in ban list").response("banlist", bans()).register(r, "bans/clear");
}

fn register_ip_banlist_service(r: &mut Vec<IncomingRpcMethod>) {
    use ip_banlist_service as s;
    let bans = || schema::IP_BAN_SCHEMA.as_array();
    parameterless(s::get).description("Get the ip ban list").response("banlist", bans()).register(r, "ip_bans");
    with_param(s::set).description("Set the ip banlist").param("banlist", bans()).response("banlist", bans()).register(r, "ip_bans/set");
    with_param(s::add)
        .description("Add ip to ban list")
        .param("add", schema::INCOMING_IP_BAN_SCHEMA.as_array())
        .response("banlist", bans())
        .register(r, "ip_bans/add");
    with_param(s::remove)
        .description("Remove ip from ban list")
        .param("ip", schema::STRING_SCHEMA.as_array())
        .response("banlist", bans())
        .register(r, "ip_bans/remove");
    parameterless(s::clear).description("Clear all ips in ban list").response("banlist", bans()).register(r, "ip_bans/clear");
}

fn register_player_service(r: &mut Vec<IncomingRpcMethod>) {
    use player_service as s;
    parameterless(s::get).description("Get all connected players").response("players", schema::PLAYER_SCHEMA.as_array()).register(r, "players");
    with_param(s::kick)
        .description("Kick players")
        .param("kick", schema::KICK_PLAYER_SCHEMA.as_array())
        .response("kicked", schema::PLAYER_SCHEMA.as_array())
        .register(r, "players/kick");
}

fn register_operator_service(r: &mut Vec<IncomingRpcMethod>) {
    use operator_service as s;
    let operators = || schema::OPERATOR_SCHEMA.as_array();
    parameterless(s::get).description("Get all oped players").response("operators", operators()).register(r, "operators");
    with_param(s::set).description("Set all oped players").param("operators", operators()).response("operators", operators()).register(r, "operators/set");
    with_param(s::add).description("Op players").param("add", operators()).response("operators", operators()).register(r, "operators/add");
    with_param(s::remove)
        .description("Deop players")
        .param("remove", schema::PLAYER_SCHEMA.as_array())
        .response("operators", operators())
        .register(r, "operators/remove");
    parameterless(s::clear).description("Deop all players").response("operators", operators()).register(r, "operators/clear");
}

fn register_server_state_service(r: &mut Vec<IncomingRpcMethod>) {
    use server_state_service as s;
    parameterless(s::status).description("Get server status").response("status", schema::SERVER_STATE_SCHEMA.as_ref()).register(r, "server/status");
    with_param(s::save).description("Save server state").param("flush", schema::BOOL_SCHEMA).response("saving", schema::BOOL_SCHEMA).register(r, "server/save");
    parameterless(s::stop).description("Stop server").response("stopping", schema::BOOL_SCHEMA).register(r, "server/stop");
    with_param(s::system_message)
        .description("Send a system message")
        .param("message", schema::SYSTEM_MESSAGE_SCHEMA.as_ref())
        .response("sent", schema::BOOL_SCHEMA)
        .register(r, "server/system_message");
}

fn setting(r: &mut Vec<IncomingRpcMethod>, key: &str, get: (ParameterlessFunction, &'static str), set: (ParameterFunction, &'static str), param: &'static str, result: &'static str, schema: fn() -> Schema) {
    parameterless(get.0).description(get.1).response(result, schema()).register(r, &format!("serversettings/{key}"));
    with_param(set.0).description(set.1).param(param, schema()).response(result, schema()).register(r, &format!("serversettings/{key}/set"));
}

fn register_server_settings_service(r: &mut Vec<IncomingRpcMethod>) {
    use server_settings_service as s;
    let bool_schema = || schema::BOOL_SCHEMA;
    let int_schema = || schema::INT_SCHEMA;
    setting(
        r,
        "autosave",
        (s::autosave, "Get whether automatic world saving is enabled on the server"),
        (s::set_autosave, "Enable or disable automatic world saving on the server"),
        "enable",
        "enabled",
        bool_schema,
    );
    setting(
        r,
        "difficulty",
        (s::difficulty, "Get the current difficulty level of the server"),
        (s::set_difficulty, "Set the difficulty level of the server"),
        "difficulty",
        "difficulty",
        || schema::DIFFICULTY_SCHEMA.as_ref(),
    );
    setting(
        r,
        "enforce_allowlist",
        (s::enforce_allowlist, "Get whether allowlist enforcement is enabled (kicks players immediately when removed from allowlist)"),
        (s::set_enforce_allowlist, "Enable or disable allowlist enforcement (when enabled, players are kicked immediately upon removal from allowlist)"),
        "enforce",
        "enforced",
        bool_schema,
    );
    setting(
        r,
        "use_allowlist",
        (s::using_allowlist, "Get whether the allowlist is enabled on the server"),
        (s::set_using_allowlist, "Enable or disable the allowlist on the server (controls whether only allowlisted players can join)"),
        "use",
        "used",
        bool_schema,
    );
    setting(
        r,
        "max_players",
        (s::max_players, "Get the maximum number of players allowed to connect to the server"),
        (s::set_max_players, "Set the maximum number of players allowed to connect to the server"),
        "max",
        "max",
        int_schema,
    );
    setting(
        r,
        "pause_when_empty_seconds",
        (s::pause_when_empty, "Get the number of seconds before the game is automatically paused when no players are online"),
        (s::set_pause_when_empty, "Set the number of seconds before the game is automatically paused when no players are online"),
        "seconds",
        "seconds",
        int_schema,
    );
    setting(
        r,
        "player_idle_timeout",
        (s::player_idle_timeout, "Get the number of seconds before idle players are automatically kicked from the server"),
        (s::set_player_idle_timeout, "Set the number of seconds before idle players are automatically kicked from the server"),
        "seconds",
        "seconds",
        int_schema,
    );
    setting(
        r,
        "allow_flight",
        (s::allow_flight, "Get whether flight is allowed for players in Survival mode"),
        (s::set_allow_flight, "Allow or disallow flight for players in Survival mode"),
        "allow",
        "allowed",
        bool_schema,
    );
    setting(
        r,
        "motd",
        (s::motd, "Get the server's message of the day displayed to players"),
        (s::set_motd, "Set the server's message of the day displayed to players"),
        "message",
        "message",
        || schema::STRING_SCHEMA,
    );
    setting(
        r,
        "spawn_protection_radius",
        (s::spawn_protection, "Get the spawn protection radius in blocks (only operators can edit within this area)"),
        (s::set_spawn_protection, "Set the spawn protection radius in blocks (only operators can edit within this area)"),
        "radius",
        "radius",
        int_schema,
    );
    setting(
        r,
        "force_game_mode",
        (s::force_game_mode, "Get whether players are forced to use the server's default game mode"),
        (s::set_force_game_mode, "Enable or disable forcing players to use the server's default game mode"),
        "force",
        "forced",
        bool_schema,
    );
    setting(
        r,
        "game_mode",
        (s::game_mode, "Get the server's default game mode"),
        (s::set_game_mode, "Set the server's default game mode"),
        "mode",
        "mode",
        || schema::GAME_TYPE_SCHEMA.as_ref(),
    );
    setting(
        r,
        "view_distance",
        (s::view_distance, "Get the server's view distance in chunks"),
        (s::set_view_distance, "Set the server's view distance in chunks"),
        "distance",
        "distance",
        int_schema,
    );
    setting(
        r,
        "simulation_distance",
        (s::simulation_distance, "Get the server's simulation distance in chunks"),
        (s::set_simulation_distance, "Set the server's simulation distance in chunks"),
        "distance",
        "distance",
        int_schema,
    );
    setting(
        r,
        "accept_transfers",
        (s::accept_transfers, "Get whether the server accepts player transfers from other servers"),
        (s::set_accept_transfers, "Enable or disable accepting player transfers from other servers"),
        "accept",
        "accepted",
        bool_schema,
    );
    setting(
        r,
        "status_heartbeat_interval",
        (s::status_heartbeat_interval, "Get the interval in seconds between server status heartbeats"),
        (s::set_status_heartbeat_interval, "Set the interval in seconds between server status heartbeats"),
        "seconds",
        "seconds",
        int_schema,
    );
    setting(
        r,
        "operator_user_permission_level",
        (s::operator_user_permission_level, "Get default operator permission level"),
        (s::set_operator_user_permission_level, "Set default operator permission level"),
        "level",
        "level",
        || schema::PERMISSION_LEVEL_SCHEMA,
    );
    setting(
        r,
        "hide_online_players",
        (s::hides_online_players, "Get whether the server hides online player information from status queries"),
        (s::set_hides_online_players, "Enable or disable hiding online player information from status queries"),
        "hide",
        "hidden",
        bool_schema,
    );
    setting(
        r,
        "status_replies",
        (s::replies_to_status, "Get whether the server responds to connection status requests"),
        (s::set_replies_to_status, "Enable or disable the server responding to connection status requests"),
        "enable",
        "enabled",
        bool_schema,
    );
    setting(
        r,
        "entity_broadcast_range",
        (s::entity_broadcast_range_percentage, "Get the entity broadcast range as a percentage"),
        (s::set_entity_broadcast_range_percentage, "Set the entity broadcast range as a percentage"),
        "percentage_points",
        "percentage_points",
        int_schema,
    );
}

fn register_game_rule_service(r: &mut Vec<IncomingRpcMethod>) {
    use game_rules_service as s;
    parameterless(s::get)
        .description("Get the available game rule keys and their current values")
        .response("gamerules", schema::TYPED_GAME_RULE_SCHEMA.as_ref().as_array())
        .register(r, "gamerules");
    with_param(s::update)
        .description("Update game rule value")
        .param("gamerule", schema::UNTYPED_GAME_RULE_SCHEMA.as_ref())
        .response("gamerule", schema::TYPED_GAME_RULE_SCHEMA.as_ref())
        .register(r, "gamerules/update");
}
