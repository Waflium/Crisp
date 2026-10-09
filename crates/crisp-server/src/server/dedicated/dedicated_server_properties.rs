use super::settings::{Properties, Settings};
use crate::server::level::game_mode::GameType;
use crate::world::difficulty::Difficulty;

const SECRET_ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";

#[derive(Clone, Debug, PartialEq)]
pub struct ServerResourcePackInfo {
    pub id: u128,
    pub url: String,
    pub hash: String,
    pub is_required: bool,
    pub prompt: Option<crate::network::chat::Component>,
}

#[derive(Clone, Debug)]
pub struct DedicatedServerProperties {
    pub properties: Properties,
    pub online_mode: bool,
    pub prevent_proxy_connections: bool,
    pub server_ip: String,
    pub allow_flight: bool,
    pub motd: String,
    pub code_of_conduct: bool,
    pub bug_report_link: String,
    pub force_game_mode: bool,
    pub enforce_whitelist: bool,
    pub difficulty: Difficulty,
    pub game_mode: GameType,
    pub level_name: String,
    pub server_port: i32,
    pub management_server_enabled: bool,
    pub management_server_host: String,
    pub management_server_port: i32,
    pub management_server_secret: String,
    pub management_server_tls_enabled: bool,
    pub management_server_tls_keystore: String,
    pub management_server_tls_keystore_password: String,
    pub management_server_allowed_origins: String,
    pub announce_player_achievements: Option<bool>,
    pub enable_query: bool,
    pub query_port: i32,
    pub enable_rcon: bool,
    pub rcon_port: i32,
    pub rcon_password: String,
    pub hardcore: bool,
    pub use_native_transport: bool,
    pub spawn_protection: i32,
    pub op_permission_level: i32,
    pub function_permission_level: i32,
    pub max_tick_time: i64,
    pub max_chained_neighbor_updates: i32,
    pub rate_limit_packets_per_second: i32,
    pub command_spam_threshold_seconds: i32,
    pub chat_spam_threshold_seconds: i32,
    pub view_distance: i32,
    pub simulation_distance: i32,
    pub max_players: i32,
    pub network_compression_threshold: i32,
    pub broadcast_rcon_to_ops: bool,
    pub broadcast_console_to_ops: bool,
    pub max_world_size: i32,
    pub sync_chunk_writes: bool,
    pub region_file_compression: String,
    pub enable_jmx_monitoring: bool,
    pub enable_status: bool,
    pub hide_online_players: bool,
    pub entity_broadcast_range_percentage: i32,
    pub text_filtering_config: String,
    pub text_filtering_version: i32,
    pub server_resource_pack_info: Option<ServerResourcePackInfo>,
    pub initial_enabled_packs: Vec<String>,
    pub initial_disabled_packs: Vec<String>,
    pub player_idle_timeout: i32,
    pub status_heartbeat_interval: i32,
    pub white_list: bool,
    pub enforce_secure_profile: bool,
    pub log_ips: bool,
    pub pause_when_empty_seconds: i32,
    pub seed: Option<i64>,
    pub generate_structures: bool,
    pub generator_settings: String,
    pub level_type: String,
    pub accepts_transfers: bool,
}

#[derive(Debug)]
pub struct InvalidProperty(pub String);

