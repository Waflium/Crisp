use std::collections::HashSet;

use chrono::Local;
use crisp_core::hash_map::java_hash_map_order;
use crisp_core::random::java_string_hash;
use serde_json::{Map, Value};

use super::banlist_service::{BAN_SOURCE, from_millis, instant_hash, to_millis};
use super::{ClientInfo, decode_params, log};
use crate::server::Server;
use crate::server::jsonrpc::api::PlayerDto;
use crate::server::jsonrpc::codec::{self, DecodeResult, Fields};
use crate::server::players::{Ban, IpBanEntry};
use crate::world::entity::EntityId;

pub fn is_inet_address(ip: &str) -> bool {
    if ip.contains(':') {
        let address = ip.split_once('%').map_or(ip, |(address, _)| address);
        return address.parse::<std::net::Ipv6Addr>().is_ok();
    }
    let parts: Vec<&str> = ip.split('.').collect();
    parts.len() == 4
        && parts.iter().all(|p| {
            !p.is_empty() && p.len() <= 3 && p.bytes().all(|b| b.is_ascii_digit()) && !(p.len() > 1 && p.starts_with('0')) && p.parse::<u32>().is_ok_and(|v| v <= 255)
        })
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct IpBan {
    ip: String,
    reason: Option<String>,
    source: String,
    expires: Option<i64>,
}

impl IpBan {
    fn from(entry: &IpBanEntry) -> IpBan {
        IpBan { ip: entry.ip.clone(), reason: entry.ban.reason.clone(), source: entry.ban.source.clone(), expires: entry.ban.expires.as_ref().map(to_millis) }
    }

    fn to_ip_ban_entry(&self) -> IpBanEntry {
        IpBanEntry {
            ip: self.ip.clone(),
            ban: Ban { created: Local::now().fixed_offset(), source: self.source.clone(), expires: self.expires.map(from_millis), reason: self.reason.clone() },
        }
    }

    fn java_hash(&self) -> i32 {
        let mut h = java_string_hash(&self.ip);
        h = h.wrapping_mul(31).wrapping_add(self.reason.as_deref().map_or(0, java_string_hash));
        h = h.wrapping_mul(31).wrapping_add(java_string_hash(&self.source));
        h.wrapping_mul(31).wrapping_add(instant_hash(self.expires))
    }
}

pub fn encode_entry(entry: &IpBanEntry) -> Value {
    let ban = IpBan::from(entry);
    let mut object = Map::new();
    object.insert("ip".into(), Value::String(ban.ip));
    if let Some(reason) = ban.reason {
        object.insert("reason".into(), Value::String(reason));
    }
    object.insert("source".into(), Value::String(ban.source));
    if let Some(expires) = ban.expires {
        object.insert("expires".into(), codec::encode_instant(expires));
    }
    Value::Object(object)
}

struct IncomingIpBanDto {
    player: Option<PlayerDto>,
    ip: Option<String>,
    reason: Option<String>,
    source: Option<String>,
    expires: Option<i64>,
}

impl IncomingIpBanDto {
    fn decode(value: &Value) -> DecodeResult<IncomingIpBanDto> {
        let mut fields = Fields::of(value)?;
        let player = fields.optional("player", PlayerDto::decode);
        let ip = fields.optional("ip", codec::string);
        let reason = fields.optional("reason", codec::string);
        let source = fields.optional("source", codec::string);
        let expires = fields.optional("expires", codec::instant);
        fields.finish(|| IncomingIpBanDto { player, ip, reason, source, expires })
    }

    fn to_ip_ban_for(&self, ip: String) -> IpBan {
        IpBan { ip, reason: self.reason.clone(), source: self.source.clone().unwrap_or_else(|| BAN_SOURCE.into()), expires: self.expires }
    }

    fn to_ip_ban(&self) -> Option<IpBan> {
        self.ip.as_ref().filter(|ip| is_inet_address(ip)).map(|ip| self.to_ip_ban_for(ip.clone()))
    }
}

struct IpBanDto {
    ip: String,
    reason: Option<String>,
    source: Option<String>,
    expires: Option<i64>,
}

impl IpBanDto {
    fn decode(value: &Value) -> DecodeResult<IpBanDto> {
        let mut fields = Fields::of(value)?;
        let ip = fields.required("ip", codec::string);
        let reason = fields.optional("reason", codec::string);
        let source = fields.optional("source", codec::string);
        let expires = fields.optional("expires", codec::instant);
        fields.finish(|| IpBanDto { ip: ip.unwrap(), reason, source, expires })
    }

    fn to_ip_ban(&self) -> IpBan {
        IpBan { ip: self.ip.clone(), reason: self.reason.clone(), source: self.source.clone().unwrap_or_else(|| BAN_SOURCE.into()), expires: self.expires }
    }
}

fn encode(server: &Server) -> Value {
    Value::Array(server.ip_bans.entries().into_iter().map(encode_entry).collect())
}

fn add_ip_ban(server: &mut Server, entry: IpBanEntry, client: ClientInfo) {
    log(client, format_args!("Add ip '{}' to ban list", entry.ip));
    server.ip_bans.add(entry);
}

fn remove_ip_ban(server: &mut Server, ip: &str, client: ClientInfo) {
    log(client, format_args!("Remove ip '{ip}' from ban list"));
    server.ip_bans.remove(ip);
}

fn ban_ip(server: &mut Server, ban: &IpBan, client: ClientInfo) -> Vec<EntityId> {
    add_ip_ban(server, ban.to_ip_ban_entry(), client);
    server.players_with_address(&ban.ip)
}

fn ban_incoming(server: &mut Server, ban: &IncomingIpBanDto, client: ClientInfo) -> Vec<EntityId> {
    if let Some(ip_ban) = ban.to_ip_ban() {
        return ban_ip(server, &ip_ban, client);
    }
    if let Some(player) = &ban.player
        && let Some(online) = server.find_player(player.id, player.name.as_deref())
    {
        let ip_ban = ban.to_ip_ban_for(server.level.player(online).ip_address());
        return ban_ip(server, &ip_ban, client);
    }
    Vec::new()
}

fn disconnect_ip_banned(server: &mut Server, players: Vec<EntityId>) {
    for player in players {
        server.level.player_mut(player).disconnect("multiplayer.disconnect.ip_banned");
    }
}

fn hash_set_order(bans: Vec<IpBan>) -> Vec<IpBan> {
    let mut unique: Vec<IpBan> = Vec::new();
    for ban in bans {
        if !unique.contains(&ban) {
            unique.push(ban);
        }
    }
    let hashes: Vec<i32> = unique.iter().map(IpBan::java_hash).collect();
    java_hash_map_order(&hashes).into_iter().map(|i| unique[i].clone()).collect()
}

pub fn get(server: &mut Server, _client: ClientInfo) -> Value {
    encode(server)
}

pub fn add(server: &mut Server, params: &Value, client: ClientInfo) -> Result<Value, String> {
    let bans = decode_params(params, |v| codec::list(v, IncomingIpBanDto::decode))?;
    let mut players = Vec::new();
    for ban in &bans {
        players.extend(ban_incoming(server, ban, client));
    }
    disconnect_ip_banned(server, players);
    Ok(encode(server))
}

pub fn clear(server: &mut Server, client: ClientInfo) -> Value {
    log(client, format_args!("Clear ip ban list"));
    server.ip_bans.clear();
    encode(server)
}

pub fn remove(server: &mut Server, params: &Value, client: ClientInfo) -> Result<Value, String> {
    let ips = decode_params(params, |v| codec::list(v, codec::string))?;
    for ip in &ips {
        remove_ip_ban(server, ip, client);
    }
    Ok(encode(server))
}

pub fn set(server: &mut Server, params: &Value, client: ClientInfo) -> Result<Value, String> {
    let ips = decode_params(params, |v| codec::list(v, IpBanDto::decode))?;
    let final_bans = hash_set_order(ips.iter().filter(|b| is_inet_address(&b.ip)).map(IpBanDto::to_ip_ban).collect());
    let current_bans = hash_set_order(server.ip_bans.entries().into_iter().map(IpBan::from).collect());
    let final_set: HashSet<&IpBan> = final_bans.iter().collect();
    let current_set: HashSet<&IpBan> = current_bans.iter().collect();
    for ban in current_bans.iter().filter(|b| !final_set.contains(b)) {
        remove_ip_ban(server, &ban.ip, client);
    }
    let added: Vec<&IpBan> = final_bans.iter().filter(|b| !current_set.contains(b)).collect();
    for ban in &added {
        add_ip_ban(server, ban.to_ip_ban_entry(), client);
    }
    let players = added.iter().flat_map(|ban| server.players_with_address(&ban.ip)).collect();
    disconnect_ip_banned(server, players);
    Ok(encode(server))
}
