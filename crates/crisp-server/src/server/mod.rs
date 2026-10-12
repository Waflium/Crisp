pub mod bossevents;
pub mod commands;
pub mod dedicated;
pub mod level;
pub mod network;
pub mod packs;
pub mod players;
pub mod rcon;
pub mod reloadable_server_registries;
pub mod reloadable_server_resources;
pub mod eula;
pub mod jsonrpc;
pub mod main;
pub mod notifications;
pub mod server_tick_rate_manager;
pub mod services;
#[cfg(test)]
mod scoreboard_tests;
pub mod server_function_library;
pub mod server_function_manager;
pub mod server_links;
pub mod server_scoreboard;
pub mod world_stem;

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use crisp_core::mth;
use crisp_core::random::{LegacyRandomSource, RandomSource};

use crisp_core::BlockPos;
use crisp_data::packets::play::{clientbound, serverbound};
use crisp_data::{DIMENSION_TYPES, SyncedRegistry, game_rule};
use crisp_protocol::nbt::text;
use sha2::{Digest, Sha256};
use tokio::sync::mpsc;

use crate::network::chat::last_seen_messages::LastSeenMessages;
use crate::network::chat::message_signature::MessageSignature;
use crate::network::chat::signed_message_body::SignedMessageBody;
use crate::server::network::chat::{ArgumentSignature, ChatState};
use crate::{LevelType, ServerConfig};
use crate::commands::source::CommandSourceStack;
use crate::network::chat::{ChatFormatting, Component, player_joined_or_left};
use crate::network::configuration::{ChatVisibility, CommonListenerCookie};
use crate::network::server_status::Favicon;
use crate::network::connection::{Outbound, Packet};
use crate::network::login::{Profile, Verdict};
use crate::world::clock::ServerClockManager;
use crate::world::entity::EntityId;
use crate::world::level::chunk::{ChunkGenerator, FlatLevelSource};
use crate::world::level::levelgen::NoiseBasedChunkGenerator;
use crate::world::level::chunk::storage::RegionStorage;
use crate::world::level::gamerules::GameRules;
use crate::world::level::border::border_change_listener::write_initialize_border;
use crate::world::level::border::world_border::WorldBorder;
use crate::world::difficulty::Difficulty;
use crate::world::level::storage::format_uuid;
use crate::server::services::Services;
use crate::world::level::storage::command_storage::CommandStorage;
use crate::world::random_sequences::RandomSequences;
use crate::world::level::timers::timer_queue::TimerQueue;
use self::players::NameAndId;
use self::players::cached_user_name_to_id_resolver::CachedUserNameToIdResolver;
use crate::world::stopwatches::Stopwatches;
use self::server_tick_rate_manager::ServerTickRateManager;
use self::bossevents::custom_boss_events::CustomBossEvents;
use crate::world::level::saveddata::weather_data::WeatherData;
use crate::world::scores::scoreboard::Scoreboard;
use crate::world::scores::scoreboard_save_data::ScoreboardSaveData;
use crate::world::level::storage::{RespawnData, WorldData, WorldDir, write_dat};
use level::ServerLevel;
use self::network::prepare_spawn_task::PrepareSpawnTask;
use level::game_mode::GameType;
use level::chunk_cache::ChunkCache;
use level::server_player::ServerPlayer;
use self::packs::PackSource;
use self::packs::repository::PackRepository;
use self::packs::resources::MultiPackResourceManager;
use self::reloadable_server_resources::ReloadableServerResources;
use self::world_stem::WorldStem;
use self::server_function_manager::ServerFunctionManager;
use self::jsonrpc::management_server::ManagementServer;
use self::notifications::NotificationManager;
use self::notifications::server_activity_monitor::ServerActivityMonitor;
use crate::network::server_connection_listener::ListenerSettings;
use crate::world::flag::{FeatureFlagSet, print_missing_flags};
use crate::world::level::data_pack_config::DataPackConfig;
use crate::world::level::world_data_configuration::WorldDataConfiguration;

pub const OVERWORLD: &str = "minecraft:overworld";
pub const THE_NETHER: &str = "minecraft:the_nether";
pub const THE_END: &str = "minecraft:the_end";

fn create_level(config: &ServerConfig, world_dir: Option<&WorldDir>, dimension: &'static str, generator: ChunkGenerator, generate_structures: bool, seed: i64) -> ServerLevel {
    let dimension_type_id = SyncedRegistry::get("minecraft:dimension_type").id_of(dimension).expect("a dimension type");
    let dimension_type = &DIMENSION_TYPES[dimension_type_id as usize];
    let storage = world_dir.map(|d| RegionStorage::new(d.regions(dimension), config.sync_chunk_writes));
    let mut chunks = ChunkCache::new(dimension_type, generator, storage);
    chunks.generate_structures = generate_structures;
    chunks.set_seed(seed);
    if let Some(dir) = world_dir {
        chunks.set_poi_storage(RegionStorage::new(dir.poi(dimension), config.sync_chunk_writes));
    }
    let entity_storage = world_dir.map(|d| RegionStorage::new(d.entities(dimension), config.sync_chunk_writes));
    let mut level = ServerLevel::new(dimension, dimension_type_id, chunks, entity_storage, config.view_distance, config.simulation_distance);
    if let Some(dir) = world_dir {
        level.load_chunk_tickets(dir);
    }
    level.biome_zoom_seed = crate::world::level::biome::obfuscate_seed(seed);
    level.world_border = WorldBorder::load(world_dir.map(|d| d.dimension_data_folder(dimension)));
    level.world_border.set_absolute_max_size(config.max_world_size);
    level.raids = crate::world::entity::raid::raids::Raids::load(world_dir.map(|d| d.dimension_data_folder(dimension)));
    level.level_name = config.level_name.clone();
    level.allow_flight = config.allow_flight;
    level.set_max_chained_neighbor_updates(config.max_chained_neighbor_updates);
    level.entity_broadcast_range_percentage = config.entity_broadcast_range_percentage;
    level.seed = seed;
    level
}
const AUTOSAVE_INTERVAL: i32 = 6000;
const STATUS_EXPIRE_TIME: Duration = Duration::from_secs(5);
const MAX_STATUS_PLAYER_SAMPLE: usize = 12;
const EVENT_PERMISSION_LEVEL_ALL: u8 = 24;
const GAME_EVENT_LIMITED_CRAFTING: u8 = 12;
const GAME_EVENT_LEVEL_CHUNKS_LOAD_START: u8 = 13;
const GAME_EVENT_IMMEDIATE_RESPAWN: u8 = 11;

pub type ConnId = u64;

pub enum Event {
    Join { conn: ConnId, profile: Profile, cookie: CommonListenerCookie, out: Outbound, address: std::net::SocketAddr },
    Packet { conn: ConnId, packet: Packet },
    Left { conn: ConnId, reason: Component },
    Console(String),
    Verify { profile: Profile, address: std::net::SocketAddr, reply: tokio::sync::oneshot::Sender<Verdict> },
    PrepareSpawn { uuid: u128, name: String, reply: tokio::sync::oneshot::Sender<()> },
    PrepareSpawnClosed { uuid: u128 },
    Halt,
    Rcon { command: String, reply: tokio::sync::oneshot::Sender<String> },
}

pub type ServerTask = Box<dyn FnOnce(&mut Server) + Send + Sync>;

pub(crate) enum Task {
    Chat { player: EntityId, body: SignedMessageBody, signature: Option<MessageSignature> },
    Command { player: EntityId, command: String },
    SignedCommand { player: EntityId, command: String, time_stamp: i64, salt: i64, arguments: Vec<ArgumentSignature>, last_seen: LastSeenMessages },
    Chained { player: EntityId, run: Box<dyn FnOnce(&mut Server)> },
    CommandSuggestions { player: EntityId },
    Rcon { command: String, reply: tokio::sync::oneshot::Sender<String> },
}

struct PendingLogin {
    profile: Profile,
    address: std::net::SocketAddr,
    reply: tokio::sync::oneshot::Sender<Verdict>,
    waiting_for_dupe: bool,
}

pub struct Server {
    pub config: ServerConfig,
    pub settings: Option<dedicated::dedicated_server_settings::DedicatedServerSettings>,
    pub levels: Vec<ServerLevel>,
    pub player_list: Vec<EntityId>,
    player_names_array: Arc<RwLock<Arc<[String]>>>,
    connections: HashMap<ConnId, EntityId>,
    pub clocks: ServerClockManager,
    pub game_rules: GameRules,
    pub weather: Rc<Cell<WeatherData>>,
    pub scoreboard: Rc<RefCell<Scoreboard>>,
    pub command_storage: Rc<RefCell<CommandStorage>>,
    pub random_sequences: Rc<RefCell<RandomSequences>>,
    pub ops: players::ServerOpList,
    pub bans: players::StoredUserList<players::UserBanEntry>,
    pub ip_bans: players::StoredUserList<players::IpBanEntry>,
    pub whitelist: players::StoredUserList<players::WhiteListEntry>,
    empty_ticks: i32,
    send_all_player_info_in: i32,
    pub seed: i64,
    pub tick_count: i32,
    ticks_until_autosave: i32,
    pub tick_rate_manager: ServerTickRateManager,
    tick_times_nanos: [i64; 100],
    aggregated_tick_times_nanos: i64,
    pending_logins: Vec<PendingLogin>,
    prepare_spawn_tasks: Vec<PrepareSpawnTask>,
    tasks: VecDeque<Task>,
    console_inputs: Vec<(String, CommandSourceStack)>,
    pub rcon_buffer: std::cell::RefCell<String>,
    random: LegacyRandomSource,
    status: Arc<RwLock<Option<Arc<str>>>>,
    last_status: Option<Instant>,
    status_icon: Option<Favicon>,
    session_id: Option<u128>,
    #[cfg(test)]
    pub console_log: std::cell::RefCell<Vec<String>>,
    pub spawn: RespawnData,
    effective_respawn_data: RespawnData,
    pub default_game_type: GameType,
    pub difficulty: Difficulty,
    pub difficulty_locked: bool,
    pub hardcore: bool,
    generate_structures: bool,
    server_brands: Vec<String>,
    was_modded: bool,
    pub scheduled_events: TimerQueue,
    pub name_to_id_cache: CachedUserNameToIdResolver,
    pub custom_boss_events: CustomBossEvents,
    pub stopwatches: Stopwatches,
    pub services: Services,
    pub functions: ServerFunctionManager,
    pub pack_repository: PackRepository,
    pub data_configuration: WorldDataConfiguration,
    pub scoreboard_save_data: ScoreboardSaveData,
    pub notification_manager: NotificationManager,
    pub server_activity_monitor: Arc<ServerActivityMonitor>,
    pub listener_settings: Arc<ListenerSettings>,
    world_dir: Option<WorldDir>,
    stopping: bool,
    ready: bool,
}