impl DedicatedServerProperties {
    pub fn new(properties: Properties) -> Result<DedicatedServerProperties, InvalidProperty> {
        let mut s = Settings { properties };
        let difficulty = |v: &str| match v.parse::<i32>() {
            Ok(id) => Some(Difficulty::VALUES[id.rem_euclid(4) as usize]),
            Err(_) => Difficulty::by_name(v),
        };
        let game_type = |v: &str| match v.parse::<i32>() {
            Ok(id) => Some(GameType::by_id(id)),
            Err(_) => GameType::by_name(v),
        };
        let permission = |v: &str| v.parse::<i32>().ok().map(|level| level.clamp(0, 4));
        let online_mode = s.get_bool("online-mode", true);
        let prevent_proxy_connections = s.get_bool("prevent-proxy-connections", false);
        let server_ip = s.get_string("server-ip", "");
        let allow_flight = s.get_bool("allow-flight", false);
        let motd = s.get_string("motd", "A Minecraft Server");
        let code_of_conduct = s.get_bool("enable-code-of-conduct", false);
        let bug_report_link = s.get_string("bug-report-link", "");
        let force_game_mode = s.get_bool("force-gamemode", false);
        let enforce_whitelist = s.get_bool("enforce-whitelist", false);
        let difficulty = s.get("difficulty", difficulty, |d| d.key().to_string(), Difficulty::Easy);
        let game_mode = s.get("gamemode", game_type, |g| g.name().to_string(), GameType::Survival);
        let level_name = s.get_string("level-name", "world");
        let server_port = s.get_int("server-port", 25565);
        let management_server_enabled = s.get_bool("management-server-enabled", false);
        let management_server_host = s.get_string("management-server-host", "localhost");
        let management_server_port = s.get_int("management-server-port", 0);
        let management_server_secret = s.get_string("management-server-secret", &generate_secret_key());
        let management_server_tls_enabled = s.get_bool("management-server-tls-enabled", true);
        let management_server_tls_keystore = s.get_string("management-server-tls-keystore", "");
        let management_server_tls_keystore_password = s.get_string("management-server-tls-keystore-password", "");
        let management_server_allowed_origins = s.get_string("management-server-allowed-origins", "");
        let announce_player_achievements = s.get_legacy("announce-player-achievements").map(|v| v.eq_ignore_ascii_case("true"));
        let enable_query = s.get_bool("enable-query", false);
        let query_port = s.get_int("query.port", 25565);
        let enable_rcon = s.get_bool("enable-rcon", false);
        let rcon_port = s.get_int("rcon.port", 25575);
        let rcon_password = s.get_string("rcon.password", "");
        let hardcore = s.get_bool("hardcore", false);
        let use_native_transport = s.get_bool("use-native-transport", true);
        let spawn_protection = s.get_int("spawn-protection", 16);
        let op_permission_level = s.get("op-permission-level", permission, i32::to_string, 4);
        let function_permission_level = s.get("function-permission-level", permission, i32::to_string, 2);
        let max_tick_time = s.get_long("max-tick-time", 60_000);
        let max_chained_neighbor_updates = s.get_int("max-chained-neighbor-updates", 1_000_000);
        let rate_limit_packets_per_second = s.get_int("rate-limit", 0);
        let command_spam_threshold_seconds = s.get_int("command-spam-threshold-seconds", 10);
        let chat_spam_threshold_seconds = s.get_int("chat-spam-threshold-seconds", 10);
        let view_distance = s.get_int("view-distance", 10);
        let simulation_distance = s.get_int("simulation-distance", 10);
        let max_players = s.get_int("max-players", 20);
        let network_compression_threshold = s.get_int("network-compression-threshold", 256);
        let broadcast_rcon_to_ops = s.get_bool("broadcast-rcon-to-ops", true);
        let broadcast_console_to_ops = s.get_bool("broadcast-console-to-ops", true);
        let max_world_size = s.get("max-world-size", |v| v.parse::<i32>().ok().map(|v| v.clamp(1, 29_999_984)), i32::to_string, 29_999_984);
        let sync_chunk_writes = s.get_bool("sync-chunk-writes", true);
        let region_file_compression = s.get_string("region-file-compression", "deflate");
        let enable_jmx_monitoring = s.get_bool("enable-jmx-monitoring", false);
        let enable_status = s.get_bool("enable-status", true);
        let hide_online_players = s.get_bool("hide-online-players", false);
        let entity_broadcast_range_percentage = match s.properties.get("entity-broadcast-range-percentage") {
            Some(raw) => raw.parse::<i32>().map_err(|_| InvalidProperty(format!("java.lang.NumberFormatException: For input string: \"{raw}\"")))?.clamp(10, 1000),
            None => 100,
        };
        s.properties.put("entity-broadcast-range-percentage", entity_broadcast_range_percentage.to_string());
        let text_filtering_config = s.get_string("text-filtering-config", "");
        let text_filtering_version = s.get_int("text-filtering-version", 0);
        let player_idle_timeout = s.get_int("player-idle-timeout", 0);
        let status_heartbeat_interval = s.get_int("status-heartbeat-interval", 0);
        let white_list = s.get_bool("white-list", true);
        let enforce_secure_profile = s.get_bool("enforce-secure-profile", true);
        let log_ips = s.get_bool("log-ips", true);
        let pause_when_empty_seconds = s.get_int("pause-when-empty-seconds", 60);
        let accepts_transfers = s.get_bool("accepts-transfers", false);
        let level_seed = s.get_string("level-seed", "");
        let generate_structures = s.get_bool("generate-structures", true);
        let generator_settings = s.get(
            "generator-settings",
            |v| Some(compact_json(if v.is_empty() { "{}" } else { v })),
            |v: &Result<String, String>| v.clone().unwrap_or_default(),
            Ok("{}".to_string()),
        );
        let generator_settings = generator_settings.map_err(InvalidProperty)?;
        let level_type = s.get("level-type", |v| Some(v.to_lowercase()), String::clone, "minecraft:normal".to_string());
        let resource_pack_id = s.get_string("resource-pack-id", "");
        let resource_pack = s.get_string("resource-pack", "");
        let resource_pack_sha1 = s.get_string("resource-pack-sha1", "");
        let resource_pack_hash = s.get_legacy("resource-pack-hash");
        let require_resource_pack = s.get_bool("require-resource-pack", false);
        let resource_pack_prompt = s.get_string("resource-pack-prompt", "");
        let server_resource_pack_info =
            server_pack_info(&resource_pack_id, &resource_pack, &resource_pack_sha1, resource_pack_hash.as_deref(), require_resource_pack, &resource_pack_prompt);
        let initial_enabled_packs = split_packs(&s.get_string("initial-enabled-packs", "vanilla"));
        let initial_disabled_packs = split_packs(&s.get_string("initial-disabled-packs", ""));
        Ok(DedicatedServerProperties {
            properties: s.properties,
            online_mode,
            prevent_proxy_connections,
            server_ip,
            allow_flight,
            motd,
            code_of_conduct,
            bug_report_link,
            force_game_mode,
            enforce_whitelist,
            difficulty,
            game_mode,
            level_name,
            server_port,
            management_server_enabled,
            management_server_host,
            management_server_port,
            management_server_secret,
            management_server_tls_enabled,
            management_server_tls_keystore,
            management_server_tls_keystore_password,
            management_server_allowed_origins,
            announce_player_achievements,
            enable_query,
            query_port,
            enable_rcon,
            rcon_port,
            rcon_password,
            hardcore,
            use_native_transport,
            spawn_protection,
            op_permission_level,
            function_permission_level,
            max_tick_time,
            max_chained_neighbor_updates,
            rate_limit_packets_per_second,
            command_spam_threshold_seconds,
            chat_spam_threshold_seconds,
            view_distance,
            simulation_distance,
            max_players,
            network_compression_threshold,
            broadcast_rcon_to_ops,
            broadcast_console_to_ops,
            max_world_size,
            sync_chunk_writes,
            region_file_compression,
            enable_jmx_monitoring,
            enable_status,
            hide_online_players,
            entity_broadcast_range_percentage,
            text_filtering_config,
            text_filtering_version,
            server_resource_pack_info,
            initial_enabled_packs,
            initial_disabled_packs,
            player_idle_timeout,
            status_heartbeat_interval,
            white_list,
            enforce_secure_profile,
            log_ips,
            pause_when_empty_seconds,
            seed: parse_seed(&level_seed),
            generate_structures,
            generator_settings,
            level_type,
            accepts_transfers,
        })
    }

