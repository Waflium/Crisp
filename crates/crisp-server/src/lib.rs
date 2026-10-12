#[macro_use]
pub mod logging;
mod advancements;
pub mod crash_report;
mod commands;
mod core;
mod nbt;
mod resources;
mod tags;
mod network;
mod recipebook;
pub mod server;
mod stats;
mod util;
mod world;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::net::TcpListener;
use tokio::sync::{mpsc, oneshot};

use crate::server::dedicated::dedicated_server_properties::{DedicatedServerProperties, ServerResourcePackInfo};
use crate::server::dedicated::dedicated_server_settings::DedicatedServerSettings;
use crate::server::services::Services;

#[derive(Clone)]
pub struct ServerConfig {
    pub motd: String,
    pub max_players: u32,
    pub view_distance: i32,
    pub simulation_distance: i32,
    pub compression_threshold: Option<usize>,
    pub seed: Option<i64>,
    pub level_name: String,
    pub world: Option<PathBuf>,
    pub server_dir: Option<PathBuf>,
    pub op_permission_level: i32,
    pub function_permission_level: i32,
    pub broadcast_console_to_ops: bool,
    pub online_mode: bool,
    pub white_list: bool,
    pub enforce_whitelist: bool,
    pub spawn_protection: i32,
    pub pause_when_empty_seconds: i32,
    pub player_idle_timeout: i32,
    pub enforce_secure_profile: bool,
    pub chat_spam_threshold_seconds: i32,
    pub command_spam_threshold_seconds: i32,
    pub game_mode: server::level::game_mode::GameType,
    pub force_game_mode: bool,
    pub difficulty: world::difficulty::Difficulty,
    pub level_type: LevelType,
    pub max_world_size: i32,
    pub log_ips: bool,
    pub broadcast_rcon_to_ops: bool,
    pub prevent_proxy_connections: bool,
    pub allow_flight: bool,
    pub sync_chunk_writes: bool,
    pub max_chained_neighbor_updates: i32,
    pub entity_broadcast_range_percentage: i32,
    pub rate_limit_packets_per_second: i32,
    pub replies_to_status: bool,
    pub hides_online_players: bool,
    pub accepts_transfers: bool,
    pub server_resource_pack: Option<ServerResourcePackInfo>,
    pub server_links: Arc<[server::server_links::Entry]>,
    pub code_of_conducts: Arc<[(String, String)]>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LevelType {
    Normal,
    Flat,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            motd: "A Minecraft Server".into(),
            max_players: 20,
            view_distance: 10,
            simulation_distance: 10,
            compression_threshold: Some(256),
            seed: None,
            level_name: "world".into(),
            world: None,
            server_dir: None,
            op_permission_level: 4,
            function_permission_level: 2,
            broadcast_console_to_ops: true,
            online_mode: true,
            white_list: true,
            enforce_whitelist: false,
            pause_when_empty_seconds: 60,
            player_idle_timeout: 0,
            enforce_secure_profile: true,
            chat_spam_threshold_seconds: 10,
            command_spam_threshold_seconds: 10,
            spawn_protection: 16,
            game_mode: server::level::game_mode::GameType::Survival,
            force_game_mode: false,
            difficulty: world::difficulty::Difficulty::Easy,
            level_type: LevelType::Normal,
            max_world_size: 29_999_984,
            log_ips: true,
            broadcast_rcon_to_ops: true,
            prevent_proxy_connections: false,
            allow_flight: false,
            sync_chunk_writes: true,
            max_chained_neighbor_updates: 1_000_000,
            entity_broadcast_range_percentage: 100,
            rate_limit_packets_per_second: 0,
            replies_to_status: true,
            hides_online_players: false,
            accepts_transfers: false,
            server_resource_pack: None,
            server_links: Arc::new([]),
            code_of_conducts: Arc::new([]),
        }
    }
}

impl ServerConfig {
    pub fn from_properties(properties: &DedicatedServerProperties, universe: &Path, level_name: String) -> ServerConfig {
        let level_type = match properties.level_type.as_str() {
            "minecraft:flat" | "flat" => LevelType::Flat,
            _ => LevelType::Normal,
        };
        ServerConfig {
            motd: properties.motd.clone(),
            max_players: properties.max_players.max(0) as u32,
            view_distance: properties.view_distance,
            simulation_distance: properties.simulation_distance,
            compression_threshold: (properties.network_compression_threshold >= 0).then_some(properties.network_compression_threshold as usize),
            seed: properties.seed,
            world: Some(universe.join(&level_name)),
            level_name,
            server_dir: Some(".".into()),
            op_permission_level: properties.op_permission_level,
            function_permission_level: properties.function_permission_level,
            broadcast_console_to_ops: properties.broadcast_console_to_ops,
            online_mode: properties.online_mode,
            white_list: properties.white_list,
            enforce_whitelist: properties.enforce_whitelist,
            spawn_protection: properties.spawn_protection,
            pause_when_empty_seconds: properties.pause_when_empty_seconds,
            player_idle_timeout: properties.player_idle_timeout,
            enforce_secure_profile: properties.enforce_secure_profile,
            chat_spam_threshold_seconds: properties.chat_spam_threshold_seconds,
            command_spam_threshold_seconds: properties.command_spam_threshold_seconds,
            game_mode: properties.game_mode,
            force_game_mode: properties.force_game_mode,
            difficulty: properties.difficulty,
            level_type,
            max_world_size: properties.max_world_size,
            log_ips: properties.log_ips,
            broadcast_rcon_to_ops: properties.broadcast_rcon_to_ops,
            prevent_proxy_connections: properties.prevent_proxy_connections,
            allow_flight: properties.allow_flight,
            sync_chunk_writes: properties.sync_chunk_writes,
            max_chained_neighbor_updates: properties.max_chained_neighbor_updates,
            entity_broadcast_range_percentage: properties.entity_broadcast_range_percentage,
            rate_limit_packets_per_second: properties.rate_limit_packets_per_second,
            replies_to_status: properties.enable_status,
            hides_online_players: properties.hide_online_players,
            accepts_transfers: properties.accepts_transfers,
            server_resource_pack: properties.server_resource_pack_info.clone(),
            server_links: server::dedicated::create_server_links(properties).into(),
            code_of_conducts: Arc::new([]),
        }
    }
}

pub enum Listen {
    Bind { ip: String, port: u16 },
    Bound(TcpListener),
}

pub async fn serve(listen: Listen, config: ServerConfig, services: Services, settings: Option<DedicatedServerSettings>, console: bool) -> anyhow::Result<()> {
    serve_world(listen, config, services, settings, console, None, None).await
}

pub(crate) async fn serve_world(
    listen: Listen,
    config: ServerConfig,
    services: Services,
    settings: Option<DedicatedServerSettings>,
    console: bool,
    world: Option<(server::packs::repository::PackRepository, server::world_stem::WorldStem)>,
    management_server: Option<Arc<server::jsonrpc::management_server::ManagementServer>>,
) -> anyhow::Result<()> {
    let (events, queue) = mpsc::unbounded_channel();
    let runtime = tokio::runtime::Handle::current();
    let (done, finished) = oneshot::channel();
    let server_events = events.clone();
    std::thread::Builder::new().name("Server thread".into()).spawn(move || {
        let _ = done.send(server::run(config, settings, world, services, queue, server_events, runtime, listen, console, management_server));
    })?;
    if console {
        tokio::spawn(async move {
            if tokio::signal::ctrl_c().await.is_ok() {
                let _ = events.send(server::Event::Halt);
            }
        });
    }
    finished.await?
}