impl Server {
    pub(crate) fn new(config: ServerConfig) -> std::io::Result<Server> {
        let saved = match &config.world {
            Some(root) => WorldData::load(&WorldDir { root: root.clone() })?,
            None => None,
        };
        let mut pack_repository = match &config.world {
            Some(root) => {
                let validator = crate::world::level::storage::parse_validator(&root.parent().unwrap_or(Path::new("")).join(crate::world::level::storage::ALLOWED_SYMLINKS_CONFIG_NAME));
                PackRepository::create_pack_repository(root.join("datapacks"), Arc::new(validator))
            }
            None => PackRepository::create_vanilla_trusted_repository(),
        };
        let initial_data_config = saved.as_ref().map_or_else(WorldDataConfiguration::default_configuration, |data| data.data_configuration.clone());
        let data_configuration = configure_pack_repository(&mut pack_repository, &initial_data_config, saved.is_none(), false);
        let resource_manager = MultiPackResourceManager::new(pack_repository.open_all_selected());
        let postponed_tags = crate::tags::load_tags_for_existing_registries(&resource_manager);
        let mut data_packs = ReloadableServerResources::load_resources(&resource_manager, postponed_tags, config.function_permission_level).map_err(std::io::Error::other)?;
        data_packs.update_components_and_static_registry_tags();
        data_packs.finalize_recipe_loading();
        crate::world::level::levelgen::structure::templatesystem::structure_template_manager::on_resource_manager_reload(&resource_manager);
        let mut world_data = match saved {
            Some(data) => data,
            None => {
                let seed = config.seed.unwrap_or_else(rand::random);
                let generator = match config.level_type {
                    LevelType::Flat => ChunkGenerator::Flat(FlatLevelSource::default()),
                    LevelType::Normal => ChunkGenerator::Noise(Arc::new(NoiseBasedChunkGenerator::overworld(seed))),
                };
                WorldData::new_world(seed, generator, true, config.game_mode, config.difficulty, false, data_configuration.clone())
            }
        };
        world_data.data_configuration = data_configuration;
        Server::with_world_stem(config, pack_repository, WorldStem { data_packs, world_data })
    }

    pub(crate) fn with_world_stem(config: ServerConfig, pack_repository: PackRepository, world_stem: WorldStem) -> std::io::Result<Server> {
        let WorldStem { data_packs, world_data: data } = world_stem;
        let data_configuration = data.data_configuration.clone();
        let new_world = !data.initialized;
        let world_dir = config.world.clone().map(|root| WorldDir { root });
        let mut clocks = ServerClockManager::new();
        let game_rules = GameRules::default();
        if !new_world {
            game_rules.load(&data.game_rules);
            clocks.load(&data.clocks);
        }
        let weather = data.weather;
        let generate_structures = data.generate_structures;
        let (mut server_brands, was_modded) = (data.server_brands, data.was_modded);
        if !server_brands.iter().any(|b| b == "vanilla") {
            server_brands.push("vanilla".into());
        }
        let mut load_listener = level::progress::LoggingLevelLoadListener::for_dedicated_server();
        let spawn = (!new_world).then_some(data.spawn);
        let (seed, game_time, generator, game_type, difficulty, difficulty_locked, hardcore) =
            (data.seed, data.game_time, data.generator, data.game_type, data.difficulty, data.difficulty_locked, data.hardcore);
        let mut overworld = create_level(&config, world_dir.as_ref(), OVERWORLD, generator, generate_structures, seed);
        if new_world {
            load_listener.start(level::progress::Stage::PrepareGlobalSpawn, 0);
        }
        let spawn = match spawn {
            Some(spawn) => spawn,
            None => RespawnData::of(OVERWORLD, level::player_spawn_finder::initial_spawn(&mut overworld.chunks), 0.0, 0.0),
        };
        if new_world {
            load_listener.finish(level::progress::Stage::PrepareGlobalSpawn);
        }
        let mut levels = vec![overworld];
        for (dimension, generator) in [
            (THE_NETHER, ChunkGenerator::Noise(Arc::new(NoiseBasedChunkGenerator::nether(seed)))),
            (THE_END, ChunkGenerator::Noise(Arc::new(NoiseBasedChunkGenerator::end(seed)))),
        ] {
            levels.push(create_level(&config, world_dir.as_ref(), dimension, generator, generate_structures, seed));
        }
        let weather = Rc::new(Cell::new(weather));
        let scoreboard_save_data = ScoreboardSaveData::load(world_dir.as_ref().map(WorldDir::data_folder));
        let scoreboard = Rc::new(RefCell::new(Scoreboard::default()));
        scoreboard.borrow_mut().load(scoreboard_save_data.data());
        let command_storage = Rc::new(RefCell::new(CommandStorage::new(world_dir.as_ref().map(WorldDir::data_folder))));
        let random_sequences = Rc::new(RefCell::new(RandomSequences::load(world_dir.as_ref().map(WorldDir::data_folder))));
        let tick_rate_manager = ServerTickRateManager::default();
        let main_thread_wakeup = levels[0].chunks.main_thread_wakeup();
        for level in &mut levels {
            level.chunks.set_main_thread_wakeup(main_thread_wakeup.clone());
            level.game_rules = game_rules.clone();
            level.weather = weather.clone();
            level.scoreboard = scoreboard.clone();
            level.command_storage = command_storage.clone();
            level.random_sequences = random_sequences.clone();
            level.tick_rate_manager = tick_rate_manager.shared();
            level.prepare_weather();
            level.sync_clocks(&clocks);
            level.overworld_clock_time = clocks.clocks[ServerClockManager::id_of(OVERWORLD).unwrap()].total_ticks;
            level.update_sky_brightness();
            level.game_time = game_time;
            level.chunks.game_time = game_time;
        }
        let name_to_id_cache = CachedUserNameToIdResolver::load(config.server_dir.clone());
        name_to_id_cache.save();
        let lists = players::PlayerLists::load(config.server_dir.clone());
        let (config_replies_to_status, config_accepts_transfers) = (config.replies_to_status, config.accepts_transfers);
        let mut server = Server {
            config,
            settings: None,
            levels,
            player_list: Vec::new(),
            player_names_array: Arc::default(),
            connections: HashMap::new(),
            clocks,
            game_rules,
            weather,
            scoreboard,
            command_storage,
            random_sequences,
            ops: lists.ops,
            bans: lists.bans,
            ip_bans: lists.ip_bans,
            whitelist: lists.whitelist,
            empty_ticks: 0,
            send_all_player_info_in: 0,
            seed,
            tick_count: 0,
            ticks_until_autosave: AUTOSAVE_INTERVAL,
            tick_rate_manager,
            tick_times_nanos: [0; 100],
            aggregated_tick_times_nanos: 0,
            pending_logins: Vec::new(),
            prepare_spawn_tasks: Vec::new(),
            tasks: VecDeque::new(),
            console_inputs: Vec::new(),
            rcon_buffer: std::cell::RefCell::new(String::new()),
            random: LegacyRandomSource::create(),
            status: Arc::default(),
            last_status: None,
            status_icon: None,
            session_id: None,
            #[cfg(test)]
            console_log: Default::default(),
            effective_respawn_data: spawn.clone(),
            spawn,
            default_game_type: game_type,
            difficulty,
            difficulty_locked,
            hardcore,
            generate_structures,
            server_brands,
            was_modded,
            scheduled_events: TimerQueue::load(world_dir.as_ref().map(WorldDir::data_folder)),
            name_to_id_cache,
            custom_boss_events: CustomBossEvents::load(world_dir.as_ref().map(WorldDir::data_folder)),
            stopwatches: Stopwatches::load(world_dir.as_ref().map(WorldDir::data_folder)),
            services: Services::offline(),
            functions: ServerFunctionManager::new(data_packs.function_library),
            pack_repository,
            data_configuration,
            scoreboard_save_data,
            notification_manager: NotificationManager::default(),
            server_activity_monitor: Arc::new(ServerActivityMonitor::new(NotificationManager::default(), 30)),
            listener_settings: Arc::new(ListenerSettings::new(config_replies_to_status, config_accepts_transfers)),
            world_dir,
            stopping: false,
            ready: false,
        };
        server.name_to_id_cache.set_resolve_offline_users(!server.config.online_mode);
        server.set_difficulty(server.config.difficulty, true);
        server.prepare_levels(&mut load_listener);
        server.update_effective_respawn_data();
        Ok(server)
    }

    pub fn watchdog_snapshot(&self) -> dedicated::server_watchdog::ServerSnapshot {
        let players: Vec<String> = self
            .players()
            .map(|p| {
                let pos = p.entity.position();
                format!(
                    "ServerPlayer['{}'/{}, l='ServerLevel[{}]', x={:.2}, y={:.2}, z={:.2}]",
                    p.profile.name, p.entity.id, self.config.level_name, pos.x, pos.y, pos.z
                )
            })
            .collect();
        let level_stats = self.levels.iter().map(|level| format!("{}: {}", level.dimension, level.get_watchdog_stats())).collect::<Vec<_>>().join(",\n");
        dedicated::server_watchdog::ServerSnapshot {
            player_count: format!("{} / {}; [{}]", players.len(), self.config.max_players, players.join(", ")),
            random_tick_rate: self.game_rules.get_int(crisp_data::game_rule::RANDOM_TICK_SPEED).to_string(),
            level_stats,
            world_seed: self.seed,
        }
    }

