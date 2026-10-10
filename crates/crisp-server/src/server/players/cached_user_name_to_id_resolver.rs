use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use chrono::{DateTime, FixedOffset, Local, Months};
use serde_json::{Map, Value};

use super::NameAndId;
use crate::network::login::offline_uuid;
use crate::server::services::Services;

const GAMEPROFILES_MRU_LIMIT: usize = 1000;
const DATE_FORMAT: &str = "%Y-%m-%d %H:%M:%S %z";

#[derive(Clone)]
struct GameProfileInfo {
    name_and_id: NameAndId,
    expiration_date: DateTime<FixedOffset>,
    last_access: u64,
}

#[derive(Default)]
struct Profiles {
    by_name: HashMap<String, u128>,
    by_uuid: HashMap<u128, GameProfileInfo>,
    operation_count: u64,
}

impl Profiles {
    fn safe_add(&mut self, mut info: GameProfileInfo) {
        self.operation_count += 1;
        info.last_access = self.operation_count;
        self.by_name.insert(info.name_and_id.name.to_lowercase(), info.name_and_id.id);
        self.by_uuid.insert(info.name_and_id.id, info);
    }
}

pub struct CachedUserNameToIdResolver {
    file: Option<PathBuf>,
    profiles: Mutex<Profiles>,
    resolve_offline_users: bool,
}

impl CachedUserNameToIdResolver {
    pub fn load(dir: Option<PathBuf>) -> CachedUserNameToIdResolver {
        let file = dir.map(|d| d.join("usercache.json"));
        let mut profiles = Profiles::default();
        if let Some(file) = &file
            && let Ok(text) = std::fs::read_to_string(file)
        {
            match serde_json::from_str::<Value>(&text) {
                Ok(Value::Array(entries)) => entries.iter().rev().filter_map(read_game_profile).for_each(|info| profiles.safe_add(info)),
                Ok(_) => {}
                Err(e) => warn!("Failed to load profile cache {}: {e}", file.display()),
            }
        }
        CachedUserNameToIdResolver { file, profiles: Mutex::new(profiles), resolve_offline_users: true }
    }

    pub fn set_resolve_offline_users(&mut self, value: bool) {
        self.resolve_offline_users = value;
    }

    pub fn add(&self, name_and_id: NameAndId) {
        let mut profiles = self.profiles.lock().unwrap();
        add_internal(&mut profiles, name_and_id);
        self.write(&profiles);
    }

    pub fn save(&self) {
        self.write(&self.profiles.lock().unwrap());
    }

    pub fn get(&self, services: &Services, name: &str) -> Option<NameAndId> {
        let user_name = name.to_lowercase();
        let mut profiles = self.profiles.lock().unwrap();
        let mut needs_save = false;
        if let Some(&id) = profiles.by_name.get(&user_name)
            && Local::now().fixed_offset() >= profiles.by_uuid[&id].expiration_date
        {
            let info = profiles.by_uuid.remove(&id).unwrap();
            profiles.by_name.remove(&info.name_and_id.name.to_lowercase());
            needs_save = true;
        }
        let result = match profiles.by_name.get(&user_name).copied() {
            Some(id) => {
                profiles.operation_count += 1;
                let operation = profiles.operation_count;
                let info = profiles.by_uuid.get_mut(&id).unwrap();
                info.last_access = operation;
                Some(info.name_and_id.clone())
            }
            None => {
                let profile = self.lookup_game_profile(services, &user_name);
                if let Some(profile) = &profile {
                    add_internal(&mut profiles, profile.clone());
                    needs_save = true;
                }
                profile
            }
        };
        if needs_save {
            self.write(&profiles);
        }
        result
    }

    pub fn get_by_id(&self, id: u128) -> Option<NameAndId> {
        let mut profiles = self.profiles.lock().unwrap();
        profiles.operation_count += 1;
        let operation = profiles.operation_count;
        let info = profiles.by_uuid.get_mut(&id)?;
        info.last_access = operation;
        Some(info.name_and_id.clone())
    }

    fn lookup_game_profile(&self, services: &Services, name: &str) -> Option<NameAndId> {
        if !is_valid_player_name(name) {
            return self.create_unknown_profile(name);
        }
        services.find_profile_by_name(name).or_else(|| self.create_unknown_profile(name))
    }

    fn create_unknown_profile(&self, name: &str) -> Option<NameAndId> {
        self.resolve_offline_users.then(|| NameAndId { id: offline_uuid(name), name: name.to_string() })
    }

    fn write(&self, profiles: &Profiles) {
        let Some(file) = &self.file else { return };
        let mut infos: Vec<&GameProfileInfo> = profiles.by_uuid.values().collect();
        infos.sort_by_key(|info| std::cmp::Reverse(info.last_access));
        let entries: Vec<Value> = infos.into_iter().take(GAMEPROFILES_MRU_LIMIT).map(write_game_profile).collect();
        let _ = std::fs::write(file, serde_json::to_string(&entries).unwrap());
    }
}

fn add_internal(profiles: &mut Profiles, name_and_id: NameAndId) {
    let expiration_date = (Local::now() + Months::new(1)).fixed_offset();
    profiles.safe_add(GameProfileInfo { name_and_id, expiration_date, last_access: 0 });
}

fn write_game_profile(info: &GameProfileInfo) -> Value {
    let mut object = Map::new();
    info.name_and_id.append_to(&mut object);
    object.insert("expiresOn".into(), info.expiration_date.format(DATE_FORMAT).to_string().into());
    Value::Object(object)
}

fn read_game_profile(json: &Value) -> Option<GameProfileInfo> {
    let name_and_id = NameAndId::from_json(json)?;
    let date = json.get("expiresOn")?.as_str()?;
    match DateTime::parse_from_str(date, DATE_FORMAT) {
        Ok(expiration_date) => Some(GameProfileInfo { name_and_id, expiration_date, last_access: 0 }),
        Err(_) => {
            warn!("Failed to parse date {date}");
            None
        }
    }
}

pub fn is_valid_player_name(name: &str) -> bool {
    name.encode_utf16().count() <= 16 && !name.encode_utf16().any(|c| c <= 32 || c >= 127)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joined_players_are_remembered() {
        let dir = std::env::temp_dir().join(format!("crisp-usercache-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let services = Services::offline();
        let cache = CachedUserNameToIdResolver::load(Some(dir.clone()));
        cache.add(NameAndId { id: 7, name: "Rozqn".into() });
        assert_eq!(cache.get(&services, "ROZQN"), Some(NameAndId { id: 7, name: "Rozqn".into() }));
        assert_eq!(cache.get(&services, "Other"), Some(NameAndId { id: offline_uuid("other"), name: "other".into() }));
        let mut reloaded = CachedUserNameToIdResolver::load(Some(dir.clone()));
        assert_eq!(reloaded.get_by_id(7).map(|p| p.name), Some("Rozqn".into()));
        reloaded.set_resolve_offline_users(false);
        assert_eq!(reloaded.get(&services, "nobody"), None);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
