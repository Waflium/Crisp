use std::collections::HashMap;
use std::hash::Hash;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use super::cached_user_name_to_id_resolver::{CachedUserNameToIdResolver, is_valid_player_name};
use crate::network::login::Profile;
use crate::server::services::Services;

const EXPIRE_AFTER_ACCESS: Duration = Duration::from_secs(10 * 60);
const MAXIMUM_SIZE: usize = 256;

struct LoadingCache<K, V> {
    entries: Mutex<HashMap<K, (V, Instant)>>,
}

impl<K: Eq + Hash + Clone, V: Clone> LoadingCache<K, V> {
    fn new() -> LoadingCache<K, V> {
        LoadingCache { entries: Mutex::new(HashMap::new()) }
    }

    fn get_unchecked(&self, key: &K, load: impl FnOnce() -> V) -> V {
        let now = Instant::now();
        let mut entries = self.entries.lock().unwrap();
        entries.retain(|_, (_, accessed)| now.duration_since(*accessed) < EXPIRE_AFTER_ACCESS);
        if let Some((value, accessed)) = entries.get_mut(key) {
            *accessed = now;
            return value.clone();
        }
        drop(entries);
        let value = load();
        let mut entries = self.entries.lock().unwrap();
        if entries.len() >= MAXIMUM_SIZE
            && let Some(oldest) = entries.iter().min_by_key(|(_, (_, accessed))| *accessed).map(|(k, _)| k.clone())
        {
            entries.remove(&oldest);
        }
        entries.insert(key.clone(), (value.clone(), now));
        value
    }
}

pub struct ProfileResolver {
    profile_cache_by_name: LoadingCache<String, Option<Profile>>,
    profile_cache_by_id: LoadingCache<u128, Option<Profile>>,
}

impl Default for ProfileResolver {
    fn default() -> ProfileResolver {
        ProfileResolver { profile_cache_by_name: LoadingCache::new(), profile_cache_by_id: LoadingCache::new() }
    }
}

impl ProfileResolver {
    pub fn fetch_by_name(&self, services: &Services, name_to_id_cache: &CachedUserNameToIdResolver, name: &str) -> Option<Profile> {
        if !is_valid_player_name(name) {
            return None;
        }
        self.profile_cache_by_name.get_unchecked(&name.to_string(), || {
            name_to_id_cache.get(services, name).and_then(|name_and_id| self.fetch_by_id(services, name_and_id.id))
        })
    }

    pub fn fetch_by_id(&self, services: &Services, id: u128) -> Option<Profile> {
        self.profile_cache_by_id.get_unchecked(&id, || services.fetch_game_profile(id))
    }
}