    fn prepare_levels(&mut self, listener: &mut level::progress::LoggingLevelLoadListener) {
        const PREPARE_LEVELS_DEFAULT_DELAY: Duration = Duration::from_millis(10);
        let mut counter = level::progress::ChunkLoadCounter::default();
        for (index, level) in self.levels.iter_mut().enumerate() {
            counter.track(index, &mut level.chunks, ChunkCache::activate_all_deactivated_tickets);
        }
        listener.start(level::progress::Stage::LoadInitialChunks, counter.total_chunks());
        loop {
            let ready = counter.ready_chunks(&self.levels);
            listener.update(level::progress::Stage::LoadInitialChunks, ready, counter.total_chunks());
            self.wait_until_next_tick(Instant::now() + PREPARE_LEVELS_DEFAULT_DELAY);
            if counter.pending_chunks(&self.levels) == 0 {
                break;
            }
        }
        listener.finish(level::progress::Stage::LoadInitialChunks);
    }

    fn wait_until_next_tick(&mut self, deadline: Instant) {
        let wakeup = self.overworld().chunks.main_thread_wakeup();
        while let Some(wait) = deadline.checked_duration_since(Instant::now()).filter(|w| !w.is_zero()) {
            let seen = wakeup.generation();
            if !self.poll_task() {
                wakeup.wait(seen, wait);
            }
        }
    }

    fn poll_task(&mut self) -> bool {
        let mut did_something = false;
        for level in &mut self.levels {
            did_something |= level.poll_chunk_task();
        }
        did_something
    }

    pub fn overworld(&self) -> &ServerLevel {
        &self.levels[0]
    }

    pub fn overworld_mut(&mut self) -> &mut ServerLevel {
        &mut self.levels[0]
    }

    pub fn get_level(&self, dimension: &str) -> Option<usize> {
        self.levels.iter().position(|level| level.dimension == dimension)
    }

    pub fn players(&self) -> impl Iterator<Item = &ServerPlayer> {
        self.player_list.iter().map(|&id| self.player(id))
    }

    pub fn player_ids(&self) -> Vec<EntityId> {
        self.player_list.clone()
    }

    pub fn player_count(&self) -> usize {
        self.player_list.len()
    }

    pub fn get_player(&self, id: EntityId) -> Option<&ServerPlayer> {
        self.levels.iter().find_map(|level| level.players.get(&id))
    }

    pub fn get_player_mut(&mut self, id: EntityId) -> Option<&mut ServerPlayer> {
        self.levels.iter_mut().find_map(|level| level.players.get_mut(&id))
    }

    pub fn player(&self, id: EntityId) -> &ServerPlayer {
        self.get_player(id).expect("player on this server")
    }

    pub fn player_mut(&mut self, id: EntityId) -> &mut ServerPlayer {
        self.get_player_mut(id).expect("player on this server")
    }

    pub fn has_player(&self, id: EntityId) -> bool {
        self.levels.iter().any(|level| level.players.contains_key(&id))
    }

    pub fn level_of(&self, entity: EntityId) -> Option<usize> {
        self.levels.iter().position(|level| level.players.contains_key(&entity) || level.entities.contains_key(&entity))
    }

    pub fn entity_level(&self, entity: EntityId) -> &ServerLevel {
        &self.levels[self.level_of(entity).expect("entity on this server")]
    }

    pub fn entity_level_mut(&mut self, entity: EntityId) -> &mut ServerLevel {
        let index = self.level_of(entity).expect("entity on this server");
        &mut self.levels[index]
    }

    pub fn set_difficulty(&mut self, difficulty: Difficulty, ignore_lock: bool) {
        if ignore_lock || !self.difficulty_locked {
            self.difficulty = if self.hardcore { Difficulty::Hard } else { difficulty };
            for level in &mut self.levels {
                level.difficulty = self.difficulty;
                level.chunks.difficulty = self.difficulty;
            }
            for player in self.players() {
                self.send_difficulty_update(&player.connection);
            }
        }
    }

    fn send_difficulty_update(&self, connection: &Outbound) {
        connection.send(clientbound::CHANGE_DIFFICULTY, |p| {
            p.varint(self.difficulty.id());
            p.bool(self.difficulty_locked);
        });
    }

    pub fn forced_game_type(&self) -> Option<GameType> {
        self.config.force_game_mode.then_some(self.default_game_type)
    }

    pub fn enforce_game_type_for_players(&mut self, game_type: Option<GameType>) -> i32 {
        let Some(game_type) = game_type else { return 0 };
        let ids = self.player_ids();
        ids.into_iter().filter(|&id| self.entity_level_mut(id).set_game_mode(id, game_type)).count() as i32
    }

    fn calculate_game_mode_for_new_player(&self, loaded: Option<GameType>) -> GameType {
        self.forced_game_type().unwrap_or(loaded.unwrap_or(self.default_game_type))
    }

    pub fn set_respawn_data(&mut self, data: RespawnData) {
        if self.spawn == data {
            return;
        }
        for player in self.players() {
            player.connection.send(clientbound::SET_DEFAULT_SPAWN_POSITION, |p| write_respawn_data(p, &data));
        }
        self.spawn = data;
        self.update_effective_respawn_data();
    }

    fn update_effective_respawn_data(&mut self) {
        let respawn_level = self.find_respawn_dimension();
        self.effective_respawn_data = self.levels[respawn_level].get_world_border_adjusted_respawn_data(self.spawn.clone());
    }

    pub fn find_respawn_dimension(&self) -> usize {
        self.get_level(&self.spawn.dimension).unwrap_or(0)
    }

    pub fn respawn_data(&self) -> &RespawnData {
        &self.effective_respawn_data
    }

    fn write_spawn(&self, p: &mut crisp_protocol::Writer) {
        write_respawn_data(p, &self.effective_respawn_data);
    }

    pub fn set_weather_parameters(&mut self, clear_time: i32, rain_time: i32, raining: bool, thundering: bool) {
        let mut weather = self.weather.get();
        weather.clear_weather_time = clear_time;
        weather.rain_time = rain_time;
        weather.thunder_time = rain_time;
        weather.raining = raining;
        weather.thundering = thundering;
        self.weather.set(weather);
    }

    pub fn save_everything(&mut self, silent: bool, flush: bool, force: bool) -> bool {
        for player in self.players() {
            self.save_player(player);
        }
        self.save_all_chunks(silent, flush, force)
    }

    pub fn save_all_chunks(&mut self, silent: bool, flush: bool, force: bool) -> bool {
        self.notification_manager.server_save_started();
        let saved_chunks = self.save_all_level_chunks(silent, flush, force);
        self.notification_manager.server_save_completed();
        saved_chunks
    }

    fn save_all_level_chunks(&mut self, silent: bool, flush: bool, force: bool) -> bool {
        let Some(dir) = &self.world_dir else { return false };
        self.scoreboard.borrow_mut().store_to_save_data_if_dirty(&mut self.scoreboard_save_data);
        for level in &mut self.levels {
            if !silent {
                info!("Saving chunks for level 'ServerLevel[{}]'/{}", self.config.level_name, level.dimension);
            }
            let no_save = level.no_save && !force;
            level.save(dir, flush, no_save);
        }
        let data = WorldData {
            seed: self.seed,
            game_time: self.overworld().game_time,
            spawn: self.spawn.clone(),
            game_type: self.default_game_type,
            difficulty: self.difficulty,
            difficulty_locked: self.difficulty_locked,
            hardcore: self.hardcore,
            clocks: self.clocks.to_nbt(),
            weather: self.weather.get(),
            generator: self.overworld().chunks.generator.clone(),
            generate_structures: self.generate_structures,
            initialized: true,
            server_brands: self.server_brands.clone(),
            was_modded: self.was_modded,
            game_rules: self.game_rules.to_nbt(),
            data_configuration: self.data_configuration.clone(),
        };
        if data.save(dir).is_err() {
            error!("Failed to save level {}", dir.root.display());
        }
        log_save_failure("command_storage.dat", self.command_storage.borrow_mut().save());
        log_save_failure("random_sequences.dat", self.random_sequences.borrow_mut().save());
        log_save_failure("scheduled_events.dat", self.scheduled_events.save());
        log_save_failure("custom_boss_events.dat", self.custom_boss_events.save());
        log_save_failure("stopwatches.dat", self.stopwatches.save());
        log_save_failure("scoreboard.dat", self.scoreboard_save_data.save());
        true
    }

    fn refresh_spawn_protection(&mut self) {
        let ops: Vec<u128> = self.ops.entries().into_iter().map(|e| e.user.id).collect();
        for level in &mut self.levels {
            let protection = &mut level.spawn_protection;
            protection.radius = self.config.spawn_protection;
            protection.dimension = self.effective_respawn_data.dimension.clone();
            protection.spawn = self.effective_respawn_data.pos;
            protection.ops.clone_from(&ops);
        }
    }

    fn auto_save(&mut self) {
        self.ticks_until_autosave = self.compute_next_autosave_interval();
        self.save_everything(true, false, false);
    }

    pub fn set_auto_save(&mut self, enable: bool) -> bool {
        let mut success = false;
        for level in &mut self.levels {
            if level.no_save == enable {
                level.no_save = !enable;
                success = true;
            }
        }
        success
    }

    pub fn halt(&mut self) {
        self.stopping = true;
    }