    pub fn with(&self, key: &str, value: String) -> Result<DedicatedServerProperties, InvalidProperty> {
        let mut properties = self.properties.clone();
        properties.put(key, value);
        DedicatedServerProperties::new(properties)
    }
}

fn generate_secret_key() -> String {
    (0..40).map(|_| SECRET_ALPHABET[rand::random_range(0..SECRET_ALPHABET.len())] as char).collect()
}

fn split_packs(packs: &str) -> Vec<String> {
    packs.split(',').map(|p| p.trim().to_string()).collect()
}

pub fn parse_seed(seed: &str) -> Option<i64> {
    let seed = seed.trim();
    if seed.is_empty() {
        return None;
    }
    Some(seed.parse::<i64>().unwrap_or_else(|_| java_string_hash(seed) as i64))
}

fn java_string_hash(s: &str) -> i32 {
    s.encode_utf16().fold(0i32, |h, c| h.wrapping_mul(31).wrapping_add(c as i32))
}

fn compact_json(raw: &str) -> Result<String, String> {
    serde_json::from_str::<serde_json::Value>(raw).map_err(|e| format!("com.google.gson.JsonSyntaxException: {e}"))?;
    let mut out = String::with_capacity(raw.len());
    let mut in_string = false;
    let mut escaped = false;
    for c in raw.chars() {
        if in_string {
            out.push(c);
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
            }
        } else if c == '"' {
            in_string = true;
            out.push(c);
        } else if !c.is_whitespace() {
            out.push(c);
        }
    }
    Ok(out)
}

