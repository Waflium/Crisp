use serde_json::{Map, Value};

use super::player_service::players;
use super::{ClientInfo, Message, decode_params, log};
use crate::server::Server;
use crate::server::jsonrpc::api::PlayerDto;
use crate::server::jsonrpc::codec::{self, DecodeResult, Fields};

fn version() -> Value {
    let mut object = Map::new();
    object.insert("name".into(), Value::String(crisp_data::VERSION_NAME.into()));
    object.insert("protocol".into(), Value::from(crisp_data::PROTOCOL_VERSION));
    Value::Object(object)
}

pub fn server_state(started: bool, players: &[PlayerDto]) -> Value {
    let mut object = Map::new();
    object.insert("started".into(), Value::Bool(started));
    if !players.is_empty() {
        object.insert("players".into(), Value::Array(players.iter().map(PlayerDto::encode).collect()));
    }
    object.insert("version".into(), version());
    Value::Object(object)
}

pub fn not_started() -> Value {
    server_state(false, &[])
}

pub fn status(server: &mut Server, _client: ClientInfo) -> Value {
    current_status(server)
}

pub fn current_status(server: &Server) -> Value {
    if !server.is_ready() { not_started() } else { server_state(true, &players(server)) }
}

pub fn save(server: &mut Server, params: &Value, client: ClientInfo) -> Result<Value, String> {
    let flush = decode_params(params, codec::boolean)?;
    log(client, format_args!("Save everything. SuppressLogs: true, flush: {flush}, force: true"));
    Ok(Value::Bool(server.save_everything(true, flush, true)))
}

pub fn stop(server: &mut Server, client: ClientInfo) -> Value {
    log(client, format_args!("Halt server. WaitForShutdown: false"));
    server.halt();
    Value::Bool(true)
}

struct SystemMessage {
    message: Message,
    overlay: bool,
    receiving_players: Option<Vec<PlayerDto>>,
}

impl SystemMessage {
    fn decode(value: &Value) -> DecodeResult<SystemMessage> {
        let mut fields = Fields::of(value)?;
        let message = fields.required("message", Message::decode);
        let overlay = fields.required("overlay", codec::boolean);
        let receiving_players = fields.lenient("receivingPlayers", |v| codec::list(v, PlayerDto::decode));
        fields.finish(|| SystemMessage { message: message.unwrap(), overlay: overlay.unwrap(), receiving_players })
    }
}

pub fn system_message(server: &mut Server, params: &Value, client: ClientInfo) -> Result<Value, String> {
    let system_message = decode_params(params, SystemMessage::decode)?;
    let Some(component) = system_message.message.as_component() else { return Ok(Value::Bool(false)) };
    match system_message.receiving_players {
        Some(players) => {
            if players.is_empty() {
                return Ok(Value::Bool(false));
            }
            for dto in players {
                let player = match (dto.id, &dto.name) {
                    (Some(id), _) => server.player_by_uuid(id),
                    (None, Some(name)) => server.player_by_name(name),
                    (None, None) => continue,
                };
                if let Some(player) = player {
                    server.send_player_system_message(player, &component, system_message.overlay);
                }
            }
        }
        None => {
            log(client, format_args!("Broadcast system message (overlay: {}): '{}'", system_message.overlay, component.get_string()));
            let ids: Vec<_> = server.level.players.keys().copied().collect();
            for player in ids {
                server.send_player_system_message(player, &component, system_message.overlay);
            }
        }
    }
    Ok(Value::Bool(true))
}