    pub fn reload_resources<S: AsRef<str>>(&mut self, packs_to_enable: &[S]) -> Result<(), String> {
        let packs_to_load = packs_to_enable.iter().filter_map(|id| self.pack_repository.pack(id.as_ref())).map(|pack| pack.open()).collect();
        let resource_manager = MultiPackResourceManager::new(packs_to_load);
        let postponed_tags = crate::tags::load_tags_for_existing_registries(&resource_manager);
        let mut resources = ReloadableServerResources::load_resources(&resource_manager, postponed_tags, self.config.function_permission_level)?;
        self.pack_repository.set_selected(packs_to_enable);
        self.data_configuration = WorldDataConfiguration::new(get_selected_packs(&self.pack_repository, true), self.data_configuration.enabled_features);
        resources.update_components_and_static_registry_tags();
        resources.finalize_recipe_loading();
        crate::world::level::levelgen::structure::templatesystem::structure_template_manager::on_resource_manager_reload(&resource_manager);
        for player in self.players() {
            self.save_player(player);
        }
        for player in self.players() {
            player.connection.send(clientbound::UPDATE_TAGS, crate::network::configuration::write_tags);
        }
        for player in self.players() {
            player.connection.send(clientbound::UPDATE_RECIPES, crate::world::item::crafting::recipe_manager::write_update_recipes);
            crate::stats::server_recipe_book::send_initial_recipe_book(&player.connection, &player.recipe_book);
        }
        self.functions.replace_library(resources.function_library);
        Ok(())
    }

    fn stop_server(&mut self) {
        self.notification_manager.server_shutting_down();
        info!("Stopping server");
        info!("Saving players");
        for player in self.players() {
            self.save_player(player);
        }
        self.remove_all();
        info!("Saving worlds");
        for level in &mut self.levels {
            level.no_save = false;
        }
        while self.levels.iter().any(|level| level.chunks.has_work()) {
            let next_tick = Instant::now() + Duration::from_millis(1);
            for level in &mut self.levels {
                level.chunks.deactivate_tickets_on_closing();
                level.tick_chunk_source_without_chunks();
            }
            self.wait_until_next_tick(next_tick);
        }
        self.save_all_chunks(false, true, false);
        for level in &mut self.levels {
            level.close();
        }
    }

    pub fn modify_clock(&mut self, clock: usize) {
        let advance_time = self.game_rules.get_bool(game_rule::ADVANCE_TIME);
        let game_time = self.overworld().game_time;
        for p in self.players() {
            p.connection.send(clientbound::SET_TIME, |w| self.clocks.write_set_time(w, game_time, &[clock], advance_time));
        }
        for level in &mut self.levels {
            level.sync_clocks(&self.clocks);
        }
    }

    pub fn set_game_rule(&mut self, rule: crisp_data::GameRule, value: i32) {
        self.game_rules.set(rule, value);
        self.notification_manager.on_game_rule_changed(rule, value);
        if rule == game_rule::REDUCED_DEBUG_INFO {
            let event = if value != 0 { 22 } else { 23 };
            for p in self.players() {
                p.connection.send(clientbound::ENTITY_EVENT, |w| {
                    w.i32(p.entity.id);
                    w.u8(event);
                });
            }
        } else if rule == game_rule::LIMITED_CRAFTING || rule == game_rule::IMMEDIATE_RESPAWN {
            let event = if rule == game_rule::LIMITED_CRAFTING { GAME_EVENT_LIMITED_CRAFTING } else { GAME_EVENT_IMMEDIATE_RESPAWN };
            for p in self.players() {
                p.connection.send(clientbound::GAME_EVENT, |w| {
                    w.u8(event);
                    w.f32(if value != 0 { 1.0 } else { 0.0 });
                });
            }
        } else if rule == game_rule::ADVANCE_TIME {
            for p in self.players() {
                p.connection.send(clientbound::SET_TIME, |w| self.full_time_sync(w));
            }
        }
    }

    fn save_player(&self, player: &ServerPlayer) {
        self.save_player_data(self.entity_level(player.entity.id).dimension, player);
    }

    fn save_player_data(&self, dimension: &str, player: &ServerPlayer) {
        let Some(dir) = &self.world_dir else { return };
        if write_dat(&dir.player(player.profile.uuid), &player.to_nbt(dimension)).is_err() {
            warn!("Failed to save player data for {}", player.profile.name);
        }
    }

    fn handle(&mut self, event: Event) {
        match event {
            Event::Join { conn, profile, cookie, out, address } => self.place_new_player(conn, profile, cookie, out, address),
            Event::Packet { conn, packet } => {
                let Some(&id) = self.connections.get(&conn) else { return };
                self.player_mut(id).received_packets += 1;
                let result = match packet.id {
                    serverbound::CHAT => self.handle_chat(id, &packet),
                    serverbound::CHAT_COMMAND => self.handle_chat_command(id, &packet),
                    serverbound::CHAT_COMMAND_SIGNED => self.handle_signed_chat_command(id, &packet),
                    serverbound::CHAT_ACK => self.handle_chat_ack(id, &packet),
                    serverbound::CHAT_SESSION_UPDATE => self.handle_chat_session_update(id, &packet),
                    serverbound::COMMAND_SUGGESTION => self.handle_custom_command_suggestions(id, &packet),
                    serverbound::CLIENT_COMMAND => self.handle_client_command(id, &packet),
                    serverbound::CHANGE_GAME_MODE => self.handle_change_game_mode(id, &packet),
                    serverbound::RESOURCE_PACK => self.handle_resource_pack_response(id, &packet),
                    _ => self.entity_level_mut(id).handle_game_packet(id, &packet),
                };
                if result.is_err() {
                    self.player_mut(id).disconnect("multiplayer.disconnect.invalid_packet");
                }
                self.entity_level_mut(id).entity_moved(id);
                if self.player(id).disconnected {
                    self.remove_player(conn, None);
                }
            }
            Event::Left { conn, reason } => self.remove_player(conn, Some(reason)),
            Event::Verify { profile, address, reply } => self.pending_logins.push(PendingLogin { profile, address, reply, waiting_for_dupe: false }),
            Event::PrepareSpawn { uuid, name, reply } => self.start_prepare_spawn(uuid, &name, reply),
            Event::PrepareSpawnClosed { uuid } => self.prepare_spawn_tasks.retain(|t| t.uuid != uuid),
            Event::Console(line) => self.console_inputs.push((line, CommandSourceStack::server(self))),
            Event::Halt => self.halt(),
            Event::Rcon { command, reply } => self.tasks.push_back(Task::Rcon { command, reply }),
        }
    }

    fn full_time_sync(&self, p: &mut crisp_protocol::Writer) {
        self.clocks.write_set_time(p, self.overworld().game_time, &self.clocks.all(), self.game_rules.get_bool(game_rule::ADVANCE_TIME));
    }