fn server_pack_info(id: &str, url: &str, sha1: &str, legacy_hash: Option<&str>, required: bool, prompt: &str) -> Option<ServerResourcePackInfo> {
    if url.is_empty() {
        return None;
    }
    let hash = if !sha1.is_empty() {
        if legacy_hash.is_some_and(|h| !h.is_empty()) {
            warn!("resource-pack-hash is deprecated and found along side resource-pack-sha1. resource-pack-hash will be ignored.");
        }
        sha1.to_string()
    } else if let Some(hash) = legacy_hash.filter(|h| !h.is_empty()) {
        warn!("resource-pack-hash is deprecated. Please use resource-pack-sha1 instead.");
        hash.to_string()
    } else {
        String::new()
    };
    if hash.is_empty() {
        warn!("You specified a resource pack without providing a sha1 hash. Pack will be updated on the client only if you change the name of the pack.");
    } else if !(hash.len() == 40 && hash.bytes().all(|b| b.is_ascii_hexdigit())) {
        warn!("Invalid sha1 for resource-pack-sha1");
    }
    let prompt = parse_resource_pack_prompt(prompt);
    let id = if id.is_empty() {
        let id = name_uuid_from_bytes(url.as_bytes());
        warn!("resource-pack-id missing, using default of {}", crate::world::level::storage::format_uuid(id));
        id
    } else {
        match crate::commands::arguments::java_uuid_from_string(id) {
            Some(id) => id,
            None => {
                warn!("Failed to parse '{id}' into UUID");
                return None;
            }
        }
    };
    Some(ServerResourcePackInfo { id, url: url.to_string(), hash, is_required: required, prompt })
}

fn parse_resource_pack_prompt(prompt: &str) -> Option<crate::network::chat::Component> {
    if prompt.is_empty() {
        return None;
    }
    match serde_json::from_str::<serde_json::Value>(prompt) {
        Ok(json) => match crate::nbt::from_json(&json).ok_or_else(|| "Empty".to_string()).and_then(|tag| crate::network::chat::component_serialization::decode(&tag)) {
            Ok(component) => Some(component),
            Err(e) => {
                warn!("Failed to parse resource pack prompt '{prompt}': {e}");
                None
            }
        },
        Err(_) => {
            warn!("Failed to parse resource pack prompt '{prompt}'");
            None
        }
    }
}

pub fn name_uuid_from_bytes(bytes: &[u8]) -> u128 {
    use md5::{Digest, Md5};
    let mut hash: [u8; 16] = Md5::digest(bytes).into();
    hash[6] = hash[6] & 0x0f | 0x30;
    hash[8] = hash[8] & 0x3f | 0x80;
    u128::from_be_bytes(hash)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_vanilla() {
        let properties = DedicatedServerProperties::new(Properties::default()).unwrap();
        assert_eq!(properties.properties.get("level-type"), Some("minecraft:normal"));
        assert_eq!(properties.properties.get("generator-settings"), Some("{}"));
        assert_eq!(properties.properties.get("initial-enabled-packs"), Some("vanilla"));
        assert_eq!(properties.properties.get("function-permission-level"), Some("2"));
        assert_eq!(properties.properties.get("management-server-secret").map(str::len), Some(40));
        assert_eq!((properties.difficulty, properties.game_mode, properties.seed), (Difficulty::Easy, GameType::Survival, None));
    }

    #[test]
    fn values_are_normalized_like_vanilla() {
        let mut raw = Properties::default();
        raw.put("difficulty", "6".into());
        raw.put("gamemode", "1".into());
        raw.put("max-players", "lots".into());
        raw.put("op-permission-level", "9".into());
        raw.put("level-seed", "Crisp".into());
        raw.put("level-type", "MINECRAFT:FLAT".into());
        raw.put("announce-player-achievements", "true".into());
        let properties = DedicatedServerProperties::new(raw).unwrap();
        assert_eq!(properties.properties.get("difficulty"), Some("normal"));
        assert_eq!(properties.properties.get("gamemode"), Some("creative"));
        assert_eq!(properties.properties.get("max-players"), Some("20"));
        assert_eq!(properties.properties.get("op-permission-level"), Some("4"));
        assert_eq!(properties.seed, Some(65_376_663));
        assert_eq!(properties.level_type, "minecraft:flat");
        assert_eq!(properties.announce_player_achievements, Some(true));
        assert_eq!(properties.properties.get("announce-player-achievements"), None);
    }
}
