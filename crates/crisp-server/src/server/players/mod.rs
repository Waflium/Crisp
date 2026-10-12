use std::fmt;
use std::path::PathBuf;

use chrono::{DateTime, FixedOffset, Local};
use crisp_core::hash_map::java_hash_map_order;
use crisp_core::random::java_string_hash;
use serde_json::{Map, Value};

use super::notifications::NotificationManager;
use crate::commands::arguments::java_uuid_from_string;
use crate::network::chat::{Arg, Component};
use crate::world::level::storage::format_uuid;

pub mod cached_user_name_to_id_resolver;
pub mod player_list;
pub mod respawn;
pub mod sleep_status;

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct NameAndId {
    pub id: u128,
    pub name: String,
}

impl NameAndId {
    pub fn from_json(object: &Value) -> Option<NameAndId> {
        Some(NameAndId { id: java_uuid_from_string(object["uuid"].as_str()?)?, name: object["name"].as_str()?.to_string() })
    }

    pub fn append_to(&self, object: &mut Map<String, Value>) {
        object.insert("uuid".into(), format_uuid(self.id).into());
        object.insert("name".into(), self.name.clone().into());
    }

    pub fn java_hash(&self) -> i32 {
        let hilo = (self.id >> 64) as u64 ^ self.id as u64;
        let uuid_hash = ((hilo >> 32) as i32) ^ (hilo as i32);
        uuid_hash.wrapping_mul(31).wrapping_add(java_string_hash(&self.name))
    }
}

impl fmt::Display for NameAndId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "NameAndId[id={}, name={}]", format_uuid(self.id), self.name)
    }
}

pub fn java_hash_set_order(users: Vec<NameAndId>) -> Vec<NameAndId> {
    let mut unique: Vec<NameAndId> = Vec::new();
    for user in users {
        if !unique.contains(&user) {
            unique.push(user);
        }
    }
    let hashes: Vec<i32> = unique.iter().map(NameAndId::java_hash).collect();
    java_hash_map_order(&hashes).into_iter().map(|i| unique[i].clone()).collect()
}

pub trait StoredUserEntry: Clone {
    fn key(&self) -> String;
    fn from_json(object: &Value) -> Option<Self>;
    fn serialize(&self, object: &mut Map<String, Value>);
    fn has_expired(&self) -> bool {
        false
    }
    fn same_as(&self, _other: &Self) -> bool {
        false
    }
    fn notify_added(&self, _notifications: &NotificationManager) {}
    fn notify_removed(&self, _notifications: &NotificationManager) {}
}

pub struct StoredUserList<E> {
    file: Option<PathBuf>,
    entries: Vec<E>,
    notifications: NotificationManager,
}

impl<E: StoredUserEntry> StoredUserList<E> {
    pub fn new(file: Option<PathBuf>) -> StoredUserList<E> {
        StoredUserList { file, entries: Vec::new(), notifications: NotificationManager::default() }
    }

    pub fn set_notification_manager(&mut self, notifications: NotificationManager) {
        self.notifications = notifications;
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.entries.iter().any(|e| e.key() == key)
    }

    pub fn get_key(&mut self, key: &str) -> Option<&E> {
        self.remove_expired();
        self.entries.iter().find(|e| e.key() == key)
    }

    pub fn add(&mut self, entry: E) -> bool {
        let key = entry.key();
        match self.entries.iter_mut().find(|e| e.key() == key) {
            Some(existing) if existing.same_as(&entry) => return false,
            Some(existing) => *existing = entry.clone(),
            None => self.entries.push(entry.clone()),
        }
        self.save_logged("adding");
        entry.notify_added(&self.notifications);
        true
    }

    fn remove_key(&mut self, key: &str) -> Option<E> {
        let index = self.entries.iter().position(|e| e.key() == key)?;
        let removed = self.entries.remove(index);
        self.save_logged("removing");
        Some(removed)
    }

    pub fn clear(&mut self) {
        for entry in self.entries() {
            entry.notify_removed(&self.notifications);
        }
        self.entries.clear();
        self.save_logged("removing");
    }