    fn place_new_player(&mut self, conn: ConnId, profile: Profile, cookie: CommonListenerCookie, out: Outbound, address: std::net::SocketAddr) {
        let old_name = self.name_to_id_cache.get_by_id(profile.uuid).map_or_else(|| profile.name.clone(), |p| p.name);
        self.name_to_id_cache.add(NameAndId { id: profile.uuid, name: profile.name.clone() });
        let mut player = ServerPlayer::new(profile, cookie.client_information, out, self.effective_respawn_data.pos);
        player.latency = cookie.latency;
        player.chat = ChatState::new(player.profile.uuid, &self.config);
        player.set_game_mode_for_player(self.calculate_game_mode_for_new_player(None), None);
        let saved = self.load_player_data(&player.profile.name, player.profile.uuid);
        let prepared = self.prepare_spawn_tasks.iter().position(|t| t.uuid == player.profile.uuid && t.reply.is_none());
        let (level_index, position, y_rot, x_rot) = match prepared {
            Some(index) => {
                let task = self.prepare_spawn_tasks.swap_remove(index);
                self.levels[task.level].wait_for_entities_around(BlockPos::containing(task.position.x, task.position.y, task.position.z).chunk(), 3);
                (task.level, task.position, task.angle.0, task.angle.1)
            }
            None => self.spawn_position(saved.as_ref()),
        };
        if let Some(nbt) = &saved {
            player.load_nbt(nbt);
            let level = &mut self.levels[level_index];
            if let Some(bed_position) = ServerPlayer::saved_sleeping_pos(nbt).filter(|&pos| level.has_chunk_at(pos)) {
                let state = crate::world::level::BlockGetter::block_state(level, bed_position);
                if let Some(kind) = crate::world::level::block::abstract_bed_block::kind(state.block()) {
                    level.set_block(bed_position, state.set(crisp_data::properties::OCCUPIED, false), crate::world::level::update_flags::UPDATE_ALL);
                    crate::world::level::block::abstract_bed_block::on_stop_sleeping(kind, level, bed_position);
                }
            }
            let (game_type, previous) = ServerPlayer::saved_game_types(nbt);
            player.set_game_mode_for_player(self.calculate_game_mode_for_new_player(game_type), previous);
        }
        player.entity.set_pos(position);
        player.entity.y_rot = y_rot;
        player.entity.x_rot = x_rot;
        let position = player.entity.position();
        info!(
            "{}[{}] logged in with entity id {} at ({}, {}, {})",
            player.profile.name,
            crate::network::server_connection_listener::loggable_address(address, self.config.log_ips),
            player.entity.id,
            crate::nbt::java_double_to_string(position.x),
            crate::nbt::java_double_to_string(position.y),
            crate::nbt::java_double_to_string(position.z)
        );
        player.address = Some(address);
        let config = &self.config;
        let level = &self.levels[level_index];
        let out = player.connection.clone();
        let id = player.entity.id;
        out.send(clientbound::LOGIN, |p| {
            p.i32(id);
            p.bool(self.hardcore);
            p.varint(self.levels.len() as i32);
            for level in &self.levels {
                p.string(level.dimension);
            }
            p.varint(config.max_players as i32);
            p.varint(config.view_distance);
            p.varint(config.simulation_distance);
            p.bool(false);
            p.bool(true);
            p.bool(false);
            self.create_common_spawn_info(p, level_index, &player);
            p.bool(config.online_mode);
            p.bool(self.enforce_secure_profile());
        });
        self.send_difficulty_update(&out);
        out.send(clientbound::PLAYER_ABILITIES, |p| player.abilities.write(p));
        out.send(clientbound::SET_HELD_SLOT, |p| p.varint(player.inventory.selected as i32));
        out.send(clientbound::UPDATE_RECIPES, crate::world::item::crafting::recipe_manager::write_update_recipes);
        let permission = self.profile_permission_level(player.profile.uuid);
        out.send(clientbound::ENTITY_EVENT, |p| {
            p.i32(id);
            p.u8(EVENT_PERMISSION_LEVEL_ALL + permission as u8);
        });
        out.send(clientbound::COMMANDS, |p| crate::commands::write_commands_packet(p, &CommandSourceStack::for_new_player(self, level_index, &player)));
        crate::stats::server_recipe_book::send_initial_recipe_book(&out, &player.recipe_book);
        level.scoreboard.borrow().update_entire_scoreboard(&out);
        let (pos, y_rot, x_rot) = (player.entity.position(), player.entity.y_rot, player.entity.x_rot);
        player.teleport(pos, y_rot, x_rot);
        if self.status.read().unwrap().is_some() && !cookie.transferred {
            out.send(clientbound::SERVER_DATA, |p| {
                text(config.motd.clone()).write(p);
                p.bool(self.status_icon.is_some());
                if let Some(icon) = &self.status_icon {
                    p.varint(icon.icon_bytes.len() as i32);
                    p.bytes(&icon.icon_bytes);
                }
            });
        }
        out.send(clientbound::PLAYER_INFO_UPDATE, |p| {
            p.u8(0xFF);
            p.varint(self.player_count() as i32);
            self.players().for_each(|o| o.write_info(p));
        });
        out.send(clientbound::PLAYER_INFO_UPDATE, |p| {
            p.u8(0xFF);
            p.varint(1);
            player.write_info(p);
        });
        out.send(clientbound::INITIALIZE_BORDER, |p| write_initialize_border(p, &level.world_border));
        out.send(clientbound::SET_TIME, |p| self.full_time_sync(p));
        out.send(clientbound::SET_DEFAULT_SPAWN_POSITION, |p| self.write_spawn(p));
        level.write_weather_for_joining(&out);
        out.send(clientbound::GAME_EVENT, |p| {
            p.u8(GAME_EVENT_LEVEL_CHUNKS_LOAD_START);
            p.f32(0.0);
        });
        self.update_joining_player_tick_rate(&out);

        self.last_status = None;
        let display_name = level.player_display_name(&player.profile.name, player.profile.uuid);
        let message = if player.profile.name.eq_ignore_ascii_case(&old_name) {
            player_joined_or_left(display_name, true)
        } else {
            Component::translatable_with("multiplayer.player.joined.renamed", vec![display_name.into(), old_name.into()]).with_color(ChatFormatting::Yellow)
        };
        self.send_system_message(&message);
        for other in self.players() {
            other.connection.send(clientbound::SYSTEM_CHAT, |p| {
                message.write(p);
                p.bool(false);
            });
            other.connection.send(clientbound::PLAYER_INFO_UPDATE, |p| {
                p.u8(0xFF);
                p.varint(1);
                player.write_info(p);
            });
        }
        self.connections.insert(conn, id);
        self.player_list.push(id);
        self.scoreboard.borrow_mut().server.add_connection(id, out.clone());
        self.levels[level_index].add_player(player);
        self.update_player_names_array();
        self.custom_boss_events.on_player_connect(&self.levels, id);
        let level = &mut self.levels[level_index];
        let others: Vec<(EntityId, u128, BlockPos)> =
            level.players.values().filter(|o| o.entity.id != id).map(|o| (o.entity.id, o.profile.uuid, o.entity.block_position())).collect();
        let player = level.player_mut(id);
        for (other, uuid, block) in others {
            player.connection.send_raw(waypoint(0, uuid, Some(block)));
            player.waypoints.insert(other, block);
        }
        level.send_active_player_effects(id);
        let player = level.player_mut(id);
        player.connection.send(clientbound::POST_EFFECTS, |p| p.varint(0));
        level.init_inventory_menu(id);
        let player = level.player_mut(id);
        if let Some(values) = player.entity.data.pack_dirty() {
            player.connection.send(clientbound::SET_ENTITY_DATA, |p| crate::world::entity::data::write_values(p, id, &values));
        }
        let food = player.food_data;
        player.connection.send(clientbound::SET_HEALTH, |p| {
            p.f32(player.health);
            p.varint(food.food_level);
            p.f32(food.saturation_level);
        });
        player.last_sent_health = player.health;
        player.last_sent_food = food.food_level;
        player.last_food_saturation_zero = food.saturation_level == 0.0;
        player.send_experience();
        player.last_sent_exp = player.experience.total;
        let joined = NameAndId { id: player.profile.uuid, name: player.profile.name.clone() };
        self.notification_manager.player_joined(&joined);
    }

    fn tick_player_list(&mut self) {
        self.send_all_player_info_in += 1;
        if self.send_all_player_info_in > 600 {
            let packet = crate::network::connection::encode(clientbound::PLAYER_INFO_UPDATE, |p| {
                p.u8(1 << 4);
                p.varint(self.player_count() as i32);
                for player in self.players() {
                    p.uuid(player.profile.uuid);
                    p.varint(player.latency);
                }
            });
            for player in self.players() {
                player.connection.send_raw(packet.clone());
            }
            self.send_all_player_info_in = 0;
        }
    }

    fn run_all_tasks(&mut self) {
        while let Some(task) = self.tasks.pop_front() {
            match task {
                Task::Chat { player, body, signature } if self.has_player(player) => self.run_chat(player, body, signature),
                Task::Command { player, command } if self.has_player(player) => self.run_unsigned_command(player, &command),
                Task::SignedCommand { player, command, time_stamp, salt, arguments, last_seen } if self.has_player(player) => {
                    self.run_signed_command(player, &command, time_stamp, salt, arguments, last_seen);
                }
                Task::Chained { player, run } if self.has_player(player) => run(self),
                Task::CommandSuggestions { player } if self.has_player(player) => self.try_process_command_suggestions(player),
                Task::Rcon { command, reply } => {
                    self.rcon_buffer.borrow_mut().clear();
                    let source = CommandSourceStack::rcon(self);
                    crate::commands::perform_prefixed_command(self, &source, &command);
                    let _ = reply.send(self.rcon_buffer.take());
                }
                _ => {}
            }
        }
    }

    fn handle_console_inputs(&mut self) {
        for (line, source) in std::mem::take(&mut self.console_inputs) {
            crate::commands::perform_prefixed_command(self, &source, &line);
        }
    }

    fn build_server_status(&mut self) {
        let mut status = serde_json::json!({
            "description": serde_json::from_str::<serde_json::Value>(&Component::literal(self.config.motd.as_str()).to_json()).unwrap(),
            "players": self.build_player_status(),
            "version": { "name": crisp_data::VERSION_NAME, "protocol": crisp_data::PROTOCOL_VERSION },
        });
        if let Some(icon) = &self.status_icon {
            status["favicon"] = serde_json::Value::String(icon.encoded());
        }
        if self.enforce_secure_profile() {
            status["enforcesSecureChat"] = serde_json::Value::Bool(true);
        }
        *self.status.write().unwrap() = Some(status.to_string().into());
    }

    fn build_player_status(&mut self) -> serde_json::Value {
        let players: Vec<&ServerPlayer> = self.levels.iter().flat_map(|level| level.players.values()).collect();
        let mut players_json = serde_json::json!({ "max": self.config.max_players, "online": players.len() });
        if self.config.hides_online_players {
            return players_json;
        }
        let sample_size = players.len().min(MAX_STATUS_PLAYER_SAMPLE);
        let offset = mth::next_int(&mut self.random, 0, (players.len() - sample_size) as i32) as usize;
        let mut sample: Vec<serde_json::Value> = players[offset..offset + sample_size]
            .iter()
            .map(|p| {
                let (uuid, name) = if p.client.allows_listing { (p.profile.uuid, p.profile.name.as_str()) } else { (0, "Anonymous Player") };
                serde_json::json!({ "id": format_uuid(uuid), "name": name })
            })
            .collect();
        for i in (2..=sample.len()).rev() {
            let swap_to = self.random.next_int_bounded(i as i32) as usize;
            sample.swap(i - 1, swap_to);
        }
        if !sample.is_empty() {
            players_json["sample"] = serde_json::Value::Array(sample);
        }
        players_json
    }

    pub fn invalidate_status(&mut self) {
        self.last_status = None;
    }

    fn tick_logins(&mut self) {
        for mut login in std::mem::take(&mut self.pending_logins) {
            if !login.waiting_for_dupe {
                if let Some(reason) = self.can_player_login(&login.profile, &login.address) {
                    let _ = login.reply.send(Verdict::Disconnect(reason));
                    continue;
                }
                login.waiting_for_dupe = self.disconnect_all_players_with_profile(login.profile.uuid);
            }
            if login.waiting_for_dupe && self.players().any(|p| p.profile.uuid == login.profile.uuid) {
                self.pending_logins.push(login);
                continue;
            }
            let session_id = *self.session_id.get_or_insert_with(|| {
                let mut bytes: [u8; 16] = rand::random();
                bytes[6] = bytes[6] & 0x0F | 0x40;
                bytes[8] = bytes[8] & 0x3F | 0x80;
                u128::from_be_bytes(bytes)
            });
            let _ = login.reply.send(Verdict::Accept { session_id });
        }
        if self.player_count() == 0 && self.pending_logins.is_empty() && self.connections.is_empty() {
            self.session_id = None;
        }
    }

    fn can_player_login(&mut self, profile: &Profile, address: &std::net::SocketAddr) -> Option<Component> {
        let key = format_uuid(profile.uuid);
        if self.bans.contains_key(&key) {
            return Some(match self.bans.get_key(&key) {
                Some(entry) => players::ban_message("multiplayer.disconnect.banned.reason", "multiplayer.disconnect.banned.expiration", &entry.ban),
                None => Component::literal("Internal server error"),
            });
        }
        if !self.is_white_listed(profile.uuid) {
            return Some(Component::translatable("multiplayer.disconnect.not_whitelisted"));
        }
        let ip = players::ip_from_address(address);
        if self.ip_bans.contains_key(&ip) {
            return Some(match self.ip_bans.get_key(&ip) {
                Some(entry) => players::ban_message("multiplayer.disconnect.banned_ip.reason", "multiplayer.disconnect.banned_ip.expiration", &entry.ban),
                None => Component::literal("Internal server error"),
            });
        }
        let bypass = self.ops.get(profile.uuid).is_some_and(|e| e.bypasses_player_limit);
        (self.player_count() >= self.config.max_players as usize && !bypass).then(|| Component::translatable("multiplayer.disconnect.server_full"))
    }

