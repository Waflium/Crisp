use std::collections::HashSet;

use serde_json::Value;

use super::{ClientInfo, decode_params, get_user, log};
use crate::server::Server;
use crate::server::jsonrpc::api::PlayerDto;
use crate::server::jsonrpc::codec;
use crate::server::players::{NameAndId, WhiteListEntry, java_hash_set_order};

pub fn entries(server: &Server) -> Vec<NameAndId> {
    server.whitelist.entries().into_iter().map(|e| e.user.clone()).collect()
}

fn encode(server: &Server) -> Value {
    Value::Array(entries(server).iter().map(|u| PlayerDto::from(u).encode()).collect())
}

pub fn add_entry(server: &mut Server, user: NameAndId, client: ClientInfo) -> bool {
    log(client, format_args!("Add player '{user}' to allowlist"));
    server.whitelist.add(WhiteListEntry { user })
}

pub fn remove_entry(server: &mut Server, user: &NameAndId, client: ClientInfo) {
    log(client, format_args!("Remove player '{user}' from allowlist"));
    server.whitelist.remove(user);
}

pub fn kick_unlisted_players(server: &mut Server, client: ClientInfo) {
    log(client, format_args!("Kick unlisted players"));
    server.kick_unlisted_players();
}

fn players(value: &Value) -> Result<Vec<PlayerDto>, String> {
    decode_params(value, |v| codec::list(v, PlayerDto::decode))
}

fn resolve(server: &Server, players: &[PlayerDto]) -> Vec<Option<NameAndId>> {
    players.iter().map(|p| get_user(server, p.id, p.name.as_deref())).collect()
}

pub fn get(server: &mut Server, _client: ClientInfo) -> Value {
    encode(server)
}

pub fn add(server: &mut Server, params: &Value, client: ClientInfo) -> Result<Value, String> {
    let players = players(params)?;
    for user in resolve(server, &players).into_iter().flatten() {
        add_entry(server, user, client);
    }
    Ok(encode(server))
}

pub fn clear(server: &mut Server, client: ClientInfo) -> Value {
    log(client, format_args!("Clear allowlist"));
    server.whitelist.clear();
    encode(server)
}

pub fn remove(server: &mut Server, params: &Value, client: ClientInfo) -> Result<Value, String> {
    let players = players(params)?;
    for user in resolve(server, &players).into_iter().flatten() {
        remove_entry(server, &user, client);
    }
    kick_unlisted_players(server, client);
    Ok(encode(server))
}

pub fn set(server: &mut Server, params: &Value, client: ClientInfo) -> Result<Value, String> {
    let players = players(params)?;
    let final_allow_list = java_hash_set_order(resolve(server, &players).into_iter().flatten().collect());
    let current_allow_list = java_hash_set_order(entries(server));
    let final_set: HashSet<&NameAndId> = final_allow_list.iter().collect();
    let current_set: HashSet<&NameAndId> = current_allow_list.iter().collect();
    for user in current_allow_list.iter().filter(|u| !final_set.contains(u)) {
        remove_entry(server, user, client);
    }
    for user in final_allow_list.iter().filter(|u| !current_set.contains(u)) {
        add_entry(server, user.clone(), client);
    }
    kick_unlisted_players(server, client);
    Ok(encode(server))
}