    pub fn entries(&self) -> Vec<&E> {
        let keys: Vec<String> = self.entries.iter().map(E::key).collect();
        crisp_protocol::nbt::java_hash_map_order(&keys).into_iter().map(|i| &self.entries[i]).collect()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn remove_expired(&mut self) {
        if self.entries.iter().any(E::has_expired) {
            self.entries.retain(|e| !e.has_expired());
        }
    }

    fn save_logged(&self, action: &str) {
        if let Err(e) = self.save() {
            warn!("Could not save the list after {action} a user: {e}");
        }
    }

    pub fn save(&self) -> std::io::Result<()> {
        let Some(file) = &self.file else { return Ok(()) };
        let array: Vec<Value> = self
            .entries()
            .into_iter()
            .map(|e| {
                let mut object = Map::new();
                e.serialize(&mut object);
                Value::Object(object)
            })
            .collect();
        std::fs::write(file, serde_json::to_string_pretty(&Value::Array(array))?)
    }

    pub fn load(&mut self) -> std::io::Result<()> {
        let Some(file) = &self.file else { return Ok(()) };
        if !file.exists() {
            return Ok(());
        }
        let Ok(Value::Array(array)) = serde_json::from_str::<Value>(&std::fs::read_to_string(file)?) else {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, format!("{} is not a JSON array", file.display())));
        };
        self.entries.clear();
        for object in &array {
            let Some(entry) = E::from_json(object) else { continue };
            let key = entry.key();
            match self.entries.iter_mut().find(|e| e.key() == key) {
                Some(existing) => *existing = entry,
                None => self.entries.push(entry),
            }
        }
        Ok(())
    }

    fn load_logged(mut self, what: &str) -> StoredUserList<E> {
        if self.load().is_err() {
            warn!("Failed to load {what}: ");
        }
        self
    }

    fn save_list_logged(&self, what: &str) {
        if let (Err(e), Some(file)) = (self.save(), &self.file) {
            warn!("Failed to save {what}: \n{}", crate::server::dedicated::settings::java_io_exception(&e, file));
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct OpEntry {
    pub user: NameAndId,
    pub level: i32,
    pub bypasses_player_limit: bool,
}

impl StoredUserEntry for OpEntry {
    fn key(&self) -> String {
        format_uuid(self.user.id)
    }

    fn from_json(object: &Value) -> Option<OpEntry> {
        Some(OpEntry {
            user: NameAndId::from_json(object)?,
            level: object["level"].as_i64().map_or(0, |l| l.clamp(0, 4) as i32),
            bypasses_player_limit: object["bypassesPlayerLimit"].as_bool().unwrap_or(false),
        })
    }

    fn serialize(&self, object: &mut Map<String, Value>) {
        self.user.append_to(object);
        object.insert("level".into(), self.level.into());
        object.insert("bypassesPlayerLimit".into(), self.bypasses_player_limit.into());
    }

    fn notify_added(&self, notifications: &NotificationManager) {
        notifications.player_oped(self);
    }

    fn notify_removed(&self, notifications: &NotificationManager) {
        notifications.player_deoped(self);
    }
}

pub type ServerOpList = StoredUserList<OpEntry>;

impl ServerOpList {
    pub fn get(&self, uuid: u128) -> Option<&OpEntry> {
        self.entries.iter().find(|e| e.user.id == uuid)
    }

    pub fn contains(&self, uuid: u128) -> bool {
        self.contains_key(&format_uuid(uuid))
    }

    pub fn can_bypass_player_limit(&self, uuid: u128) -> bool {
        self.get(uuid).is_some_and(|e| e.bypasses_player_limit)
    }

    pub fn remove(&mut self, user: &NameAndId) -> bool {
        let Some(entry) = self.remove_key(&format_uuid(user.id)) else { return false };
        self.notifications.player_deoped(&entry);
        true
    }
}

impl StoredUserList<WhiteListEntry> {
    pub fn remove(&mut self, user: &NameAndId) -> bool {
        if self.remove_key(&format_uuid(user.id)).is_none() {
            return false;
        }
        self.notifications.player_removed_from_allowlist(user);
        true
    }
}

impl StoredUserList<UserBanEntry> {
    pub fn remove(&mut self, user: &NameAndId) -> bool {
        if self.remove_key(&format_uuid(user.id)).is_none() {
            return false;
        }
        self.notifications.player_unbanned(user);
        true
    }
}

impl StoredUserList<IpBanEntry> {
    pub fn remove(&mut self, ip: &str) -> bool {
        if self.remove_key(ip).is_none() {
            return false;
        }
        self.notifications.ip_unbanned(ip);
        true
    }
}

const DATE_FORMAT: &str = "%Y-%m-%d %H:%M:%S %z";

fn format_date(date: &DateTime<FixedOffset>) -> String {
    date.with_timezone(&Local).format(DATE_FORMAT).to_string()
}

#[derive(Clone, Debug)]
pub struct Ban {
    pub created: DateTime<FixedOffset>,
    pub source: String,
    pub expires: Option<DateTime<FixedOffset>>,
    pub reason: Option<String>,
}

impl PartialEq for Ban {
    fn eq(&self, other: &Ban) -> bool {
        self.source == other.source && self.expires == other.expires && self.reason == other.reason
    }
}

impl Ban {
    pub fn new(source: Option<String>, expires: Option<DateTime<FixedOffset>>, reason: Option<String>) -> Ban {
        Ban { created: Local::now().fixed_offset(), source: source.unwrap_or_else(|| "(Unknown)".into()), expires, reason }
    }

    fn from_json(object: &Value) -> Ban {
        let date = |key: &str| object[key].as_str().and_then(|s| DateTime::parse_from_str(s, DATE_FORMAT).ok());
        Ban {
            created: date("created").unwrap_or_else(|| Local::now().fixed_offset()),
            source: object["source"].as_str().unwrap_or("(Unknown)").to_string(),
            expires: date("expires"),
            reason: object["reason"].as_str().map(str::to_string),
        }
    }

    fn serialize(&self, object: &mut Map<String, Value>) {
        object.insert("created".into(), format_date(&self.created).into());
        object.insert("source".into(), self.source.clone().into());
        object.insert("expires".into(), self.expires.as_ref().map_or_else(|| "forever".to_string(), format_date).into());
        if let Some(reason) = &self.reason {
            object.insert("reason".into(), reason.clone().into());
        }
    }

    pub fn reason_message(&self) -> Component {
        match &self.reason {
            Some(reason) => Component::literal(reason.as_str()),
            None => Component::translatable("multiplayer.disconnect.banned.reason.default"),
        }
    }

    fn has_expired(&self) -> bool {
        self.expires.is_some_and(|e| e < Local::now())
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct UserBanEntry {
    pub user: NameAndId,
    pub ban: Ban,
}

impl StoredUserEntry for UserBanEntry {
    fn key(&self) -> String {
        format_uuid(self.user.id)
    }

    fn from_json(object: &Value) -> Option<UserBanEntry> {
        Some(UserBanEntry { user: NameAndId::from_json(object)?, ban: Ban::from_json(object) })
    }

    fn serialize(&self, object: &mut Map<String, Value>) {
        self.user.append_to(object);
        self.ban.serialize(object);
    }

    fn has_expired(&self) -> bool {
        self.ban.has_expired()
    }

    fn same_as(&self, other: &Self) -> bool {
        self == other
    }

    fn notify_added(&self, notifications: &NotificationManager) {
        notifications.player_banned(self);
    }

    fn notify_removed(&self, notifications: &NotificationManager) {
        notifications.player_unbanned(&self.user);
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct IpBanEntry {
    pub ip: String,
    pub ban: Ban,
}

impl StoredUserEntry for IpBanEntry {
    fn key(&self) -> String {
        self.ip.clone()
    }

    fn from_json(object: &Value) -> Option<IpBanEntry> {
        Some(IpBanEntry { ip: object["ip"].as_str()?.to_string(), ban: Ban::from_json(object) })
    }

    fn serialize(&self, object: &mut Map<String, Value>) {
        object.insert("ip".into(), self.ip.clone().into());
        self.ban.serialize(object);
    }

    fn has_expired(&self) -> bool {
        self.ban.has_expired()
    }

    fn same_as(&self, other: &Self) -> bool {
        self == other
    }

    fn notify_added(&self, notifications: &NotificationManager) {
        notifications.ip_banned(self);
    }

    fn notify_removed(&self, notifications: &NotificationManager) {
        notifications.ip_unbanned(&self.ip);
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct WhiteListEntry {
    pub user: NameAndId,
}

impl StoredUserEntry for WhiteListEntry {
    fn key(&self) -> String {
        format_uuid(self.user.id)
    }

    fn from_json(object: &Value) -> Option<WhiteListEntry> {
        Some(WhiteListEntry { user: NameAndId::from_json(object)? })
    }

    fn serialize(&self, object: &mut Map<String, Value>) {
        self.user.append_to(object);
    }

    fn notify_added(&self, notifications: &NotificationManager) {
        notifications.player_added_to_allowlist(&self.user);
    }

    fn notify_removed(&self, notifications: &NotificationManager) {
        notifications.player_removed_from_allowlist(&self.user);
    }
}

pub fn ip_from_address(address: &std::net::SocketAddr) -> String {
    let text = match address {
        std::net::SocketAddr::V4(a) => format!("/{}:{}", a.ip(), a.port()),
        std::net::SocketAddr::V6(a) => {
            let groups: Vec<String> = a.ip().segments().iter().map(|s| format!("{s:x}")).collect();
            format!("/[{}]:{}", groups.join(":"), a.port())
        }
    };
    let mut ip = &text[text.find('/').map_or(0, |i| i + 1)..];
    if let Some(colon) = ip.find(':') {
        ip = &ip[..colon];
    }
    ip.to_string()
}

pub struct PlayerLists {
    pub ops: ServerOpList,
    pub bans: StoredUserList<UserBanEntry>,
    pub ip_bans: StoredUserList<IpBanEntry>,
    pub whitelist: StoredUserList<WhiteListEntry>,
}

impl PlayerLists {
    pub fn load(dir: Option<PathBuf>) -> PlayerLists {
        let file = |name: &str| dir.as_ref().map(|d| d.join(name));
        let bans = StoredUserList::new(file("banned-players.json")).load_logged("user banlist");
        bans.save_list_logged("user banlist");
        let ip_bans = StoredUserList::new(file("banned-ips.json")).load_logged("ip banlist");
        ip_bans.save_list_logged("ip banlist");
        let ops = ServerOpList::new(file("ops.json")).load_logged("operators list");
        let whitelist = StoredUserList::new(file("whitelist.json")).load_logged("white-list");
        ops.save_list_logged("operators list");
        if whitelist.file.as_ref().is_some_and(|f| !f.exists()) {
            whitelist.save_list_logged("white-list");
        }
        PlayerLists { ops, bans, ip_bans, whitelist }
    }
}

pub fn ban_date(date: &DateTime<FixedOffset>) -> String {
    let local = date.with_timezone(&Local);
    let offset = local.offset().local_minus_utc();
    let zone = if offset == 0 {
        "GMT".to_string()
    } else {
        let sign = if offset < 0 { '-' } else { '+' };
        format!("GMT{sign}{:02}:{:02}", offset.abs() / 3600, offset.abs() % 3600 / 60)
    };
    format!("{} {zone}", local.format("%Y-%m-%d at %H:%M:%S"))
}

pub fn ban_message(key: &str, expiration_key: &str, ban: &Ban) -> Component {
    let mut reason = Component::translatable_with(key, vec![Arg::Component(ban.reason_message())]);
    if let Some(expires) = &ban.expires {
        reason = reason.append(Component::translatable_with(expiration_key, vec![Arg::String(ban_date(expires))]));
    }
    reason
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn ip_strings() {
        assert_eq!(ip_from_address(&"127.0.0.1:25565".parse().unwrap()), "127.0.0.1");
        assert_eq!(ip_from_address(&"[::1]:25565".parse().unwrap()), "[0");
    }

    #[test]
    fn ban_round_trip() {
        let entry = UserBanEntry { user: NameAndId { id: 5, name: "Bob".into() }, ban: Ban::new(None, None, None) };
        let mut object = Map::new();
        entry.serialize(&mut object);
        assert_eq!(object["expires"], "forever");
        assert_eq!(object["source"], "(Unknown)");
        assert!(object.get("reason").is_none());
        let read = UserBanEntry::from_json(&Value::Object(object)).unwrap();
        assert_eq!(read, entry);
        assert_eq!(json!(read.key()), json!("00000000-0000-0000-0000-000000000005"));
    }
}