    pub fn enforce_secure_profile(&self) -> bool {
        self.config.enforce_secure_profile && self.config.online_mode && self.services.can_validate_profile_keys()
    }

    pub fn set_player_idle_timeout(&mut self, minutes: i32) {
        self.config.player_idle_timeout = minutes;
        if let Some(settings) = &mut self.settings {
            settings.update("player-idle-timeout", minutes.to_string());
        }
    }

    pub(crate) fn kick_if_idle(&mut self, id: EntityId) -> bool {
        let timeout = self.config.player_idle_timeout;
        let player = self.player_mut(id);
        if timeout > 0 && player.last_action_time.elapsed() > Duration::from_secs(timeout as u64 * 60) {
            player.disconnect("multiplayer.disconnect.idling");
            return true;
        }
        false
    }

    fn tick_second(&mut self, id: EntityId) -> bool {
        let rate_limit_packets_per_second = self.config.rate_limit_packets_per_second;
        let player = self.player_mut(id);
        let tick_count = player.connection_tick_count;
        player.connection_tick_count = tick_count.wrapping_add(1);
        if tick_count % 20 != 0 {
            return false;
        }
        let received = player.received_packets as f32;
        player.average_received_packets = received + 0.75 * (player.average_received_packets - received);
        player.received_packets = 0;
        if rate_limit_packets_per_second > 0 && player.average_received_packets > rate_limit_packets_per_second as f32 {
            warn!("Player exceeded rate-limit (sent {} packets per second)", crate::nbt::java_float_to_string(player.average_received_packets));
            player.disconnect("disconnect.exceeded_packet_rate");
            return true;
        }
        false
    }

    pub fn set_using_whitelist(&mut self, enabled: bool) {
        self.config.white_list = enabled;
        if let Some(settings) = &mut self.settings {
            settings.update("white-list", enabled.to_string());
        }
    }

    pub fn kick_unlisted_players(&mut self) {
        if self.config.enforce_whitelist && self.config.white_list {
            let unlisted: Vec<EntityId> =
                self.players().filter(|p| !self.whitelist.contains_key(&format_uuid(p.profile.uuid))).map(|p| p.entity.id).collect();
            for id in unlisted {
                self.player_mut(id).disconnect("multiplayer.disconnect.not_whitelisted");
            }
        }
    }

    pub fn is_white_listed(&self, uuid: u128) -> bool {
        !self.config.white_list || self.ops.contains(uuid) || self.whitelist.contains_key(&format_uuid(uuid))
    }

    fn disconnect_all_players_with_profile(&mut self, uuid: u128) -> bool {
        let dupes: Vec<EntityId> = self.players().filter(|p| p.profile.uuid == uuid).map(|p| p.entity.id).collect();
        for &id in &dupes {
            self.player_mut(id).disconnect("multiplayer.disconnect.duplicate_login");
        }
        !dupes.is_empty()
    }

    pub fn send_system_message(&self, message: &Component) {
        #[cfg(test)]
        self.console_log.borrow_mut().push(message.get_string());
        info!("System chat: {}", message.get_string());
    }

    pub fn broadcast_system_message(&self, message: &Component, overlay: bool) {
        self.send_system_message(message);
        for player in self.players() {
            self.send_player_system_message(player.entity.id, message, overlay);
        }
    }

    pub fn send_player_system_message(&self, id: EntityId, message: &Component, overlay: bool) {
        let Some(player) = self.get_player(id) else { return };
        if player.client.chat_visibility == ChatVisibility::Hidden && !overlay {
            return;
        }
        player.connection.send(clientbound::SYSTEM_CHAT, |p| {
            message.write(p);
            p.bool(overlay);
        });
    }

    pub fn entity_name(&self, id: EntityId) -> String {
        let Some(level) = self.level_of(id) else { return String::new() };
        let level = &self.levels[level];
        match level.players.get(&id) {
            Some(p) => p.profile.name.clone(),
            None => level.entities.get(&id).map_or_else(String::new, |e| e.name().get_string()),
        }
    }

    pub fn entity_display_name(&self, id: EntityId) -> Component {
        match self.level_of(id) {
            Some(level) => self.levels[level].display_name(id),
            None => Component::empty(),
        }
    }

    pub fn profile_permission_level(&self, uuid: u128) -> i32 {
        self.ops.get(uuid).map_or(0, |e| e.level)
    }

    pub fn is_op(&self, uuid: u128) -> bool {
        self.ops.contains(uuid)
    }

    pub fn send_player_permission_level(&self, id: EntityId) {
        let Some(player) = self.get_player(id) else { return };
        let level = self.profile_permission_level(player.profile.uuid);
        player.connection.send(clientbound::ENTITY_EVENT, |p| {
            p.i32(id);
            p.u8(EVENT_PERMISSION_LEVEL_ALL + level as u8);
        });
        let source = CommandSourceStack::player(self, id);
        player.connection.send(clientbound::COMMANDS, |p| crate::commands::write_commands_packet(p, &source));
    }

    fn update_player_names_array(&self) {
        let names: Arc<[String]> = self.players().map(|p| p.profile.name.clone()).collect();
        *self.player_names_array.write().unwrap() = names;
    }

    fn remove_all(&mut self) {
        let mut i = 0;
        while let Some(&id) = self.player_list.get(i) {
            self.player_mut(id).disconnect("multiplayer.disconnect.server_shutdown");
            if let Some(conn) = self.connections.iter().find_map(|(&conn, &player)| (player == id).then_some(conn)) {
                self.remove_player(conn, None);
            }
            i += 1;
        }
    }

    fn remove_player(&mut self, conn: ConnId, reason: Option<Component>) {
        let Some(id) = self.connections.remove(&conn) else { return };
        let Some(level_index) = self.level_of(id) else { return };
        let level = &mut self.levels[level_index];
        if level.is_sleeping(id) {
            level.stop_sleep_in_bed(id, true, false);
        }
        level.player_mut(id).disconnected = true;
        level.remove_player_menus(id);
        let Some(player) = level.remove_player(id) else { return };
        self.player_list.retain(|&p| p != id);
        self.scoreboard.borrow_mut().server.remove_connection(id);
        self.update_player_names_array();
        self.custom_boss_events.on_player_disconnect(&self.levels, id);
        let reason = player.disconnect_reason.clone().or_else(|| reason.map(|r| r.get_string())).unwrap_or_else(|| "Disconnected".into());
        info!("{} lost connection: {reason}", player.profile.name);
        self.invalidate_status();
        self.save_player_data(self.levels[level_index].dimension, &player);
        self.notification_manager.player_left(&NameAndId { id: player.profile.uuid, name: player.profile.name.clone() });
        let message = player_joined_or_left(self.levels[level_index].player_display_name(&player.profile.name, player.profile.uuid), false);
        self.send_system_message(&message);
        for other in self.players() {
            other.connection.send(clientbound::SYSTEM_CHAT, |p| {
                message.write(p);
                p.bool(false);
            });
        }
        for other in self.levels[level_index].players.values_mut() {
            if other.waypoints.remove(&id).is_some() {
                other.connection.send_raw(waypoint(1, player.profile.uuid, None));
            }
        }
        for other in self.players() {
            other.connection.send(clientbound::PLAYER_INFO_REMOVE, |p| {
                p.varint(1);
                p.uuid(player.profile.uuid);
            });
        }
    }

    fn tick(&mut self) {
        let start = Instant::now();
        let empty_tick_threshold = self.config.pause_when_empty_seconds * 20;
        if empty_tick_threshold > 0 {
            if self.player_count() == 0 && !self.tick_rate_manager.is_sprinting() {
                self.empty_ticks += 1;
            } else {
                self.empty_ticks = 0;
            }
            if self.empty_ticks >= empty_tick_threshold {
                if self.empty_ticks == empty_tick_threshold {
                    info!("Server empty for {} seconds, pausing", self.config.pause_when_empty_seconds);
                    self.auto_save();
                }
                self.tick_logins();
                self.tick_prepare_spawns();
                self.handle_console_inputs();
                return;
            }
        }
        self.tick_count += 1;
        self.tick_rate_manager.tick();
        ServerFunctionManager::tick(self);
        if self.tick_rate_manager.runs_normally() {
            self.clocks.tick(self.game_rules.get_bool(game_rule::ADVANCE_TIME));
        }
        let overworld_clock_time = self.clocks.clocks[ServerClockManager::id_of(OVERWORLD).unwrap()].total_ticks;
        for level in &mut self.levels {
            level.sync_clocks(&self.clocks);
            level.overworld_clock_time = overworld_clock_time;
        }
        if self.tick_count % 20 == 0 {
            let game_time = self.overworld().game_time;
            for p in self.players() {
                p.connection.send(clientbound::SET_TIME, |w| {
                    w.i64(game_time);
                    w.varint(0);
                });
            }
        }
        self.update_effective_respawn_data();
        for index in 0..self.levels.len() {
            self.tick_level(index);
        }
        for id in self.player_ids() {
            self.entity_level_mut(id).tick_connection_start(id);
            if let Some(level) = self.level_of(id) {
                self.handle_pending_portal_teleport(level);
            }
            let connected = self.entity_level_mut(id).tick_connection_end(id);
            if connected {
                self.tick_command_suggestions(id);
            }
            if !connected || self.kick_if_idle(id) || self.tick_second(id) {
                let conn = self.connections.iter().find(|&(_, &p)| p == id).map(|(&c, _)| c);
                if let Some(conn) = conn {
                    self.remove_player(conn, None);
                }
            }
        }
        self.tick_logins();
        self.tick_prepare_spawns();
        self.handle_console_inputs();
        self.tick_player_list();
        self.update_waypoints();
        for id in self.player_ids() {
            self.entity_level_mut(id).send_next_chunks(id);
        }
        self.server_activity_monitor.tick();
        if self.last_status.is_none_or(|t| t.elapsed() >= STATUS_EXPIRE_TIME) {
            self.last_status = Some(Instant::now());
            self.build_server_status();
        }
        self.ticks_until_autosave -= 1;
        if self.ticks_until_autosave <= 0 {
            self.auto_save();
        }
        let tick_time = start.elapsed().as_nanos() as i64;
        let tick_index = (self.tick_count % 100) as usize;
        self.aggregated_tick_times_nanos += tick_time - self.tick_times_nanos[tick_index];
        self.tick_times_nanos[tick_index] = tick_time;
    }

