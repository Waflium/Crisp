use serde_json::{Map, Value};

use super::{ClientInfo, decode_params, get_user, log};
use crate::server::Server;
use crate::server::jsonrpc::api::PlayerDto;
use crate::server::jsonrpc::codec::{self, DecodeResult, Fields};
use crate::server::players::{NameAndId, OpEntry};

#[derive(Clone, PartialEq, Eq)]
struct Op {
    user: NameAndId,
    permission_level: Option<i32>,
    bypasses_player_limit: Option<bool>,
}

struct OperatorDto {
    player: PlayerDto,
    permission_level: Option<i32>,
    bypasses_player_limit: Option<bool>,
}

fn permission_level(value: &Value) -> DecodeResult<i32> {
    Ok(codec::int(value)?.clamp(0, 4))
}

impl OperatorDto {
    fn decode(value: &Value) -> DecodeResult<OperatorDto> {
        let mut fields = Fields::of(value)?;
        let player = fields.required("player", PlayerDto::decode);
        let permission_level = fields.optional("permissionLevel", permission_level);
        let bypasses_player_limit = fields.optional("bypassesPlayerLimit", codec::boolean);
        fields.finish(|| OperatorDto { player: player.unwrap(), permission_level, bypasses_player_limit })
    }
}

pub fn encode_entry(entry: &OpEntry) -> Value {
    let mut object = Map::new();
    object.insert("player".into(), PlayerDto::from(&entry.user).encode());
    object.insert("permissionLevel".into(), Value::from(entry.level));
    object.insert("bypassesPlayerLimit".into(), Value::Bool(entry.bypasses_player_limit));
    Value::Object(object)
}

fn encode(server: &Server) -> Value {
    Value::Array(server.ops.entries().into_iter().map(encode_entry).collect())
}

fn op(server: &mut Server, op: &Op, client: ClientInfo) {
    log(client, format_args!("Op '{}'", op.user));
    server.op(op.user.clone(), op.permission_level, op.bypasses_player_limit);
}

fn deop(server: &mut Server, user: &NameAndId, client: ClientInfo) {
    log(client, format_args!("Deop '{user}'"));
    server.deop(user);
}

fn resolve(server: &Server, operators: &[OperatorDto]) -> Vec<Op> {
    operators
        .iter()
        .filter_map(|o| {
            get_user(server, o.player.id, o.player.name.as_deref())
                .map(|user| Op { user, permission_level: o.permission_level, bypasses_player_limit: o.bypasses_player_limit })
        })
        .collect()
}

fn unique(ops: Vec<Op>) -> Vec<Op> {
    let mut unique: Vec<Op> = Vec::new();
    for op in ops {
        if !unique.contains(&op) {
            unique.push(op);
        }
    }
    unique
}

fn operators(value: &Value) -> Result<Vec<OperatorDto>, String> {
    decode_params(value, |v| codec::list(v, OperatorDto::decode))
}

pub fn get(server: &mut Server, _client: ClientInfo) -> Value {
    encode(server)
}

pub fn clear(server: &mut Server, client: ClientInfo) -> Value {
    log(client, format_args!("Clear operator list"));
    server.ops.clear();
    encode(server)
}

pub fn remove(server: &mut Server, params: &Value, client: ClientInfo) -> Result<Value, String> {
    let players = decode_params(params, |v| codec::list(v, PlayerDto::decode))?;
    let users: Vec<NameAndId> = players.iter().filter_map(|p| get_user(server, p.id, p.name.as_deref())).collect();
    for user in &users {
        deop(server, user, client);
    }
    Ok(encode(server))
}

pub fn add(server: &mut Server, params: &Value, client: ClientInfo) -> Result<Value, String> {
    let operators = operators(params)?;
    for operator in resolve(server, &operators) {
        op(server, &operator, client);
    }
    Ok(encode(server))
}

pub fn set(server: &mut Server, params: &Value, client: ClientInfo) -> Result<Value, String> {
    let operators = operators(params)?;
    let final_operators = unique(resolve(server, &operators));
    let current_operators = unique(
        server
            .ops
            .entries()
            .into_iter()
            .map(|e| Op { user: e.user.clone(), permission_level: Some(e.level), bypasses_player_limit: Some(e.bypasses_player_limit) })
            .collect(),
    );
    for operator in current_operators.iter().filter(|o| !final_operators.contains(o)) {
        deop(server, &operator.user, client);
    }
    for operator in final_operators.iter().filter(|o| !current_operators.contains(o)) {
        op(server, operator, client);
    }
    Ok(encode(server))
}
