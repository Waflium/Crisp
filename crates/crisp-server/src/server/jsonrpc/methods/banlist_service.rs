use std::collections::HashSet;

use chrono::{DateTime, FixedOffset, Local};
use crisp_core::hash_map::java_hash_map_order;
use crisp_core::random::java_string_hash;
use serde_json::{Map, Value};

use super::{ClientInfo, decode_params, get_user, log};
use crate::server::Server;
use crate::server::jsonrpc::api::PlayerDto;
use crate::server::jsonrpc::codec::{self, DecodeResult, Fields};
use crate::server::players::{Ban, NameAndId, UserBanEntry};

pub const BAN_SOURCE: &str = "Management server";

pub fn to_millis(date: &DateTime<FixedOffset>) -> i64 {
    date.timestamp_millis()
}

pub fn from_millis(millis: i64) -> DateTime<FixedOffset> {
    DateTime::from_timestamp_millis(millis).unwrap_or_default().with_timezone(&Local).fixed_offset()
}

pub fn instant_hash(millis: Option<i64>) -> i32 {
    let Some(millis) = millis else { return 0 };
    let seconds = millis.div_euclid(1000);
    let nanos = (millis.rem_euclid(1000) * 1_000_000) as i32;
    ((seconds ^ ((seconds as u64) >> 32) as i64) as i32).wrapping_add(nanos.wrapping_mul(51))
}

fn string_hash(value: Option<&str>) -> i32 {
    value.map_or(0, java_string_hash)
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct UserBan {
    player: NameAndId,
    reason: Option<String>,
    source: String,
    expires: Option<i64>,
}

impl UserBan {
    fn from(entry: &UserBanEntry) -> UserBan {
        UserBan { player: entry.user.clone(), reason: entry.ban.reason.clone(), source: entry.ban.source.clone(), expires: entry.ban.expires.as_ref().map(to_millis) }
    }

    fn to_ban_entry(&self) -> UserBanEntry {
        UserBanEntry {
            user: self.player.clone(),
            ban: Ban { created: Local::now().fixed_offset(), source: self.source.clone(), expires: self.expires.map(from_millis), reason: self.reason.clone() },
        }
    }

    fn java_hash(&self) -> i32 {
        let mut h = self.player.java_hash();
        h = h.wrapping_mul(31).wrapping_add(string_hash(self.reason.as_deref()));
        h = h.wrapping_mul(31).wrapping_add(java_string_hash(&self.source));
        h.wrapping_mul(31).wrapping_add(instant_hash(self.expires))
    }
}

pub struct UserBanDto {
    pub player: PlayerDto,
    pub reason: Option<String>,
    pub source: Option<String>,
    pub expires: Option<i64>,
}

impl UserBanDto {
    fn decode(value: &Value) -> DecodeResult<UserBanDto> {
        let mut fields = Fields::of(value)?;
        let player = fields.required("player", PlayerDto::decode);
        let reason = fields.optional("reason", codec::string);
        let source = fields.optional("source", codec::string);
        let expires = fields.optional("expires", codec::instant);
        fields.finish(|| UserBanDto { player: player.unwrap(), reason, source, expires })
    }

    pub fn from_entry(entry: &UserBanEntry) -> Value {
        let ban = UserBan::from(entry);
        UserBanDto { player: PlayerDto::from(&ban.player), reason: ban.reason, source: Some(ban.source), expires: ban.expires }.encode()
    }

    pub fn encode(&self) -> Value {
        let mut object = Map::new();
        object.insert("player".into(), self.player.encode());
        if let Some(reason) = &self.reason {
            object.insert("reason".into(), Value::String(reason.clone()));
        }
        if let Some(source) = &self.source {
            object.insert("source".into(), Value::String(source.clone()));
        }
        if let Some(expires) = self.expires {
            object.insert("expires".into(), codec::encode_instant(expires));
        }
        Value::Object(object)
    }

    fn to_user_ban(&self, player: NameAndId) -> UserBan {
        UserBan { player, reason: self.reason.clone(), source: self.source.clone().unwrap_or_else(|| BAN_SOURCE.into()), expires: self.expires }
    }
}

fn encode(server: &Server) -> Value {
    Value::Array(server.bans.entries().into_iter().map(UserBanDto::from_entry).collect())
}

pub fn add_user_ban(server: &mut Server, ban: UserBanEntry, client: ClientInfo) {
    log(client, format_args!("Add player 'literal{{{}}}' to banlist. Reason: '{}'", ban.user.name, ban.ban.reason_message().get_string()));
    server.bans.add(ban);
}

pub fn remove_user_ban(server: &mut Server, user: &NameAndId, client: ClientInfo) {
    log(client, format_args!("Remove player '{user}' from banlist"));
    server.bans.remove(user);
}

fn disconnect_banned(server: &mut Server, id: u128) {
    if let Some(player) = server.player_by_uuid(id) {
        server.level.player_mut(player).disconnect("multiplayer.disconnect.banned");
    }
}

fn bans(value: &Value) -> Result<Vec<UserBanDto>, String> {
    decode_params(value, |v| codec::list(v, UserBanDto::decode))
}

fn resolve(server: &Server, bans: &[UserBanDto]) -> Vec<UserBan> {
    bans.iter().filter_map(|b| get_user(server, b.player.id, b.player.name.as_deref()).map(|u| b.to_user_ban(u))).collect()
}

fn hash_set_order(bans: Vec<UserBan>) -> Vec<UserBan> {
    let mut unique: Vec<UserBan> = Vec::new();
    for ban in bans {
        if !unique.contains(&ban) {
            unique.push(ban);
        }
    }
    let hashes: Vec<i32> = unique.iter().map(UserBan::java_hash).collect();
    java_hash_map_order(&hashes).into_iter().map(|i| unique[i].clone()).collect()
}

pub fn get(server: &mut Server, _client: ClientInfo) -> Value {
    encode(server)
}

pub fn add(server: &mut Server, params: &Value, client: ClientInfo) -> Result<Value, String> {
    let bans = bans(params)?;
    for ban in resolve(server, &bans) {
        add_user_ban(server, ban.to_ban_entry(), client);
        disconnect_banned(server, ban.player.id);
    }
    Ok(encode(server))
}

pub fn clear(server: &mut Server, _client: ClientInfo) -> Value {
    server.bans.clear();
    encode(server)
}

pub fn remove(server: &mut Server, params: &Value, client: ClientInfo) -> Result<Value, String> {
    let players = decode_params(params, |v| codec::list(v, PlayerDto::decode))?;
    let users: Vec<NameAndId> = players.iter().filter_map(|p| get_user(server, p.id, p.name.as_deref())).collect();
    for user in users {
        remove_user_ban(server, &user, client);
    }
    Ok(encode(server))
}

pub fn set(server: &mut Server, params: &Value, client: ClientInfo) -> Result<Value, String> {
    let bans = bans(params)?;
    let final_bans = hash_set_order(resolve(server, &bans));
    let current_bans = hash_set_order(server.bans.entries().into_iter().map(UserBan::from).collect());
    let final_set: HashSet<&UserBan> = final_bans.iter().collect();
    let current_set: HashSet<&UserBan> = current_bans.iter().collect();
    for ban in current_bans.iter().filter(|b| !final_set.contains(b)) {
        remove_user_ban(server, &ban.player, client);
    }
    for ban in final_bans.iter().filter(|b| !current_set.contains(b)) {
        add_user_ban(server, ban.to_ban_entry(), client);
        disconnect_banned(server, ban.player.id);
    }
    Ok(encode(server))
}