    fn tick_level(&mut self, index: usize) {
        if index != 0 {
            let game_time = self.overworld().game_time;
            let level = &mut self.levels[index];
            level.game_time = game_time;
            level.chunks.game_time = game_time;
        }
        let level = &mut self.levels[index];
        let runs = level.tick_until_sleep();
        if level.should_wake_up_all_players() {
            let default_clock = level.dimension_type().default_clock.and_then(ServerClockManager::id_of);
            if self.game_rules.get_bool(game_rule::ADVANCE_TIME)
                && let Some(clock) = default_clock
            {
                self.clocks.move_to_time_marker(clock, "minecraft:wake_up_from_sleep");
                self.modify_clock(clock);
            }
            self.levels[index].wake_up_all_players_after_clock();
        }
        let level = &mut self.levels[index];
        level.update_sky_brightness();
        if runs && index == 0 {
            let time = level.tick_time();
            TimerQueue::tick(self, time);
        }
        self.levels[index].tick_after_time(runs);
        while self.levels[index].tick_entities() {
            self.handle_pending_portal_teleport(index);
        }
        self.levels[index].tick_after_entities();
    }

    pub fn tick_times_nanos(&self) -> &[i64; 100] {
        &self.tick_times_nanos
    }

    fn update_waypoints(&mut self) {
        for level in &mut self.levels {
            let positions: Vec<(EntityId, u128, BlockPos)> =
                level.players.values().map(|p| (p.entity.id, p.profile.uuid, p.entity.block_position())).collect();
            for viewer in level.players.values_mut() {
                for &(id, uuid, block) in &positions {
                    if id == viewer.entity.id {
                        continue;
                    }
                    match viewer.waypoints.get(&id) {
                        None => viewer.connection.send_raw(waypoint(0, uuid, Some(block))),
                        Some(&old) if old != block => viewer.connection.send_raw(waypoint(2, uuid, Some(block))),
                        Some(_) => continue,
                    }
                    viewer.waypoints.insert(id, block);
                }
            }
        }
    }
}

pub fn waypoint(op: i32, uuid: u128, block: Option<BlockPos>) -> Vec<u8> {
    crate::network::connection::encode(clientbound::WAYPOINT, |p| {
        p.varint(op);
        p.bool(true);
        p.uuid(uuid);
        p.string("minecraft:default");
        p.bool(false);
        match block {
            Some(b) => {
                p.varint(1);
                p.varint(b.x);
                p.varint(b.y);
                p.varint(b.z);
            }
            None => p.varint(0),
        }
    })
}

pub fn hashed_seed(seed: i64) -> i64 {
    let hash = Sha256::digest(seed.to_le_bytes());
    i64::from_le_bytes(hash[..8].try_into().unwrap())
}

impl Server {
    fn load_status_icon(&self) -> Option<Favicon> {
        let is_regular_file = |path: &PathBuf| std::fs::metadata(path).is_ok_and(|m| m.is_file());
        let icon_path = self
            .config
            .server_dir
            .as_ref()
            .map(|dir| dir.join("server-icon.png"))
            .filter(is_regular_file)
            .or_else(|| self.world_dir.as_ref().map(WorldDir::icon_file).filter(is_regular_file))?;
        let loaded = std::fs::read(&icon_path).map_err(|e| format!("java.io.IOException: {e}")).and_then(|contents| {
            let png_info = crate::util::png_info::PngInfo::from_bytes(&contents)?;
            if png_info.width == 64 && png_info.height == 64 {
                Ok(Favicon { icon_bytes: contents })
            } else {
                Err(format!(
                    "java.lang.IllegalArgumentException: Invalid world icon size [{}, {}], but expected [64, 64]",
                    png_info.width, png_info.height
                ))
            }
        });
        loaded.inspect_err(|e| error!("Couldn't load server icon\n{e}")).ok()
    }
}

fn write_respawn_data(p: &mut crisp_protocol::Writer, data: &RespawnData) {
    p.string(&data.dimension);
    p.position(data.pos.x, data.pos.y, data.pos.z);
    p.f32(data.yaw);
    p.f32(data.pitch);
}

pub(crate) fn log_save_failure(file_name: &str, result: std::io::Result<()>) {
    if result.is_err() {
        error!("Could not save data to {file_name}");
    }
}

pub fn configure_pack_repository(
    pack_repository: &mut PackRepository,
    initial_data_config: &WorldDataConfiguration,
    init_mode: bool,
    safe_mode: bool,
) -> WorldDataConfiguration {
    let data_pack_config = &initial_data_config.data_packs;
    let forced_features = if init_mode { FeatureFlagSet::default() } else { initial_data_config.enabled_features };
    let allowed_features = if init_mode { FeatureFlagSet::ALL_FLAGS } else { initial_data_config.enabled_features };
    pack_repository.reload();
    if safe_mode {
        return configure_repository_with_selection(pack_repository, &[packs::repository::VANILLA_ID.to_string()], forced_features, false);
    }
    let mut selected: Vec<String> = Vec::new();
    for id in &data_pack_config.enabled {
        if pack_repository.is_available(id) {
            if !selected.contains(id) {
                selected.push(id.clone());
            }
        } else {
            warn!("Missing data pack {id}");
        }
    }
    for pack in pack_repository.available_packs() {
        let pack_id = pack.id();
        if data_pack_config.disabled.iter().any(|d| d == pack_id) {
            continue;
        }
        let pack_features = pack.requested_features();
        let is_selected = selected.iter().any(|s| s == pack_id);
        if !is_selected && pack.pack_source().should_add_automatically() {
            if pack_features.is_subset_of(allowed_features) {
                info!("Found new data pack {pack_id}, loading it automatically");
                selected.push(pack_id.to_string());
            } else {
                info!(
                    "Found new data pack {pack_id}, but can't load it due to missing features {}",
                    print_missing_flags(allowed_features, pack_features)
                );
            }
        }
        if is_selected && !pack_features.is_subset_of(allowed_features) {
            warn!(
                "Pack {pack_id} requires features {} that are not enabled for this world, disabling pack.",
                print_missing_flags(allowed_features, pack_features)
            );
            selected.retain(|s| s != pack_id);
        }
    }
    if selected.is_empty() {
        info!("No datapacks selected, forcing vanilla");
        selected.push(packs::repository::VANILLA_ID.to_string());
    }
    configure_repository_with_selection(pack_repository, &selected, forced_features, true)
}

fn configure_repository_with_selection(
    pack_repository: &mut PackRepository,
    selected: &[String],
    forced_features: FeatureFlagSet,
    disable_inactive: bool,
) -> WorldDataConfiguration {
    pack_repository.set_selected(selected);
    enable_forced_feature_packs(pack_repository, forced_features);
    let pack_config = get_selected_packs(pack_repository, disable_inactive);
    let pack_requested_features = pack_repository.requested_feature_flags().join(forced_features);
    WorldDataConfiguration::new(pack_config, pack_requested_features)
}

fn enable_forced_feature_packs(pack_repository: &mut PackRepository, forced_features: FeatureFlagSet) {
    let provided_features = pack_repository.requested_feature_flags();
    let mut missing_features = forced_features.subtract(provided_features);
    if missing_features.is_empty() {
        return;
    }
    let mut selected: Vec<String> = pack_repository.selected_ids().into_iter().map(str::to_string).collect();
    for pack in pack_repository.available_packs() {
        if missing_features.is_empty() {
            break;
        }
        if pack.pack_source() == PackSource::Feature {
            let pack_features = pack.requested_features();
            if !pack_features.is_empty() && pack_features.intersects(missing_features) && pack_features.is_subset_of(forced_features) {
                if selected.iter().any(|s| s == pack.id()) {
                    panic!("Tried to force '{}', but it was already enabled", pack.id());
                }
                selected.push(pack.id().to_string());
                info!("Found feature pack ('{}') for requested feature, forcing to enabled", pack.id());
                missing_features = missing_features.subtract(pack_features);
            }
        }
    }
    pack_repository.set_selected(&selected);
}

pub fn get_selected_packs(pack_repository: &PackRepository, disable_inactive: bool) -> DataPackConfig {
    let selected = pack_repository.selected_ids();
    let disabled = if disable_inactive {
        pack_repository.available_ids().filter(|id| !selected.contains(id)).map(str::to_string).collect()
    } else {
        Vec::new()
    };
    DataPackConfig { enabled: selected.into_iter().map(str::to_string).collect(), disabled }
}

#[allow(clippy::too_many_arguments)]
pub fn run(
    config: ServerConfig,
    settings: Option<dedicated::dedicated_server_settings::DedicatedServerSettings>,
    world: Option<(PackRepository, WorldStem)>,
    services: Services,
    mut events: mpsc::UnboundedReceiver<Event>,
    sender: mpsc::UnboundedSender<Event>,
    runtime: tokio::runtime::Handle,
    listen: crate::Listen,
    console: bool,
    management_server: Option<Arc<ManagementServer>>,
) -> anyhow::Result<()> {
    let (task_sender, tasks) = std::sync::mpsc::channel::<ServerTask>();
    if let Some(management_server) = &management_server {
        management_server.set_server(task_sender);
    }
    let notification_manager = NotificationManager::new(management_server.clone());
    let server_activity_monitor = Arc::new(ServerActivityMonitor::new(notification_manager.clone(), 30));
    let listener_settings = Arc::new(ListenerSettings::new(config.replies_to_status, config.accepts_transfers));
    if console {
        dedicated::start_console_thread(sender.clone());
    }
    info!("Starting minecraft server version {}", crisp_data::VERSION_NAME);
    info!("Loading properties");
    info!("Default game type: {}", config.game_mode.name().to_uppercase());
    info!("Generating keypair");
    let keys = Arc::new(crate::network::encryption::ServerKeyPair::generate()?);
    let listener = match listen {
        crate::Listen::Bind { ip, port } => {
            info!("Starting Minecraft server on {}:{port}", if ip.is_empty() { "*" } else { &ip });
            let address = if ip.is_empty() { format!("0.0.0.0:{port}") } else { format!("{ip}:{port}") };
            match runtime.block_on(tokio::net::TcpListener::bind(&address)) {
                Ok(listener) => listener,
                Err(e) => {
                    warn!("**** FAILED TO BIND TO PORT!");
                    warn!("The exception was: {}", dedicated::java_bind_exception(&e));
                    warn!("Perhaps a server is already running on that port?");
                    return Ok(());
                }
            }
        }
        crate::Listen::Bound(listener) => {
            let local = listener.local_addr()?;
            info!("Starting Minecraft server on {}:{}", local.ip(), local.port());
            listener
        }
    };
    if !config.online_mode {
        warn!("**** SERVER IS RUNNING IN OFFLINE/INSECURE MODE!");
        warn!("The server will make no attempt to authenticate usernames. Beware.");
        warn!("While this makes the game possible to play without internet access, it also opens up the ability for hackers to connect with any username they choose.");
        warn!("To change this, set \"online-mode\" to \"true\" in the server.properties file.");
    }
    let port = listener.local_addr()?.port();
    let status = Arc::new(RwLock::new(None));
    let player_names_array: Arc<RwLock<Arc<[String]>>> = Arc::default();
    let context = Arc::new(crate::network::server_connection_listener::ListenerContext {
        config: config.clone(),
        keys,
        services: services.clone(),
        status: status.clone(),
        player_names_array: player_names_array.clone(),
        events: sender.clone(),
        settings: listener_settings.clone(),
        server_activity_monitor: server_activity_monitor.clone(),
    });
    runtime.spawn(crate::network::server_connection_listener::accept_connections(listener, context));
    let started = Instant::now();
    info!("Preparing level \"{}\"", config.level_name);
    let mut server = match world {
        Some((pack_repository, world_stem)) => Server::with_world_stem(config, pack_repository, world_stem)?,
        None => Server::new(config)?,
    };
    server.attach_notification_manager(notification_manager, server_activity_monitor, listener_settings);
    if let Some(management_server) = &management_server {
        management_server.set_waker(server.overworld().chunks.waker());
    }
    server.settings = settings;
    info!("Done ({:.3}s)! For help, type \"help\"", started.elapsed().as_secs_f64());
    server.services = services;
    server.status = status;
    server.player_names_array = player_names_array;
    let mut query_thread_gs4 = None;
    let mut rcon_thread = None;
    if let Some(properties) = server.settings.as_ref().map(|s| s.properties().clone()) {
        if properties.enable_query {
            info!("Starting GS4 status listener");
            let interface = rcon::query_thread_gs4::QueryServerInterface {
                server_ip: properties.server_ip.clone(),
                server_port: port,
                server_name: server.config.motd.clone(),
                max_players: server.config.max_players,
                level_id_name: server.config.level_name.clone(),
                player_names_array: server.player_names_array.clone(),
            };
            query_thread_gs4 = rcon::query_thread_gs4::QueryThreadHandle::create(interface, properties.query_port, &runtime);
        }
        if properties.enable_rcon {
            info!("Starting remote control listener");
            rcon_thread = rcon::rcon_thread::RconThread::create(&properties.server_ip, properties.rcon_port, &properties.rcon_password, sender.clone(), &runtime);
        }
    }
    let timing = dedicated::server_watchdog::ServerTiming::new();
    timing.set_next_tick_time(Instant::now(), server.tick_rate_manager.milliseconds_per_tick());
    if let Ok(mut snapshot) = timing.snapshot.lock() {
        *snapshot = server.watchdog_snapshot();
    }
    let max_tick_time = server.settings.as_ref().map_or(0, |s| s.properties().max_tick_time);
    if max_tick_time > 0 {
        dedicated::server_watchdog::start(timing.clone(), max_tick_time, PathBuf::from("."));
    }
    server.save_everything(false, true, true);
    server.notification_manager.server_started();
    server.status_icon = server.load_status_icon();
    server.build_server_status();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run_server_loop(&mut server, &mut events, &tasks, &timing)));
    if let Err(panic) = result {
        let message = panic.downcast_ref::<String>().cloned().or_else(|| panic.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_default();
        error!("Encountered an unexpected exception\n{message}");
        let mut report = crate::crash_report::CrashReport::new("Exception in server tick loop", message, crash_frames());
        dedicated::server_watchdog::fill_system_report(&mut report, &server.watchdog_snapshot());
        let file = crate::crash_report::crash_report_file(Path::new("."));
        if report.save_to_file(&file) {
            error!("This crash report has been saved to: {}", std::path::absolute(&file).unwrap_or(file).display());
        } else {
            error!("We were unable to save this crash report to disk.");
        }
    }
    timing.stop();
    if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| server.stop_server())).is_err() {
        error!("Exception stopping the server");
    }
    if let Some(rcon_thread) = rcon_thread {
        rcon_thread.stop();
    }
    if let Some(query_thread_gs4) = query_thread_gs4 {
        query_thread_gs4.stop();
    }
    if let Some(management_server) = management_server {
        management_server.stop();
    }
    std::thread::sleep(Duration::from_millis(100));
    Ok(())
}

fn crash_frames() -> Vec<String> {
    std::backtrace::Backtrace::force_capture()
        .to_string()
        .lines()
        .filter_map(|line| line.trim().split_once(": ").map(|(_, frame)| frame.to_string()))
        .collect()
}

fn run_server_loop(server: &mut Server, events: &mut mpsc::UnboundedReceiver<Event>, tasks: &std::sync::mpsc::Receiver<ServerTask>, timing: &dedicated::server_watchdog::ServerTiming) {
    const OVERLOADED_THRESHOLD: Duration = Duration::from_secs(1);
    const OVERLOADED_WARNING_INTERVAL: Duration = Duration::from_secs(10);
    const SNAPSHOT_INTERVAL: u64 = 20;
    let mut next_tick = Instant::now();
    let mut last_overload_warning = next_tick;
    let mut ticks: u64 = 0;
    while !server.stopping {
        let this_tick = if server.tick_rate_manager.is_sprinting() && server.check_should_sprint_this_tick() {
            next_tick = Instant::now();
            last_overload_warning = next_tick;
            Duration::ZERO
        } else {
            let this_tick = Duration::from_nanos(server.tick_rate_manager.nanoseconds_per_tick() as u64);
            if let Some(behind) = Instant::now().checked_duration_since(next_tick)
                && behind > OVERLOADED_THRESHOLD + 20 * this_tick
                && next_tick.saturating_duration_since(last_overload_warning) >= OVERLOADED_WARNING_INTERVAL + 100 * this_tick
            {
                let ticks = (behind.as_nanos() / this_tick.as_nanos()) as u32;
                warn!("Can't keep up! Is the server overloaded? Running {}ms or {ticks} ticks behind", behind.as_millis());
                next_tick += this_tick * ticks;
                last_overload_warning = next_tick;
            }
            this_tick
        };
        let sprinting = this_tick.is_zero();
        next_tick += this_tick;
        timing.set_next_tick_time(next_tick, server.tick_rate_manager.milliseconds_per_tick());
        if ticks.is_multiple_of(SNAPSHOT_INTERVAL)
            && let Ok(mut snapshot) = timing.snapshot.lock()
        {
            *snapshot = server.watchdog_snapshot();
        }
        ticks += 1;
        for level in &mut server.levels {
            level.chunks.tick_deadline = Some(next_tick);
        }
        server.refresh_spawn_protection();
        while let Ok(event) = events.try_recv() {
            server.handle(event);
        }
        for level in &mut server.levels {
            level.chunks.run_light_updates();
            level.process_chunk_events();
        }
        server.tick();
        server.run_all_tasks();
        server.mark_ready();
        let wakeup = server.overworld().chunks.main_thread_wakeup();
        loop {
            let seen = wakeup.generation();
            while let Ok(task) = tasks.try_recv() {
                task(server);
            }
            let Some(wait) = next_tick.checked_duration_since(Instant::now()).filter(|w| !w.is_zero()) else { break };
            if !server.poll_task() {
                wakeup.wait(seen, wait);
            }
        }
        if sprinting {
            server.tick_rate_manager.end_tick_work();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::level::BlockGetter;

    #[test]
    fn normal_world_spawns_on_terrain_and_reloads() {
        let dir = std::env::temp_dir().join(format!("crisp-normal-world-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let config = ServerConfig { seed: Some(12345), world: Some(dir.clone()), ..ServerConfig::default() };
        let mut server = Server::new(config.clone()).unwrap();
        let spawn = server.spawn.pos;
        assert!(((spawn.x >> 4) - 6).abs() <= 5 && ((spawn.z >> 4) + 2).abs() <= 5, "{spawn:?}");
        assert!(crisp_core::shapes::is_face_full(server.levels[0].chunks.block_state(spawn.below()).collision_voxel_shape(), crisp_core::Direction::Up), "{spawn:?}");
        assert!(server.save_everything(true, true, true));
        drop(server);
        let server = Server::new(config).unwrap();
        assert_eq!(server.spawn.pos, spawn);
        assert!(matches!(&server.levels[0].chunks.generator, ChunkGenerator::Noise(g) if g.seed() == 12345));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn hashed_seed_matches_vanilla() {
        assert_eq!(super::hashed_seed(-8153440939324367893), 0x441855f4320ff0d7);
    }
}
