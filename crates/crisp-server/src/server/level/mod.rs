pub mod block_entities;
pub mod chunk_cache;
pub mod chunk_holder;
pub mod chunk_level;
pub mod chunk_map;
pub mod chunk_task_dispatcher;
pub mod chunk_tracker;
pub mod chunk_workers;
pub mod distance_manager;
pub mod events;
pub mod game_mode;
pub mod server_player;
pub mod player_spawn_finder;
pub mod progress;
mod precipitation;
pub mod server_boss_event;
pub mod server_entity;
pub mod server_entity_getter;
pub mod threaded_level_light_engine;
pub mod ticket;
pub mod world_gen_region;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::util::FastIndexMap;

use crisp_core::random::{LegacyRandomSource, RandomSource};
use crisp_core::shapes::{BooleanOp, join_is_not_empty};
use crisp_core::{AABB, BlockPos, ChunkPos, Direction, Vec3, mth};
use crisp_data::packets::play::clientbound;
use crisp_data::{Block, BlockState, DimensionType, Fluid, blocks};

use crate::advancements::triggers::criteria_triggers;
use crate::network::connection::encode;
use crate::world::attribute::environment_attribute_system::{EnvironmentAccess, EnvironmentAttributeReader};
use crate::world::attribute::EnvironmentAttributeSystem;
use crate::world::clock::ServerClockManager;
use crate::world::entity::{Entity, EntityId, ItemEntity, LevelEntity};
use crate::world::item::ItemStack;
use crate::world::level::BlockGetter;
use crate::world::level::block;
use crate::world::level::block::entity::BlockEntity;
use crate::world::level::border::world_border::{Settings, WorldBorder};
use crate::world::level::chunk::block_entities::EntityCreationType;
use crate::world::level::chunk::storage::RegionStorage;
use crate::world::level::entity::PersistentEntitySectionManager;
use crate::world::level::entity::entity_tick_list::EntityTickList;
use crate::world::level::gamerules::GameRules;
use crate::world::level::pathfinder::path_type_cache::PathTypeCache;
use crate::world::level::saveddata::weather_data::WeatherData;
use crate::world::level::saveddata::maps::map_id::MapId;
use crate::world::level::saveddata::maps::map_item_saved_data::MapItemSavedData;
use crate::world::level::storage::saved_data_storage::SavedDataStorage;
use crate::world::level::storage::{WorldDir, read_dat, write_dat};
use crisp_protocol::nbt::Nbt;
use crate::world::tick_rate_manager::TickRateManager;
use crate::world::level::update_flags::*;
use crate::server::waypoints::server_waypoint_manager::ServerWaypointManager;
use crate::world::scores::scoreboard::Scoreboard;
use crate::world::ticks::{ScheduledTick, TickPriority};
use chunk_cache::ChunkCache;
use chunk_map::{ChunkMap, TrackingConfig};
use server_player::ServerPlayer;
use server_entity::UpdateInterval;

#[derive(Clone, Debug, Default)]
pub struct SpawnProtection {
    pub radius: i32,
    pub dimension: String,
    pub spawn: BlockPos,
    pub ops: Vec<u128>,
}

pub const RAIN_DELAY: (i32, i32) = (12000, 180000);
pub const RAIN_DURATION: (i32, i32) = (12000, 24000);
pub const THUNDER_DELAY: (i32, i32) = (12000, 180000);
pub const THUNDER_DURATION: (i32, i32) = (3600, 15600);

const GAME_EVENT_START_RAINING: u8 = 1;
const GAME_EVENT_STOP_RAINING: u8 = 2;
const GAME_EVENT_RAIN_LEVEL_CHANGE: u8 = 7;
const GAME_EVENT_THUNDER_LEVEL_CHANGE: u8 = 8;

const UPDATE_ORDER: [Direction; 6] = [Direction::West, Direction::East, Direction::Down, Direction::Up, Direction::North, Direction::South];

pub const UPDATE_SHAPE_ORDER: [Direction; 6] = [Direction::West, Direction::East, Direction::North, Direction::South, Direction::Down, Direction::Up];


enum NeighborUpdates {
    Shape { direction: Direction, neighbor_state: BlockState, pos: BlockPos, neighbor_pos: BlockPos, flags: i32, update_limit: i32 },
    Simple { pos: BlockPos, block: Block },
    Full { state: BlockState, pos: BlockPos, block: Block, moved_by_piston: bool },
    Multi { source_pos: BlockPos, source_block: Block, skip: Option<Direction>, idx: usize },
}

enum NeighborStep {
    Shape { direction: Direction, neighbor_state: BlockState, pos: BlockPos, neighbor_pos: BlockPos, flags: i32, update_limit: i32 },
    Changed { state: Option<BlockState>, pos: BlockPos, block: Block, moved_by_piston: bool },
}

struct NeighborUpdater {
    max_chained_neighbor_updates: i32,
    stack: Vec<NeighborUpdates>,
    added_this_layer: Vec<NeighborUpdates>,
    count: i32,
}

pub struct ServerLevel {
    pub level_name: String,
    pub seed: i64,
    pub command_storage: Rc<RefCell<crate::world::level::storage::command_storage::CommandStorage>>,
    pub random_sequences: Rc<RefCell<crate::world::random_sequences::RandomSequences>>,
    pub server_data_storage: Rc<RefCell<SavedDataStorage>>,
    pub game_rules: GameRules,
    pub spawn_protection: SpawnProtection,
    pub difficulty: crate::world::difficulty::Difficulty,
    pub no_save: bool,
    pub allow_flight: bool,
    pub entity_broadcast_range_percentage: i32,
    pub weather: Rc<Cell<WeatherData>>,
    pub scoreboard: Rc<RefCell<Scoreboard>>,
    pub world_border: WorldBorder,
    pub raids: crate::world::entity::raid::raids::Raids,
    pub pending_portal_teleport: Option<crate::world::level::block::portal::PendingPortalTeleport>,
    pub global_level_events: Vec<(i32, BlockPos, i32)>,
    pub pending_system_broadcasts: Vec<crate::network::chat::Component>,
    pub pending_function_executions: Vec<(String, EntityId)>,
    entity_tick_range: Option<(usize, usize)>,
    pub rain_level: f32,
    pub o_rain_level: f32,
    pub thunder_level: f32,
    pub o_thunder_level: f32,
    pub dimension: &'static str,
    pub index: usize,
    pub dimension_type_id: i32,
    pub chunks: ChunkCache,
    pub structure_check: crate::world::level::levelgen::structure::structure_check::StructureCheck,
    pub chunk_map: ChunkMap,
    pub players: FastIndexMap<EntityId, ServerPlayer>,
    pub entities: FastIndexMap<EntityId, LevelEntity>,
    pub entity_manager: PersistentEntitySectionManager,
    pub(crate) tick_list: EntityTickList,
    waypoint_manager: RefCell<ServerWaypointManager>,
    pub random: LegacyRandomSource,
    pub sleep_status: crate::server::players::sleep_status::SleepStatus,
    pub thread_safe_random: LegacyRandomSource,
    neighbor_updater: NeighborUpdater,
    sub_tick_count: i64,
    rand_value: i32,
    pub biome_zoom_seed: i64,
    pub overworld_clock_time: i64,
    pub simulation_distance: i32,
    empty_time: i32,
    pub tick_rate_manager: Rc<TickRateManager>,
    block_events: Vec<(BlockPos, Block, i32, i32)>,
    pub game_time: i64,
    pub redstone_torch_toggles: Vec<(BlockPos, i64)>,
    environment_attributes: EnvironmentAttributeSystem,
    pub clock_ticks: Vec<i64>,
    pub(crate) sky_darken: i32,
    pub path_types_by_pos_cache: std::cell::RefCell<PathTypeCache>,
}

impl EnvironmentAccess for ServerLevel {
    fn biome(&self, x: i32, y: i32, z: i32) -> crate::world::level::chunk::Biome {
        self.get_biome(BlockPos::new(x, y, z))
    }

    fn noise_biome(&self, quart_x: i32, quart_y: i32, quart_z: i32) -> crate::world::level::chunk::Biome {
        self.get_noise_biome(quart_x, quart_y, quart_z)
    }

    fn clock_total_ticks(&self, clock: usize) -> i64 {
        self.clock_ticks[clock]
    }

    fn rain_level(&self) -> f32 {
        self.get_rain_level(1.0)
    }

    fn thunder_level(&self) -> f32 {
        self.get_thunder_level(1.0)
    }
}

impl ServerLevel {
    pub fn new(
        dimension: &'static str,
        dimension_type_id: i32,
        mut chunks: ChunkCache,
        entity_storage: Option<RegionStorage>,
        view_distance: i32,
        simulation_distance: i32,
    ) -> ServerLevel {
        chunks.set_view_distance(view_distance);
        chunks.set_simulation_distance(simulation_distance);
        let structure_check = crate::world::level::levelgen::structure::structure_check::StructureCheck::new(chunks.chunk_scan_access(), chunks.generator.clone(), chunks.dimension, chunks.seed);
        let mut level = ServerLevel {
            level_name: String::new(),
            seed: 0,
            command_storage: Rc::new(RefCell::new(crate::world::level::storage::command_storage::CommandStorage::new(None))),
            random_sequences: Rc::new(RefCell::new(crate::world::random_sequences::RandomSequences::load(None))),
            server_data_storage: Rc::new(RefCell::new(SavedDataStorage::new(None))),
            game_rules: GameRules::default(),
            difficulty: crate::world::difficulty::Difficulty::Easy,
            no_save: false,
            allow_flight: false,
            entity_broadcast_range_percentage: 100,
            spawn_protection: SpawnProtection::default(),
            weather: Rc::default(),
            rain_level: 0.0,
            o_rain_level: 0.0,
            thunder_level: 0.0,
            o_thunder_level: 0.0,
            dimension,
            index: 0,
            dimension_type_id,
            chunks,
            structure_check,
            chunk_map: ChunkMap::new(view_distance),
            players: FastIndexMap::default(),
            entities: FastIndexMap::default(),
            entity_manager: PersistentEntitySectionManager::new(entity_storage),
            tick_list: EntityTickList::default(),
            waypoint_manager: RefCell::default(),
            random: LegacyRandomSource::create(),
            sleep_status: Default::default(),
            thread_safe_random: LegacyRandomSource::create(),
            neighbor_updater: NeighborUpdater { max_chained_neighbor_updates: 1_000_000, stack: Vec::new(), added_this_layer: Vec::new(), count: 0 },
            sub_tick_count: 0,
            rand_value: LegacyRandomSource::create().next_int(),
            biome_zoom_seed: 0,
            overworld_clock_time: 0,
            simulation_distance,
            empty_time: 0,
            tick_rate_manager: Rc::default(),
            block_events: Vec::new(),
            game_time: 0,
            redstone_torch_toggles: Vec::new(),
            scoreboard: Rc::default(),
            world_border: WorldBorder::new(Settings::DEFAULT, None),
            raids: crate::world::entity::raid::raids::Raids::load(None),
            pending_portal_teleport: None,
            global_level_events: Vec::new(),
            pending_system_broadcasts: Vec::new(),
            pending_function_executions: Vec::new(),
            entity_tick_range: None,
            environment_attributes: EnvironmentAttributeSystem::default(),
            clock_ticks: ServerClockManager::new().clocks.iter().map(|c| c.total_ticks).collect(),
            sky_darken: 0,
            path_types_by_pos_cache: std::cell::RefCell::default(),
        };
        level.environment_attributes = EnvironmentAttributeSystem::builder().add_default_layers(level.dimension_type(), level.can_have_weather()).build();
        level.update_sky_brightness();
        level
    }

    pub fn get_watchdog_stats(&self) -> String {
        let entity_type_name = |entity_type: i32| crisp_data::registry::ENTITY_TYPE.get(entity_type as usize).copied().unwrap_or("");
        let entity_types = get_type_count(
            self.players.values().map(|p| entity_type_name(p.entity.entity_type)).chain(self.entities.values().map(|e| entity_type_name(e.entity().entity_type))),
        );
        let tickers = &self.chunks.block_entity_tickers.list;
        let block_entity_types = get_type_count(tickers.iter().map(|&(_, pos)| {
            self.chunks.loaded_chunk(pos.chunk()).and_then(|chunk| chunk.block_entities.map.get(&pos)).map_or("<null>", |be| be.ty().name())
        }));
        let entity_manager_stats = self.entity_manager.gather_stats();
        format!(
            "players: {}, entities: {} [{}], block_entities: {} [{}], block_ticks: {}, fluid_ticks: {}, chunk_source: Chunks[S] W: {} E: {}",
            self.players.len(),
            entity_manager_stats,
            entity_types,
            tickers.len(),
            block_entity_types,
            self.chunks.block_ticks.count(),
            self.chunks.fluid_ticks.count(),
            self.chunks.loaded_chunks_count(),
            entity_manager_stats
        )
    }

    pub fn save(&mut self, dir: &WorldDir, flush: bool, no_save: bool) {
        if no_save {
            return;
        }
        self.save_level_data(dir);
        if self.chunks.save(self.game_time, flush).is_err() {
            warn!("Failed to synchronize chunks");
        }
        if flush {
            self.save_all_entities();
        } else {
            self.auto_save_entities();
        }
    }

    fn save_level_data(&mut self, dir: &WorldDir) {
        crate::server::log_save_failure("world_border.dat", self.world_border.save());
        crate::server::log_save_failure("raids.dat", self.raids.save());
        let tickets = &mut self.chunks.distance_manager.ticket_storage;
        if tickets.dirty {
            let file = Nbt::Compound(vec![("data".into(), tickets.to_nbt()), ("DataVersion".into(), Nbt::Int(crisp_data::WORLD_VERSION))]);
            match write_dat(&chunk_tickets_file(dir, self.dimension), &file) {
                Ok(()) => tickets.dirty = false,
                Err(_) => error!("Could not save data to chunk_tickets.dat"),
            }
        }
    }

    pub fn load_chunk_tickets(&mut self, dir: &WorldDir) {
        match read_dat(&chunk_tickets_file(dir, self.dimension)) {
            Ok(tag) => {
                if let Some(data) = tag.as_ref().and_then(|t| t.get("data")) {
                    self.chunks.distance_manager.ticket_storage = ticket::TicketStorage::from_nbt(data);
                }
            }
            Err(_) => error!("Error loading saved data: SavedDataType[minecraft:chunk_tickets]"),
        }
    }

    pub fn environment_attributes(&self) -> EnvironmentAttributeReader<'_, ServerLevel> {
        self.environment_attributes.reader(self)
    }

    pub fn environment_attribute_loot_value(
        &self,
        attribute: &crate::world::attribute::environment_attribute::EnvironmentAttributeInfo,
        origin: Option<Vec3>,
    ) -> crate::world::attribute::Value {
        self.environment_attributes.get_loot_value(attribute, origin, self)
    }

    pub fn sync_clocks(&mut self, clocks: &ServerClockManager) {
        self.clock_ticks = clocks.clocks.iter().map(|c| c.total_ticks).collect();
        self.environment_attributes.invalidate_tick_cache();
    }

    pub fn prepare_weather(&mut self) {
        let weather = self.weather.get();
        if weather.raining {
            self.rain_level = 1.0;
            if weather.thundering {
                self.thunder_level = 1.0;
            }
        }
    }

    pub fn can_have_weather(&self) -> bool {
        let dimension = self.dimension_type();
        dimension.has_skylight && !dimension.has_ceiling && self.dimension != "minecraft:the_end"
    }

    pub fn get_rain_level(&self, a: f32) -> f32 {
        mth::lerp_f(a, self.o_rain_level, self.rain_level)
    }

    pub fn get_thunder_level(&self, a: f32) -> f32 {
        mth::lerp_f(a, self.o_thunder_level, self.thunder_level) * self.get_rain_level(a)
    }

    pub fn is_raining(&self) -> bool {
        self.can_have_weather() && self.get_rain_level(1.0) as f64 > 0.2
    }

    pub fn is_thundering(&self) -> bool {
        self.can_have_weather() && self.get_thunder_level(1.0) as f64 > 0.9
    }

    fn advance_weather_cycle(&mut self) {
        let was_raining = self.is_raining();
        if self.can_have_weather() {
            if self.game_rules.get_bool(crisp_data::game_rule::ADVANCE_WEATHER) {
                let mut weather = self.weather.get();
                let w = &mut weather;
                if w.clear_weather_time > 0 {
                    w.clear_weather_time -= 1;
                    w.thunder_time = if w.thundering { 0 } else { 1 };
                    w.rain_time = if w.raining { 0 } else { 1 };
                    w.thundering = false;
                    w.raining = false;
                } else {
                    let sample = |random: &mut LegacyRandomSource, (min, max): (i32, i32)| mth::random_between_inclusive(random, min, max);
                    if w.thunder_time > 0 {
                        w.thunder_time -= 1;
                        if w.thunder_time == 0 {
                            w.thundering = !w.thundering;
                        }
                    } else if w.thundering {
                        w.thunder_time = sample(&mut self.random, THUNDER_DURATION);
                    } else {
                        w.thunder_time = sample(&mut self.random, THUNDER_DELAY);
                    }
                    if w.rain_time > 0 {
                        w.rain_time -= 1;
                        if w.rain_time == 0 {
                            w.raining = !w.raining;
                        }
                    } else if w.raining {
                        w.rain_time = sample(&mut self.random, RAIN_DURATION);
                    } else {
                        w.rain_time = sample(&mut self.random, RAIN_DELAY);
                    }
                }
                self.weather.set(weather);
            }
            let weather = self.weather.get();
            self.o_thunder_level = self.thunder_level;
            self.thunder_level = mth::clamp_f(self.thunder_level + if weather.thundering { 0.01 } else { -0.01 }, 0.0, 1.0);
            self.o_rain_level = self.rain_level;
            self.rain_level = mth::clamp_f(self.rain_level + if weather.raining { 0.01 } else { -0.01 }, 0.0, 1.0);
        }
        if self.o_rain_level != self.rain_level {
            self.broadcast_game_event(GAME_EVENT_RAIN_LEVEL_CHANGE, self.rain_level);
        }
        if self.o_thunder_level != self.thunder_level {
            self.broadcast_game_event(GAME_EVENT_THUNDER_LEVEL_CHANGE, self.thunder_level);
        }
        if was_raining != self.is_raining() {
            self.broadcast_game_event(if was_raining { GAME_EVENT_STOP_RAINING } else { GAME_EVENT_START_RAINING }, 0.0);
            self.broadcast_game_event(GAME_EVENT_RAIN_LEVEL_CHANGE, self.rain_level);
            self.broadcast_game_event(GAME_EVENT_THUNDER_LEVEL_CHANGE, self.thunder_level);
        }
    }

    pub fn write_weather_for_joining(&self, out: &crate::network::connection::Outbound) {
        if self.is_raining() {
            for (event, value) in [(GAME_EVENT_START_RAINING, 0.0), (GAME_EVENT_RAIN_LEVEL_CHANGE, self.get_rain_level(1.0)), (GAME_EVENT_THUNDER_LEVEL_CHANGE, self.get_thunder_level(1.0))] {
                out.send(clientbound::GAME_EVENT, |p| {
                    p.u8(event);
                    p.f32(value);
                });
            }
        }
    }

    fn broadcast_game_event(&self, event: u8, value: f32) {
        let packet = encode(clientbound::GAME_EVENT, |p| {
            p.u8(event);
            p.f32(value);
        });
        for player in self.players.values() {
            player.connection.send_raw(packet.clone());
        }
    }

    pub fn is_under_spawn_protection(&self, pos: BlockPos, player: EntityId) -> bool {
        let protection = &self.spawn_protection;
        if self.dimension != protection.dimension {
            return false;
        }
        if protection.ops.is_empty() || protection.ops.contains(&self.player(player).profile.uuid) || protection.radius <= 0 {
            return false;
        }
        let distance = (pos.x - protection.spawn.x).abs().max((pos.z - protection.spawn.z).abs());
        distance <= protection.radius
    }

    pub fn may_interact(&self, player: EntityId, pos: BlockPos) -> bool {
        !self.is_under_spawn_protection(pos, player) && self.world_border.is_within_bounds_pos(pos)
    }

    pub fn send_spawn_protection_message(&self, player: EntityId, pos: BlockPos) {
        use crate::network::chat::{Arg, ChatFormatting, Component};
        let message = Component::translatable_with("build.spawn_protection", vec![Arg::String(format!("{}, {}, {}", pos.x, pos.y, pos.z))]).with_color(ChatFormatting::Red);
        self.player(player).connection.send(clientbound::SYSTEM_CHAT, |p| {
            message.write(p);
            p.bool(true);
        });
    }

    pub fn send_overlay_message(&self, player: EntityId, message: &crate::network::chat::Component) {
        self.player(player).connection.send(clientbound::SYSTEM_CHAT, |p| {
            message.write(p);
            p.bool(true);
        });
    }

    pub fn send_build_limit_message(&self, player: EntityId, too_high: bool, limit: i32) {
        use crate::network::chat::{Arg, ChatFormatting, Component};
        let message = Component::translatable_with(if too_high { "build.tooHigh" } else { "build.tooLow" }, vec![Arg::Int(limit)]).with_color(ChatFormatting::Red);
        self.player(player).connection.send(clientbound::SYSTEM_CHAT, |p| {
            message.write(p);
            p.bool(true);
        });
    }

    pub fn set_chunk_forced(&mut self, x: i32, z: i32, forced: bool) -> bool {
        let pos = ChunkPos::new(x, z);
        let updated = self.chunks.update_chunk_forced(pos, forced);
        if forced && updated {
            self.chunks.chunk(pos);
        }
        updated
    }

    pub fn get_waypoint_manager(&self) -> std::cell::RefMut<'_, ServerWaypointManager> {
        self.waypoint_manager.borrow_mut()
    }

    pub fn dimension_type(&self) -> &'static DimensionType {
        self.chunks.dimension
    }

    pub fn player(&self, id: EntityId) -> &ServerPlayer {
        &self.players[&id]
    }

    pub fn player_mut(&mut self, id: EntityId) -> &mut ServerPlayer {
        self.players.get_mut(&id).expect("player in this level")
    }

    pub fn entity_ref(&self, id: EntityId) -> Option<&Entity> {
        match self.players.get(&id) {
            Some(p) => Some(&p.entity),
            None => self.entities.get(&id).map(LevelEntity::entity),
        }
    }

    pub fn all_entities(&self) -> impl Iterator<Item = EntityId> + '_ {
        self.entity_manager.visible_entity_storage.all_entities()
    }

    pub fn is_in_valid_bounds(&self, pos: BlockPos) -> bool {
        !self.chunks.is_outside_build_height(pos.y) && (-30_000_000..30_000_000).contains(&pos.x) && (-30_000_000..30_000_000).contains(&pos.z)
    }

    pub fn set_block(&mut self, pos: BlockPos, state: BlockState, flags: i32) -> bool {
        self.set_block_with_limit(pos, state, flags, UPDATE_LIMIT)
    }

    pub fn set_block_with_limit(&mut self, pos: BlockPos, state: BlockState, flags: i32, update_limit: i32) -> bool {
        self.set_block_removing(pos, state, flags, update_limit, &mut None)
    }

    pub fn set_block_removing(&mut self, pos: BlockPos, state: BlockState, flags: i32, update_limit: i32, removed: &mut Option<BlockEntity>) -> bool {
        if !self.is_in_valid_bounds(pos) {
            return false;
        }
        let Some(old) = self.chunks.set_block_state(pos, state) else { return false };
        let new_block = state.block();
        let block_changed = old.block() != new_block;
        let moved_by_piston = flags & UPDATE_MOVE_BY_PISTON != 0;
        if block_changed && old.has_block_entity() && !block::of(state).should_changed_state_keep_block_entity(old) {
            if flags & UPDATE_SKIP_BLOCK_ENTITY_SIDEEFFECTS == 0 {
                self.pre_remove_side_effects(pos);
            }
            *removed = self.chunks.remove_block_entity(pos);
            if let Some(entity) = removed {
                self.block_entity_set_removed(pos, entity);
            }
        }
        if (block_changed || new_block.is_instance_of("BaseRailBlock")) && (flags & UPDATE_NEIGHBORS != 0 || moved_by_piston) {
            block::of(old).affect_neighbors_after_removal(old, self, pos, moved_by_piston);
        }
        if !self.block_state(pos).is_block(new_block) {
            return false;
        }
        if flags & UPDATE_SKIP_ON_PLACE == 0 {
            block::of(state).on_place(state, self, pos, old, moved_by_piston);
        }
        if state.has_block_entity() && self.block_state(pos).is_block(new_block) {
            let existing = self.chunks.block_entity(pos, EntityCreationType::Check).map(|e| e.is_valid_block_state(state));
            if existing == Some(false) {
                let ty = self.chunks.block_entity_ref(pos).map(BlockEntity::ty);
                warn!("Found mismatched block entity @ {pos:?}: type = {ty:?}, state = {state:?}");
                if let Some(entity) = self.chunks.remove_block_entity(pos) {
                    self.block_entity_set_removed(pos, &entity);
                }
            }
            if existing == Some(true) {
                if let Some(entity) = self.chunks.block_entity(pos, EntityCreationType::Check) {
                    entity.set_block_state(state);
                }
                self.chunks.update_block_entity_ticker(pos);
            } else if let Some(entity) = BlockEntity::new_for(pos, state) {
                self.chunks.add_and_register_block_entity(entity);
            }
        }
        if self.block_state(pos) == state {
            if flags & UPDATE_CLIENTS != 0 {
                self.send_block_updated(pos, old, state);
            }
            if flags & UPDATE_NEIGHBORS != 0 {
                self.update_neighbors_at(pos, old.block());
                if state.has_analog_output_signal() {
                    self.update_neighbour_for_output_signal(pos, new_block);
                }
            }
            if flags & UPDATE_KNOWN_SHAPE == 0 && update_limit > 0 {
                let neighbour_flags = flags & !(UPDATE_NEIGHBORS | UPDATE_SUPPRESS_DROPS);
                block::of(old).update_indirect_neighbour_shapes(old, self, pos, neighbour_flags, update_limit - 1);
                self.update_neighbour_shapes(state, pos, neighbour_flags, update_limit - 1);
                block::of(state).update_indirect_neighbour_shapes(state, self, pos, neighbour_flags, update_limit - 1);
            }
            self.update_poi_on_block_state_change(pos, old, state);
        }
        true
    }

    pub fn update_poi_on_block_state_change(&mut self, pos: BlockPos, old: BlockState, new: BlockState) {
        self.chunks.poi_manager.update_on_block_state_change(pos, old, new);
    }

    pub fn send_block_updated(&mut self, pos: BlockPos, old: BlockState, current: BlockState) {
        if self.chunks.is_ticking(pos.chunk()) {
            self.chunk_map.block_changed(pos);
        }
        self.path_types_by_pos_cache.get_mut().invalidate(pos);
        if join_is_not_empty(old.collision_voxel_shape(), current.collision_voxel_shape(), BooleanOp::NotSame) {
            let navigations_to_update: Vec<EntityId> = self
                .entities
                .iter()
                .filter_map(|(&id, entity)| match entity {
                    LevelEntity::Mob(mob) if mob.ai.navigation.should_recompute_path(self, id, pos) => Some(id),
                    _ => None,
                })
                .collect();
            for id in navigations_to_update {
                self.get_navigation(id, |navigation, level| navigation.recompute_path(level, id));
            }
        }
    }

    pub fn remove_block(&mut self, pos: BlockPos, moved_by_piston: bool) -> bool {
        let fluid = self.fluid_state(pos);
        let flags = UPDATE_ALL | if moved_by_piston { UPDATE_MOVE_BY_PISTON } else { 0 };
        self.set_block(pos, legacy_block(fluid), flags)
    }

    pub fn destroy_block(&mut self, pos: BlockPos, drop_resources: bool, breaker: Option<EntityId>, update_limit: i32) -> bool {
        let state = self.block_state(pos);
        if state.is_air() {
            return false;
        }
        self.level_event(None, events::PARTICLES_AND_SOUND_DESTROY_BLOCK, pos, state.id() as i32);
        if drop_resources {
            let block_entity = if state.has_block_entity() { self.get_block_entity(pos).cloned() } else { None };
            self.drop_resources_with_entity(state, pos, block_entity.as_ref(), breaker, None);
        }
        let fluid = self.fluid_state(pos);
        self.set_block_with_limit(pos, legacy_block(fluid), UPDATE_ALL, update_limit)
    }

    pub fn drop_resources(&mut self, state: BlockState, pos: BlockPos, breaker: Option<EntityId>, tool: Option<ItemStack>) {
        self.drop_resources_with_entity(state, pos, None, breaker, tool);
    }

    pub fn drop_resources_with_entity(&mut self, state: BlockState, pos: BlockPos, block_entity: Option<&BlockEntity>, breaker: Option<EntityId>, tool: Option<ItemStack>) {
        let drops = block::get_drops(state, self, pos, block_entity, breaker, tool.as_ref().unwrap_or(&ItemStack::empty()));
        for stack in drops {
            self.pop_resource(pos, stack);
        }
        block::of(state).spawn_after_break(state, self, pos, tool, true);
    }

    pub fn pop_experience(&mut self, pos: BlockPos, amount: i32) {
        if self.game_rules.get_bool(crisp_data::game_rule::BLOCK_DROPS) {
            self.award_experience(Vec3::at_center_of(pos), amount);
        }
    }

    pub fn pop_resource(&mut self, pos: BlockPos, stack: ItemStack) {
        let half_height = ItemEntity::DIMENSIONS.height as f64 / 2.0;
        let x = pos.x as f64 + 0.5 + mth::next_double(&mut self.random, -0.25, 0.25);
        let y = pos.y as f64 + 0.5 + mth::next_double(&mut self.random, -0.25, 0.25) - half_height;
        let z = pos.z as f64 + 0.5 + mth::next_double(&mut self.random, -0.25, 0.25);
        if stack.is_empty() || !self.game_rules.get_bool(crisp_data::game_rule::BLOCK_DROPS) {
            return;
        }
        let mut item = ItemEntity::new(Vec3::new(x, y, z), stack);
        item.set_default_pickup_delay();
        self.add_fresh_entity(LevelEntity::Item(item));
    }

    pub fn pop_resource_from_face(&mut self, pos: BlockPos, face: crisp_core::Direction, stack: ItemStack) {
        let (step_x, step_y, step_z) = (face.step_x(), face.step_y(), face.step_z());
        let half_width = ItemEntity::DIMENSIONS.width as f64 / 2.0;
        let half_height = ItemEntity::DIMENSIONS.height as f64 / 2.0;
        let random = &mut self.random;
        let x = pos.x as f64 + 0.5 + if step_x == 0 { mth::next_double(random, -0.25, 0.25) } else { step_x as f64 * (0.5 + half_width) };
        let y = pos.y as f64 + 0.5 + if step_y == 0 { mth::next_double(random, -0.25, 0.25) } else { step_y as f64 * (0.5 + half_height) } - half_height;
        let z = pos.z as f64 + 0.5 + if step_z == 0 { mth::next_double(random, -0.25, 0.25) } else { step_z as f64 * (0.5 + half_width) };
        let delta_x = if step_x == 0 { mth::next_double(random, -0.1, 0.1) } else { step_x as f64 * 0.1 };
        let delta_y = if step_y == 0 { mth::next_double(random, 0.0, 0.1) } else { step_y as f64 * 0.1 + 0.1 };
        let delta_z = if step_z == 0 { mth::next_double(random, -0.1, 0.1) } else { step_z as f64 * 0.1 };
        if stack.is_empty() || !self.game_rules.get_bool(crisp_data::game_rule::BLOCK_DROPS) {
            return;
        }
        let mut item = ItemEntity::with_movement(Vec3::new(x, y, z), stack, Vec3::new(delta_x, delta_y, delta_z));
        item.set_default_pickup_delay();
        self.add_fresh_entity(LevelEntity::Item(item));
    }

    pub fn update_neighbors_at(&mut self, pos: BlockPos, block: Block) {
        self.update_neighbors_at_except_from_facing(pos, block, None);
    }

    pub fn update_neighbors_at_except_from_facing(&mut self, pos: BlockPos, block: Block, skip: Option<Direction>) {
        let idx = usize::from(skip == Some(UPDATE_ORDER[0]));
        self.add_and_run(pos, NeighborUpdates::Multi { source_pos: pos, source_block: block, skip, idx });
    }

    pub fn neighbor_changed(&mut self, pos: BlockPos, block: Block) {
        self.add_and_run(pos, NeighborUpdates::Simple { pos, block });
    }

    pub fn neighbor_changed_with_state(&mut self, state: BlockState, pos: BlockPos, block: Block, moved_by_piston: bool) {
        self.add_and_run(pos, NeighborUpdates::Full { state, pos, block, moved_by_piston });
    }

    pub fn neighbor_shape_changed(&mut self, direction: Direction, pos: BlockPos, neighbor_pos: BlockPos, neighbor_state: BlockState, flags: i32, update_limit: i32) {
        self.add_and_run(pos, NeighborUpdates::Shape { direction, neighbor_state, pos, neighbor_pos, flags, update_limit });
    }

    pub fn update_neighbour_shapes(&mut self, state: BlockState, pos: BlockPos, flags: i32, update_limit: i32) {
        for direction in UPDATE_SHAPE_ORDER {
            self.neighbor_shape_changed(direction.opposite(), pos.relative(direction), pos, state, flags, update_limit);
        }
    }

    pub fn set_max_chained_neighbor_updates(&mut self, max_chained_neighbor_updates: i32) {
        self.neighbor_updater.max_chained_neighbor_updates = max_chained_neighbor_updates;
    }

    fn add_and_run(&mut self, pos: BlockPos, update: NeighborUpdates) {
        let updater = &mut self.neighbor_updater;
        let running_already = updater.count > 0;
        let too_many = updater.max_chained_neighbor_updates >= 0 && updater.count >= updater.max_chained_neighbor_updates;
        updater.count += 1;
        if !too_many {
            if running_already {
                updater.added_this_layer.push(update);
            } else {
                updater.stack.push(update);
            }
        } else if updater.count - 1 == updater.max_chained_neighbor_updates {
            error!("Too many chained neighbor updates. Skipping the rest. First skipped position: {}, {}, {}", pos.x, pos.y, pos.z);
        }
        if !running_already {
            self.run_neighbor_updates();
        }
    }

    fn run_neighbor_updates(&mut self) {
        loop {
            let updater = &mut self.neighbor_updater;
            if updater.stack.is_empty() && updater.added_this_layer.is_empty() {
                break;
            }
            while let Some(update) = updater.added_this_layer.pop() {
                updater.stack.push(update);
            }
            while self.neighbor_updater.added_this_layer.is_empty() {
                let (step, more) = self.next_neighbor_step();
                self.execute_neighbor_step(step);
                if !more {
                    self.neighbor_updater.stack.pop();
                    break;
                }
            }
        }
        let updater = &mut self.neighbor_updater;
        updater.stack.clear();
        updater.added_this_layer.clear();
        updater.count = 0;
    }

    fn next_neighbor_step(&mut self) -> (NeighborStep, bool) {
        match self.neighbor_updater.stack.last_mut().expect("a queued update") {
            &mut NeighborUpdates::Shape { direction, neighbor_state, pos, neighbor_pos, flags, update_limit } => {
                (NeighborStep::Shape { direction, neighbor_state, pos, neighbor_pos, flags, update_limit }, false)
            }
            &mut NeighborUpdates::Simple { pos, block } => (NeighborStep::Changed { state: None, pos, block, moved_by_piston: false }, false),
            &mut NeighborUpdates::Full { state, pos, block, moved_by_piston } => (NeighborStep::Changed { state: Some(state), pos, block, moved_by_piston }, false),
            NeighborUpdates::Multi { source_pos, source_block, skip, idx } => {
                let direction = UPDATE_ORDER[*idx];
                *idx += 1;
                if *idx < UPDATE_ORDER.len() && Some(UPDATE_ORDER[*idx]) == *skip {
                    *idx += 1;
                }
                let step = NeighborStep::Changed { state: None, pos: source_pos.relative(direction), block: *source_block, moved_by_piston: false };
                (step, *idx < UPDATE_ORDER.len())
            }
        }
    }

    fn execute_neighbor_step(&mut self, step: NeighborStep) {
        match step {
            NeighborStep::Shape { direction, neighbor_state, pos, neighbor_pos, flags, update_limit } => {
                let current = self.block_state(pos);
                if flags & UPDATE_SKIP_SHAPE_UPDATE_ON_WIRE != 0 && current.is_block(blocks::REDSTONE_WIRE) {
                    return;
                }
                let new = block::of(current).update_shape(current, self, pos, direction, neighbor_pos, neighbor_state);
                self.update_or_destroy(current, new, pos, flags, update_limit);
            }
            NeighborStep::Changed { state, pos, block, moved_by_piston } => {
                let state = state.unwrap_or_else(|| self.block_state(pos));
                block::of(state).neighbor_changed(state, self, pos, block, moved_by_piston);
            }
        }
    }

    pub fn update_or_destroy(&mut self, old: BlockState, new: BlockState, pos: BlockPos, flags: i32, update_limit: i32) {
        if new != old {
            if new.is_air() {
                self.destroy_block(pos, flags & UPDATE_SUPPRESS_DROPS == 0, None, update_limit);
            } else {
                self.set_block_with_limit(pos, new, flags & !UPDATE_SUPPRESS_DROPS, update_limit);
            }
        }
    }

    pub fn update_neighbours_on_block_set(&mut self, pos: BlockPos, old: BlockState) {
        let state = self.block_state(pos);
        let block = state.block();
        if !old.is_block(block) {
            block::of(old).affect_neighbors_after_removal(old, self, pos, false);
        }
        self.update_neighbors_at(pos, block);
        if state.has_analog_output_signal() {
            self.update_neighbour_for_output_signal(pos, block);
        }
    }

    pub fn update_from_neighbour_shapes(&mut self, state: BlockState, pos: BlockPos) -> BlockState {
        let mut new = state;
        for direction in UPDATE_SHAPE_ORDER {
            let neighbour_pos = pos.relative(direction);
            let neighbour = self.block_state(neighbour_pos);
            new = block::of(new).update_shape(new, self, pos, direction, neighbour_pos, neighbour);
        }
        new
    }

    pub fn post_process_generation(&mut self, chunk_pos: ChunkPos) {
        let chunk = self.chunks.chunk(chunk_pos);
        let min_section_y = chunk.min_y() >> 4;
        let section_count = chunk.post_processing.len();
        for section_index in 0..section_count {
            let offsets = std::mem::take(&mut self.chunks.chunk(chunk_pos).post_processing[section_index]);
            for packed in offsets {
                let pos = BlockPos::new(
                    chunk_pos.min_block_x() + (packed & 15) as i32,
                    ((min_section_y + section_index as i32) << 4) + ((packed >> 4) & 15) as i32,
                    chunk_pos.min_block_z() + ((packed >> 8) & 15) as i32,
                );
                let state = self.block_state(pos);
                if !state.fluid_state().is_empty() {
                    crate::world::level::material::tick(self, pos, state);
                }
                if state.block().info().classes.contains(&"LiquidBlock") {
                    block::of(state).tick(state, self, pos);
                } else {
                    let new_state = self.update_from_neighbour_shapes(state, pos);
                    if new_state != state {
                        self.set_block(pos, new_state, 276);
                    }
                }
            }
        }
        for pos in self.chunks.chunk(chunk_pos).block_entities.pending.keys().collect::<Vec<_>>() {
            self.chunks.block_entity(pos, EntityCreationType::Check);
        }
        self.chunks.chunk(chunk_pos).block_entities.pending.clear();
    }

    pub fn block_event(&mut self, pos: BlockPos, block: Block, b0: i32, b1: i32) {
        if !self.block_events.contains(&(pos, block, b0, b1)) {
            self.block_events.push((pos, block, b0, b1));
        }
    }

    fn run_block_events(&mut self) {
        while !self.block_events.is_empty() {
            let (pos, block, b0, b1) = self.block_events.remove(0);
            let state = self.block_state(pos);
            if state.is_block(block) && block::of(state).trigger_event(state, self, pos, b0, b1) {
                let packet = encode(clientbound::BLOCK_EVENT, |p| {
                    p.position(pos.x, pos.y, pos.z);
                    p.u8(b0 as u8);
                    p.u8(b1 as u8);
                    p.varint(block.0 as i32);
                });
                self.broadcast_near(None, pos.x as f64, pos.y as f64, pos.z as f64, 64.0, &packet);
            }
        }
    }

    pub fn broadcast_near(&self, except: Option<EntityId>, x: f64, y: f64, z: f64, range: f64, packet: &[u8]) {
        for p in self.players.values() {
            if Some(p.entity.id) == except {
                continue;
            }
            let (dx, dy, dz) = (x - p.entity.x(), y - p.entity.y(), z - p.entity.z());
            if dx * dx + dy * dy + dz * dz < range * range {
                p.connection.send_raw(packet.to_vec());
            }
        }
    }

    pub fn global_level_event(&mut self, event: i32, pos: BlockPos, data: i32) {
        if self.game_rules.get_bool(crisp_data::game_rule::GLOBAL_SOUND_EVENTS) {
            self.global_level_events.push((event, pos, data));
        } else {
            self.level_event(None, event, pos, data);
        }
    }

    pub fn level_event(&self, source: Option<EntityId>, event: i32, pos: BlockPos, data: i32) {
        let packet = encode(clientbound::LEVEL_EVENT, |p| {
            p.i32(event);
            p.position(pos.x, pos.y, pos.z);
            p.i32(data);
            p.bool(false);
        });
        self.broadcast_near(source, pos.x as f64, pos.y as f64, pos.z as f64, 64.0, &packet);
    }

    pub fn play_sound(&mut self, except: Option<EntityId>, pos: BlockPos, sound: i32, source: SoundSource, volume: f32, pitch: f32) {
        let seed = self.thread_safe_random.next_long();
        let (x, y, z) = (pos.x as f64 + 0.5, pos.y as f64 + 0.5, pos.z as f64 + 0.5);
        let packet = encode(clientbound::SOUND, |p| {
            p.varint(sound + 1);
            p.varint(source as i32);
            p.i32((x * 8.0) as i32);
            p.i32((y * 8.0) as i32);
            p.i32((z * 8.0) as i32);
            p.f32(volume);
            p.f32(pitch);
            p.i64(seed);
        });
        let range = if volume > 1.0 { 16.0 * volume } else { 16.0 } as f64;
        self.broadcast_near(except, x, y, z, range, &packet);
    }

    pub fn broadcast_entity_event(&self, entity: EntityId, event: u8) {
        let packet = encode(clientbound::ENTITY_EVENT, |p| {
            p.i32(entity);
            p.u8(event);
        });
        self.send_to_tracking_players_and_self(entity, &packet);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn send_block_particles(&self, state: BlockState, x: f64, y: f64, z: f64, count: i32, dist: (f64, f64, f64), speed: f64) {
        self.send_particles("minecraft:block", |p| p.varint(state.id() as i32), x, y, z, count, dist, speed);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn send_particles(&self, particle: &str, options: impl Fn(&mut crisp_protocol::Writer), x: f64, y: f64, z: f64, count: i32, dist: (f64, f64, f64), speed: f64) {
        self.send_particles_with_speeds(particle, options, x, y, z, count, dist, (speed, speed, speed), 0);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn send_particles_with_speeds(
        &self,
        particle: &str,
        options: impl Fn(&mut crisp_protocol::Writer),
        x: f64,
        y: f64,
        z: f64,
        count: i32,
        dist: (f64, f64, f64),
        speed: (f64, f64, f64),
        randomization_type: i32,
    ) {
        let packet = encode(clientbound::LEVEL_PARTICLES, |p| {
            p.varint(crisp_data::registry_id(crisp_data::registry::PARTICLE_TYPE, particle));
            options(p);
            p.bool(false);
            p.bool(false);
            p.f64(x);
            p.f64(y);
            p.f64(z);
            p.f32(dist.0 as f32);
            p.f32(dist.1 as f32);
            p.f32(dist.2 as f32);
            p.f32(speed.0 as f32);
            p.f32(speed.1 as f32);
            p.f32(speed.2 as f32);
            p.varint(count);
            p.varint(randomization_type);
        });
        for player in self.players.values() {
            if player.entity.is_removed() {
                continue;
            }
            let center = Vec3::at_center_of(player.entity.block_position());
            if center.distance_to_sqr(Vec3::new(x, y, z)) < 32.0 * 32.0 {
                player.connection.send_raw(packet.clone());
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn send_particles_to_player(
        &self,
        player: EntityId,
        particle: &crate::core::particles::particle_options::ParticleOptions,
        override_limiter: bool,
        always_show: bool,
        pos: Vec3,
        count: i32,
        dist: Vec3,
        speed: f64,
    ) -> bool {
        let Some(player) = self.players.get(&player) else { return false };
        if !Vec3::at_center_of(player.entity.block_position()).closer_than(pos, if override_limiter { 512.0 } else { 32.0 }) {
            return false;
        }
        player.connection.send(clientbound::LEVEL_PARTICLES, |p| {
            particle.write(p);
            p.bool(override_limiter);
            p.bool(always_show);
            p.f64(pos.x);
            p.f64(pos.y);
            p.f64(pos.z);
            p.f32(dist.x as f32);
            p.f32(dist.y as f32);
            p.f32(dist.z as f32);
            for _ in 0..3 {
                p.f32(speed as f32);
            }
            p.varint(count);
            p.varint(0);
        });
        true
    }

    pub fn add_world_gen_chunk_entities(&mut self, entities: &[Nbt]) {
        for nbt in entities {
            if let Some(entity) = self.load_entity(nbt) {
                self.add_entity(entity);
            }
        }
    }

    pub fn add_fresh_entity(&mut self, entity: LevelEntity) -> bool {
        if entity.entity().is_removed() {
            let ty = crisp_data::registry::ENTITY_TYPE[entity.entity().entity_type as usize];
            warn!("Tried to add entity {ty} but it was marked as removed already");
            return false;
        }
        self.add_entity(entity)
    }

    pub fn add_player(&mut self, player: ServerPlayer) {
        let (id, uuid) = (player.entity.id, player.entity.uuid);
        self.world_border.listener.add_connection(id, player.connection.clone());
        self.players.insert(id, player);
        self.entity_manager.add_player_uuid(uuid);
        self.entity_manager.visible_entity_storage.add(id, uuid);
        self.add_player_to_section(id);
        if self.is_transmitting_waypoint(id) {
            self.get_waypoint_manager().track_waypoint(self, id);
        }
        self.tick_list.add(id);
        let config = TrackingConfig { range: 32 * 16, update_interval: UpdateInterval::Periodic(2), track_deltas: false, stepped: true, is_item: false };
        self.track_entity(id, config);
        if self.is_receiving_waypoints(id) {
            self.get_waypoint_manager().add_player(self, id);
        }
        if self.is_transmitting_waypoint(id) {
            self.get_waypoint_manager().track_waypoint(self, id);
        }
        self.update_sleeping_player_list();
    }

    pub fn reset_empty_time(&mut self) {
        self.empty_time = 0;
    }

    pub fn remove_player_immediately(&mut self, id: EntityId, reason: crate::world::entity::RemovalReason) -> ServerPlayer {
        self.player_remove(id, reason);
        self.world_border.listener.remove_connection(id);
        let player = self.players.shift_remove(&id).expect("removed player");
        self.update_sleeping_player_list();
        player
    }

    pub fn remove_player(&mut self, id: EntityId) -> Option<ServerPlayer> {
        self.world_border.listener.remove_connection(id);
        self.player_left_level(id);
        let player = self.players.shift_remove(&id);
        self.update_sleeping_player_list();
        player
    }

    pub fn player_left_level(&mut self, id: EntityId) {
        let uuid = self.player(id).entity.uuid;
        self.untrack_entity(id);
        self.get_waypoint_manager().remove_player(self, id);
        self.remove_player_from_section(id);
        self.entity_manager.visible_entity_storage.remove(id, uuid);
        self.entity_manager.remove_player_uuid(uuid);
        self.tick_list.remove(id);
    }

    pub fn remove_entity(&mut self, id: EntityId) {
        let reason = self.entities.get(&id).and_then(|e| e.entity().removal);
        self.entity_removed(id, reason);
        self.entities.swap_remove(&id);
    }

    pub fn close(&mut self) {
        if self.world_border.save().is_err() {
            error!("Could not save data to world_border.dat");
        }
        if self.chunks.save(self.game_time, true).is_err() {
            warn!("Failed to synchronize chunks");
        }
        self.save_all_entities();
    }

    pub fn get_entities(&self, except: Option<EntityId>, area: &AABB) -> Vec<EntityId> {
        let mut found = Vec::new();
        self.entity_manager.section_storage.for_each_accessible_non_empty_section(area, |section| {
            for &id in section.entities() {
                if Some(id) != except
                    && let Some(entity) = self.entity_ref(id)
                    && !entity.is_removed()
                    && entity.bounding_box().intersects(area)
                    && !self.is_spectator(id)
                {
                    found.push(id);
                }
            }
        });
        found
    }

    pub fn items_in(&self, area: &AABB, except: Option<EntityId>) -> Vec<EntityId> {
        let mut found = Vec::new();
        self.entity_manager.section_storage.for_each_accessible_non_empty_section(area, |section| {
            for &id in section.entities() {
                if Some(id) != except
                    && let Some(LevelEntity::Item(item)) = self.entities.get(&id)
                    && !item.entity.is_removed()
                    && item.entity.bounding_box().intersects(area)
                {
                    found.push(id);
                }
            }
        });
        found
    }

    pub fn tick(&mut self) {
        let runs = self.tick_until_time();
        if runs {
            self.tick_time();
        }
        self.tick_after_time(runs);
        while self.tick_entities() {
            self.pending_portal_teleport = None;
        }
        self.tick_after_entities();
    }

    pub fn tick_until_time(&mut self) -> bool {
        let runs = self.tick_until_sleep();
        if self.should_wake_up_all_players() {
            self.wake_up_all_players_after_clock();
        }
        self.update_sky_brightness();
        runs
    }

    pub fn tick_until_sleep(&mut self) -> bool {
        self.environment_attributes.invalidate_tick_cache();
        let runs = self.tick_rate_manager.runs_normally();
        if runs {
            self.world_border.tick();
            self.advance_weather_cycle();
        }
        runs
    }

    pub fn wake_up_all_players_after_clock(&mut self) {
        self.wake_up_all_players();
        if self.game_rules.get_bool(crisp_data::game_rule::ADVANCE_WEATHER) && self.is_raining() {
            self.reset_weather_cycle();
        }
    }

    pub fn reset_weather_cycle(&mut self) {
        let mut w = self.weather.get();
        w.rain_time = 0;
        w.raining = false;
        w.thunder_time = 0;
        w.thundering = false;
        self.weather.set(w);
    }

    pub fn get_map_data(&self, id: MapId) -> Option<Rc<RefCell<MapItemSavedData>>> {
        self.server_data_storage.borrow_mut().get_map_data(id)
    }

    pub fn set_map_data(&mut self, id: MapId, data: MapItemSavedData) {
        self.server_data_storage.borrow_mut().set_map_data(id, data);
    }

    pub fn get_free_map_id(&mut self) -> MapId {
        self.server_data_storage.borrow_mut().map_index().get_next_map_id()
    }

    pub fn find_closest_biome_3d(
        &self,
        biome_test: impl Fn(crate::world::level::chunk::Biome) -> bool,
        origin: BlockPos,
        max_search_radius: i32,
        sample_resolution_horizontal: i32,
        sample_resolution_vertical: i32,
    ) -> Option<(BlockPos, crate::world::level::chunk::Biome)> {
        self.chunks.generator.find_closest_biome_3d(
            origin,
            max_search_radius,
            sample_resolution_horizontal,
            sample_resolution_vertical,
            biome_test,
            self.chunks.min_y(),
            self.chunks.max_y(),
        )
    }

    pub fn tick_time(&mut self) -> i64 {
        self.game_time += 1;
        self.chunks.game_time = self.game_time;
        self.game_time
    }

    pub fn tick_chunk_source_without_chunks(&mut self) {
        self.chunks.purge_stale_tickets();
        self.chunks.run_distance_manager_updates();
        self.process_chunk_events();
        self.chunks.tick_unloads(self.no_save, true);
    }

    pub fn tick_after_time(&mut self, runs: bool) {
        if runs {
            self.tick_block_ticks();
            self.tick_fluid_ticks();
            self.raids.tick();
        }
        if runs {
            self.chunks.purge_stale_tickets();
        }
        self.chunks.run_distance_manager_updates();
        self.process_chunk_events();
        if runs {
            self.tick_chunks();
        }
        self.broadcast_changed_chunks();
        self.tick_chunk_map();
        self.chunks.tick_unloads(self.no_save, false);
        if runs {
            self.run_block_events();
        }
        if self.chunks.has_active_tickets() {
            self.empty_time = 0;
        }
        if runs {
            self.empty_time += 1;
        }
        if self.empty_time < 300 {
            self.entity_tick_range = Some((0, self.tick_list.begin_iteration()));
        }
    }

    pub fn tick_entities(&mut self) -> bool {
        let Some((start, end)) = self.entity_tick_range else { return false };
        for i in start..end {
            let Some(id) = self.tick_list.at(i) else { continue };
            if self.players.contains_key(&id) {
                self.common_tick(id);
                self.tick_player_entity(id);
            } else if let Some(entity) = self.entities.get(&id) {
                if entity.entity().is_removed() {
                    self.remove_entity(id);
                } else if !self.tick_rate_manager.is_entity_frozen() {
                    let chunk = entity.entity().chunk_position();
                    if self.is_mob(id) {
                        self.check_mob_despawn(id);
                    }
                    if self.in_entity_ticking_range(chunk) {
                        self.common_tick(id);
                        self.tick_entity(id);
                    } else if self.entities.get(&id).is_some_and(|e| e.entity().is_removed()) {
                        self.remove_entity(id);
                    }
                }
            }
            if self.pending_portal_teleport.is_some() {
                self.entity_tick_range = Some((i + 1, end));
                return true;
            }
        }
        self.entity_tick_range = Some((end, end));
        false
    }

    pub fn tick_after_entities(&mut self) {
        if self.entity_tick_range.take().is_some() {
            self.tick_list.end_iteration();
            self.tick_block_entities();
        }
        self.tick_entity_manager();
    }

    fn create_tick<T>(&mut self, pos: BlockPos, ty: T, delay: i32, priority: TickPriority) -> ScheduledTick<T> {
        let sub_tick_order = self.sub_tick_count;
        self.sub_tick_count += 1;
        ScheduledTick { ty, pos, trigger_tick: self.game_time + delay as i64, priority, sub_tick_order }
    }

    pub fn schedule_tick(&mut self, pos: BlockPos, block: Block, delay: i32) {
        self.schedule_tick_with_priority(pos, block, delay, TickPriority::NORMAL);
    }

    pub fn schedule_tick_with_priority(&mut self, pos: BlockPos, block: Block, delay: i32, priority: TickPriority) {
        let tick = self.create_tick(pos, block, delay, priority);
        self.chunks.block_ticks.schedule(tick);
    }

    pub fn schedule_fluid_tick(&mut self, pos: BlockPos, fluid: Fluid, delay: i32) {
        let tick = self.create_tick(pos, fluid, delay, TickPriority::NORMAL);
        self.chunks.fluid_ticks.schedule(tick);
    }

    pub fn has_scheduled_tick(&self, pos: BlockPos, block: Block) -> bool {
        self.chunks.block_ticks.has_scheduled_tick(pos, block)
    }

    pub fn will_tick_this_tick(&self, pos: BlockPos, block: Block) -> bool {
        self.chunks.block_ticks.will_tick_this_tick(pos, block)
    }

    fn tick_block_ticks(&mut self) {
        let entity_manager = &self.entity_manager;
        let due = self.chunks.collect_block_ticks(self.game_time, 65536, |c| entity_manager.are_entities_loaded(c.pack()));
        for tick in due {
            self.chunks.block_ticks.ran(tick.pos, tick.ty);
            let state = self.block_state(tick.pos);
            if state.is_block(tick.ty) {
                block::of(state).tick(state, self, tick.pos);
            }
        }
    }

    fn tick_fluid_ticks(&mut self) {
        let entity_manager = &self.entity_manager;
        let due = self.chunks.collect_fluid_ticks(self.game_time, 65536, |c| entity_manager.are_entities_loaded(c.pack()));
        for tick in due {
            self.chunks.fluid_ticks.ran(tick.pos, tick.ty);
            let state = self.block_state(tick.pos);
            if state.fluid_state().fluid == tick.ty {
                crate::world::level::material::tick(self, tick.pos, state);
            }
        }
    }

    pub fn in_entity_ticking_range(&self, chunk: ChunkPos) -> bool {
        self.chunks.distance_manager.in_entity_ticking_range(chunk.pack())
    }

    pub fn in_block_ticking_range(&self, chunk: ChunkPos) -> bool {
        self.chunks.distance_manager.in_block_ticking_range(chunk.pack())
    }

    pub fn set_simulation_distance(&mut self, simulation_distance: i32) {
        self.simulation_distance = simulation_distance;
        self.chunks.set_simulation_distance(simulation_distance);
    }

    fn tick_chunks(&mut self) {
        self.chunks.distance_manager.natural_spawn_chunk_count();
        let tick_speed = self.game_rules.get_int(crisp_data::game_rule::RANDOM_TICK_SPEED);
        let mut spawning_chunks = self.collect_spawning_chunks();
        crate::util::shuffle(&mut spawning_chunks, &mut self.random);
        for chunk in spawning_chunks {
            self.tick_spawning_chunk(chunk);
        }
        for chunk in self.chunks.block_ticking_chunks() {
            self.tick_chunk(chunk, tick_speed);
        }
    }

    fn collect_spawning_chunks(&mut self) -> Vec<ChunkPos> {
        let candidates = self.chunks.distance_manager.spawn_candidate_chunks();
        candidates
            .into_iter()
            .map(ChunkPos::unpack)
            .filter(|&pos| self.chunks.is_ticking(pos) && self.any_player_close_enough_for_spawning(pos))
            .collect()
    }

    pub fn find_nearest_map_structure(
        &mut self,
        structure_tag: &[crate::world::level::levelgen::structure::structure::StructureHolder],
        origin: BlockPos,
        max_search_radius: i32,
        create_reference: bool,
    ) -> Option<BlockPos> {
        if !self.chunks.generate_structures {
            return None;
        }
        let generator = self.chunks.generator.clone();
        generator.find_nearest_map_structure(self, structure_tag, origin, max_search_radius, create_reference).map(|(pos, _)| pos)
    }

    pub fn is_spawning_monsters(&self) -> bool {
        self.game_rules.get_bool(crisp_data::game_rule::SPAWN_MOBS) && self.game_rules.get_bool(crisp_data::game_rule::SPAWN_MONSTERS)
    }

    pub fn any_player_close_enough_for_spawning(&self, pos: ChunkPos) -> bool {
        let (x, z) = ((pos.min_block_x() + 8) as f64, (pos.min_block_z() + 8) as f64);
        self.players.values().any(|p| {
            let (dx, dz) = (x - p.entity.x(), z - p.entity.z());
            !self.is_spectator(p.entity.id) && dx * dx + dz * dz < 16384.0
        })
    }

    fn tick_spawning_chunk(&mut self, pos: ChunkPos) {
        if let Some(chunk) = self.chunks.chunk_if_present(pos) {
            chunk.inhabited_time += 1;
        }
        if self.in_entity_ticking_range(pos) {
            self.tick_thunder(pos);
        }
    }

    fn tick_thunder(&mut self, pos: ChunkPos) {
        if self.is_raining() && self.is_thundering() && self.random.next_int_bounded(100000) == 0 {
            let target = self.block_random_pos(pos.min_block_x(), 0, pos.min_block_z(), 15);
            let target = self.find_lightning_target_around(target);
            if self.is_raining_at(target) {
                let difficulty = self.current_difficulty_at(target);
                let is_trap = self.game_rules.get_bool(crisp_data::game_rule::SPAWN_MOBS)
                    && self.random.next_double() < difficulty.effective_difficulty as f64 * 0.01
                    && !self.block_state(target.below()).is(&crisp_data::block_tags::LIGHTNING_RODS);
                self.strike_lightning(target, is_trap);
            }
        }
    }

    fn find_lightning_rod(&mut self, center: BlockPos) -> Option<BlockPos> {
        use crate::world::entity::ai::village::poi::poi_manager::Occupancy;
        use crate::world::entity::ai::village::poi::poi_types::PoiType;
        use crate::world::level::world_gen_level::WorldGenLevel;
        let records = self.chunks.poi_manager.get_in_range(&|poi_type| poi_type == PoiType::LightningRod, center, 128, Occupancy::Any);
        let mut nearby_lightning_rod: Option<BlockPos> = None;
        for lightning_rod_pos in records.into_iter().map(|record| record.pos) {
            if lightning_rod_pos.y == self.get_height(crate::world::level::chunk::HeightmapType::WorldSurface, lightning_rod_pos.x, lightning_rod_pos.z) - 1
                && nearby_lightning_rod.is_none_or(|best| lightning_rod_pos.dist_sqr(center) < best.dist_sqr(center))
            {
                nearby_lightning_rod = Some(lightning_rod_pos);
            }
        }
        nearby_lightning_rod.map(BlockPos::above)
    }

    fn find_lightning_target_around(&mut self, pos: BlockPos) -> BlockPos {
        let chunk = self.chunks.chunk(pos.chunk());
        let height = chunk.height_at(crate::world::level::chunk::HeightmapType::MotionBlocking, pos.x, pos.z) + 1;
        let mut center = BlockPos::new(pos.x, height, pos.z);
        if let Some(lightning_rod_target) = self.find_lightning_rod(center) {
            return lightning_rod_target;
        }
        let search = AABB::new(center.x as f64, center.y as f64, center.z as f64, center.x as f64 + 1.0, (self.chunks.max_y() + 2) as f64, center.z as f64 + 1.0)
            .inflate(3.0, 3.0, 3.0);
        let targets: Vec<BlockPos> = self
            .players
            .values()
            .filter(|p| self.is_alive(p.entity.id) && p.entity.bounding_box().intersects(&search))
            .map(|p| p.entity.block_position())
            .filter(|&p| self.brightness(crate::world::level::light_layer::LightLayer::Sky, p) >= 15)
            .collect();
        if !targets.is_empty() {
            return targets[self.random.next_int_bounded(targets.len() as i32) as usize];
        }
        if center.y == self.chunks.min_y() - 1 {
            center = BlockPos::new(center.x, center.y + 2, center.z);
        }
        center
    }

    fn block_random_pos(&mut self, x: i32, y: i32, z: i32, y_mask: i32) -> BlockPos {
        self.rand_value = self.rand_value.wrapping_mul(3).wrapping_add(1013904223);
        let val = self.rand_value >> 2;
        BlockPos::new(x + (val & 15), y + (val >> 16 & y_mask), z + (val >> 8 & 15))
    }

    fn tick_chunk(&mut self, pos: ChunkPos, tick_speed: i32) {
        let (min_x, min_z) = (pos.min_block_x(), pos.min_block_z());
        for _ in 0..tick_speed {
            if self.random.next_int_bounded(48) == 0 {
                let pos = self.block_random_pos(min_x, 0, min_z, 15);
                self.tick_precipitation(pos);
            }
        }
        if tick_speed <= 0 {
            return;
        }
        let mut index = 0;
        loop {
            let Some(chunk) = self.chunks.loaded_chunk(pos) else { return };
            let Some(next) = (index..chunk.sections.len()).find(|&i| chunk.sections[i].is_randomly_ticking()) else { return };
            index = next + 1;
            let min_y = self.chunks.min_y() + next as i32 * 16;
            for _ in 0..tick_speed {
                let block_pos = self.block_random_pos(min_x, min_y, min_z, 15);
                let Some(state) = self.chunks.loaded_chunk(pos).map(|c| c.block_state(block_pos)) else { return };
                if state.is_randomly_ticking() {
                    block::of(state).random_tick(state, self, block_pos);
                }
                let fluid = state.fluid_state();
                if fluid.is_randomly_ticking() {
                    crate::world::level::material::random_tick(self, block_pos, fluid);
                }
            }
        }
    }

    pub(crate) fn common_tick(&mut self, id: EntityId) {
        let e = self.entity_mut(id);
        e.set_old_pos();
        e.tick_count += 1;
    }

    fn tick_player_entity(&mut self, id: EntityId) {
        self.player_mut(id).movement.tick_client_load_timeout();
        self.tick_game_mode(id);
        let living = &mut self.player_mut(id).living;
        if living.damage_cooldown_time > 0 {
            living.damage_cooldown_time -= 1;
        }
        self.tick_container_menu(id);
        self.tick_camera(id);
        criteria_triggers::TICK.trigger(self, id);
        if let Some(levitation_start_pos) = self.player(id).levitation_start_pos {
            let duration = self.entity(id).tick_count - self.player(id).levitation_start_time;
            criteria_triggers::LEVITATION.trigger(self, id, levitation_start_pos, duration);
        }
        let p = self.player_mut(id);
        if p.entity.fall_distance > 0.0 && p.starting_to_fall_position.is_none() {
            p.starting_to_fall_position = Some(p.entity.position());
        }
        p.update_player_attributes();
        let p = self.player_mut(id);
        p.advancements.flush_dirty(&p.connection, true);
        if p.post_effects_dirty {
            p.send_post_effects();
        }
    }

    fn tick_entity(&mut self, id: EntityId) {
        match self.entities.get(&id) {
            Some(LevelEntity::Item(_)) => self.tick_item(id),
            Some(LevelEntity::LightningBolt(_)) => self.tick_lightning_bolt(id),
            Some(LevelEntity::EyeOfEnder(_)) => self.tick_eye_of_ender(id),
            Some(LevelEntity::EndCrystal(_)) => self.tick_end_crystal(id),
            Some(LevelEntity::PrimedTnt(_)) => self.tick_primed_tnt(id),
            Some(LevelEntity::FallingBlock(_)) => self.tick_falling_block(id),
            Some(LevelEntity::ExperienceOrb(_)) => self.tick_experience_orb(id),
            Some(LevelEntity::Mob(_)) => self.mob_tick(id),
            None => return,
        }
        if self.entities.get(&id).is_some_and(|e| e.entity().is_removed()) {
            self.remove_entity(id);
        }
    }

    pub(crate) fn merge_with_neighbours(&mut self, id: EntityId) {
        let Some(item) = self.entities[&id].as_item() else { return };
        if !item.is_mergeable() {
            return;
        }
        let area = item.merge_area();
        for other in self.items_in(&area, Some(id)) {
            let [Some(LevelEntity::Item(this)), Some(LevelEntity::Item(that))] = self.entities.get_disjoint_mut([&id, &other]) else { continue };
            if !that.is_mergeable() {
                continue;
            }
            ItemEntity::try_to_merge(this, that);
            if that.entity.is_removed() {
                self.remove_entity(other);
            }
            if self.entities[&id].entity().is_removed() {
                self.remove_entity(id);
                break;
            }
        }
    }

    pub fn pick_up_items(&mut self, player: EntityId) {
        let area = self.player(player).entity.bounding_box().inflate(1.0, 0.5, 1.0);
        for id in self.items_in(&area, None) {
            let Some(LevelEntity::Item(item)) = self.entities.get_mut(&id) else { continue };
            if item.pickup_delay != 0 || item.target.is_some_and(|t| t != self.players[&player].profile.uuid) {
                continue;
            }
            let original = item.item.count;
            let stack = item.item.clone();
            let picked_up_item = stack.get_item();
            let left = self.player_mut(player).pick_up(stack);
            if left == original {
                continue;
            }
            let packet = encode(clientbound::TAKE_ITEM_ENTITY, |p| {
                p.varint(id);
                p.varint(player);
                p.varint(original);
            });
            self.send_to_tracking_players(id, &packet);
            let Some(LevelEntity::Item(item)) = self.entities.get_mut(&id) else { continue };
            item.item.count = left;
            if left == 0 {
                item.entity.discard();
                item.item.count = original;
            }
            let (picked, thrower) = (item.item.clone(), item.thrower);
            if left == 0 {
                self.remove_entity(id);
            }
            self.award_stat(player, crate::stats::stats::ITEM_PICKED_UP.get(picked_up_item.0), original);
            if let Some(thrower) = thrower.and_then(|uuid| self.entity_manager.visible_entity_storage.get(uuid)) {
                criteria_triggers::THROWN_ITEM_PICKED_UP_BY_PLAYER.trigger(self, player, &picked, thrower);
            }
        }
    }

    pub fn drop_from_player(&mut self, player: EntityId, stack: ItemStack, randomly: bool) {
        let item = self.create_item_stack_to_drop(player, stack, randomly, false);
        self.add_fresh_entity(LevelEntity::Item(item));
    }

    pub fn create_item_stack_to_drop(&mut self, player: EntityId, stack: ItemStack, randomly: bool, thrown_from_hand: bool) -> ItemEntity {
        let p = self.player_mut(player);
        let uuid = p.profile.uuid;
        let e = &mut p.entity;
        let y = e.eye_y() - 0.3f32 as f64;
        let mut item = ItemEntity::new(Vec3::new(e.x(), y, e.z()), stack);
        item.pickup_delay = 40;
        if thrown_from_hand {
            item.thrower = Some(uuid);
        }
        if randomly {
            let pow = e.random.next_float() * 0.5;
            let dir = e.random.next_float() * (std::f32::consts::PI * 2.0);
            item.entity.delta_movement = Vec3::new((-mth::sin(dir as f64) * pow) as f64, 0.2f32 as f64, (mth::cos(dir as f64) * pow) as f64);
        } else {
            let sin_x = mth::sin((e.x_rot * mth::DEG_TO_RAD) as f64);
            let cos_x = mth::cos((e.x_rot * mth::DEG_TO_RAD) as f64);
            let sin_y = mth::sin((e.y_rot * mth::DEG_TO_RAD) as f64);
            let cos_y = mth::cos((e.y_rot * mth::DEG_TO_RAD) as f64);
            let dir = e.random.next_float() * (std::f32::consts::PI * 2.0);
            let pow2 = 0.02 * e.random.next_float();
            let jitter = (e.random.next_float() - e.random.next_float()) * 0.1;
            item.entity.delta_movement = Vec3::new(
                (-sin_y * cos_x * 0.3) as f64 + (dir as f64).cos() * pow2 as f64,
                (-sin_x * 0.3 + 0.1 + jitter) as f64,
                (cos_y * cos_x * 0.3) as f64 + (dir as f64).sin() * pow2 as f64,
            );
        }
        item
    }
}

impl BlockGetter for ServerLevel {
    fn block_state(&mut self, pos: BlockPos) -> BlockState {
        self.chunks.block_state(pos)
    }

    fn get_min_y(&self) -> i32 {
        self.chunks.min_y()
    }
}

impl crate::world::level::LevelReader for ServerLevel {
    fn raw_brightness(&mut self, pos: BlockPos, darkening: i32) -> i32 {
        ServerLevel::raw_brightness(self, pos, darkening)
    }

    fn get_brightness(&mut self, layer: crate::world::level::light_layer::LightLayer, pos: BlockPos) -> i32 {
        ServerLevel::brightness(self, layer, pos)
    }

    fn get_sea_level(&self) -> i32 {
        ServerLevel::sea_level(self)
    }

    fn is_inside_build_height(&self, y: i32) -> bool {
        ServerLevel::is_inside_build_height(self, y)
    }

    fn is_fast_lava(&self) -> bool {
        crate::world::level::material::lava_fluid::is_fast_lava(self)
    }

    fn get_control_input_signal(&mut self, pos: BlockPos, direction: crisp_core::Direction, only_diodes: bool) -> i32 {
        ServerLevel::get_control_input_signal(self, pos, direction, only_diodes)
    }

    fn get_block_entity(&mut self, pos: BlockPos) -> Option<&mut crate::world::level::block::entity::BlockEntity> {
        ServerLevel::get_block_entity(self, pos)
    }
}

impl crate::world::level::ScheduledTickAccess for ServerLevel {
    fn schedule_tick_with_priority(&mut self, pos: BlockPos, block: Block, delay: i32, priority: TickPriority) {
        ServerLevel::schedule_tick_with_priority(self, pos, block, delay, priority);
    }

    fn schedule_fluid_tick(&mut self, pos: BlockPos, fluid: crisp_data::Fluid, delay: i32) {
        ServerLevel::schedule_fluid_tick(self, pos, fluid, delay);
    }

    fn has_scheduled_tick(&self, pos: BlockPos, block: Block) -> bool {
        ServerLevel::has_scheduled_tick(self, pos, block)
    }
}

impl crate::world::level::LevelWriter for ServerLevel {
    fn set_block(&mut self, pos: BlockPos, state: BlockState, flags: i32) -> bool {
        ServerLevel::set_block(self, pos, state, flags)
    }
}

impl crate::world::level::LevelAccessor for ServerLevel {
    fn get_random(&mut self) -> &mut dyn crisp_core::random::RandomSource {
        &mut self.random
    }

    fn mark_pos_for_post_processing(&mut self, pos: BlockPos) {
        warn!("Trying to mark a block for post processing @ MutableBlockPos{{x={}, y={}, z={}}}, but this operation is not supported.", pos.x, pos.y, pos.z);
    }

    fn play_sound(&mut self, pos: BlockPos, sound: i32, source: SoundSource, volume: f32, pitch: f32) {
        ServerLevel::play_sound(self, None, pos, sound, source, volume, pitch);
    }
}

impl ServerLevel {
    pub fn with_level_random<T>(&mut self, f: impl FnOnce(&mut ServerLevel, &mut crisp_core::random::LegacyRandomSource) -> T) -> T {
        let mut random = self.random.clone();
        let result = f(self, &mut random);
        self.random = random;
        result
    }
}

impl crate::world::level::world_gen_level::WorldGenLevel for ServerLevel {
    fn get_seed(&self) -> i64 {
        self.seed
    }

    fn ensure_can_write(&self, _pos: BlockPos) -> bool {
        true
    }

    fn generator(&self) -> &crate::world::level::chunk::ChunkGenerator {
        &self.chunks.generator
    }

    fn get_difficulty(&self) -> crate::world::difficulty::Difficulty {
        self.difficulty
    }

    fn generation(&self) -> crate::world::level::levelgen::material::WorldGenerationContext {
        let generator = &self.chunks.generator;
        crate::world::level::levelgen::material::WorldGenerationContext {
            min_gen_y: self.chunks.min_y().max(generator.min_y()),
            gen_depth: self.chunks.height().min(generator.gen_depth()),
            sea_level: generator.sea_level(),
        }
    }

    fn get_block_state(&mut self, pos: BlockPos) -> BlockState {
        self.chunks.block_state(pos)
    }

    fn get_height(&mut self, kind: crate::world::level::chunk::HeightmapType, x: i32, z: i32) -> i32 {
        if (-30_000_000..30_000_000).contains(&x) && (-30_000_000..30_000_000).contains(&z) {
            match self.chunks.loaded_chunk_mut(ChunkPos::new(x >> 4, z >> 4)) {
                Some(chunk) => chunk.get_height(kind, x, z) + 1,
                None => self.chunks.min_y(),
            }
        } else {
            self.sea_level() + 1
        }
    }

    fn get_height_of_level(&self) -> i32 {
        self.chunks.height()
    }

    fn get_biome(&mut self, pos: BlockPos) -> crate::world::level::chunk::Biome {
        ServerLevel::get_biome(self, pos)
    }

    fn set_section_block_state(&mut self, pos: BlockPos, state: BlockState) {
        let chunk = self.chunks.chunk(ChunkPos::new(pos.x >> 4, pos.z >> 4));
        let index = ((pos.y - chunk.min_y()) >> 4) as usize;
        chunk.sections[index].set_block_state((pos.x & 15) as usize, (pos.y & 15) as usize, (pos.z & 15) as usize, state);
    }

    fn destroy_block(&mut self, pos: BlockPos, drop_resources: bool) -> bool {
        ServerLevel::destroy_block(self, pos, drop_resources, None, 512)
    }

    fn add_fresh_entity(&mut self, entity: LevelEntity) -> bool {
        ServerLevel::add_fresh_entity(self, entity)
    }
}

pub fn legacy_block(fluid: crisp_data::FluidState) -> BlockState {
    use crisp_data::Fluid;
    use crisp_data::properties::LEVEL;
    match fluid.fluid {
        Fluid::Empty => blocks::AIR.default_state(),
        Fluid::Water | Fluid::FlowingWater => blocks::WATER.default_state().set(LEVEL, fluid_block_level(fluid)),
        Fluid::Lava | Fluid::FlowingLava => blocks::LAVA.default_state().set(LEVEL, fluid_block_level(fluid)),
    }
}

fn fluid_block_level(fluid: crisp_data::FluidState) -> i32 {
    if fluid.is_source() { 0 } else { 8 - (fluid.amount as i32).min(8) + if fluid.falling { 8 } else { 0 } }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SoundSource {
    Master,
    Music,
    Records,
    Weather,
    Blocks,
    Hostile,
    Neutral,
    Players,
    Ambient,
    Voice,
    Ui,
}

impl SoundSource {
    pub const VALUES: [SoundSource; 11] = [
        SoundSource::Master,
        SoundSource::Music,
        SoundSource::Records,
        SoundSource::Weather,
        SoundSource::Blocks,
        SoundSource::Hostile,
        SoundSource::Neutral,
        SoundSource::Players,
        SoundSource::Ambient,
        SoundSource::Voice,
        SoundSource::Ui,
    ];

    pub fn name(self) -> &'static str {
        match self {
            SoundSource::Master => "master",
            SoundSource::Music => "music",
            SoundSource::Records => "record",
            SoundSource::Weather => "weather",
            SoundSource::Blocks => "block",
            SoundSource::Hostile => "hostile",
            SoundSource::Neutral => "neutral",
            SoundSource::Players => "player",
            SoundSource::Ambient => "ambient",
            SoundSource::Voice => "voice",
            SoundSource::Ui => "ui",
        }
    }

    pub fn by_name(name: &str) -> Option<SoundSource> {
        SoundSource::VALUES.into_iter().find(|s| s.name() == name)
    }
}

fn chunk_tickets_file(dir: &WorldDir, dimension: &str) -> std::path::PathBuf {
    dir.dimension_data_folder(dimension).join("minecraft").join("chunk_tickets.dat")
}

fn get_type_count<'a>(values: impl Iterator<Item = &'a str>) -> String {
    let mut count_by_type: crisp_core::fastutil::Object2ObjectOpenHashMap<i32> = crisp_core::fastutil::Object2ObjectOpenHashMap::new(16, 0.75);
    for ty in values {
        *count_by_type.get_or_insert_with(ty, || 0) += 1;
    }
    let mut entries: Vec<(&str, i32)> = count_by_type.stream().map(|(ty, &count)| (ty, count)).collect();
    entries.sort_by(|a, b| b.1.cmp(&a.1));
    entries.iter().take(5).map(|(ty, count)| format!("{ty}:{count}")).collect::<Vec<_>>().join(",")
}

#[cfg(test)]
pub mod testing {
    use crisp_core::{BlockPos, Vec3};

    use super::ServerLevel;
    use super::chunk_cache::ChunkCache;
    use super::server_player::ServerPlayer;
    use crate::network::configuration::ClientInfo;
    use crate::network::connection::Outbound;
    use crate::network::login::Profile;
    use crate::world::entity::EntityId;
    use crate::world::level::chunk::FlatLevelSource;

    pub fn level() -> ServerLevel {
        let dimension = &crisp_data::DIMENSION_TYPES[0];
        let chunks = ChunkCache::new(dimension, FlatLevelSource::default(), None);
        ServerLevel::new(super::super::OVERWORLD, 0, chunks, None, 10, 10)
    }

    pub fn player(level: &mut ServerLevel, pos: Vec3, y_rot: f32, x_rot: f32) -> EntityId {
        player_with_profile(level, Profile { uuid: 1, name: "Tester".into(), properties: Vec::new() }, pos, y_rot, x_rot)
    }

    pub fn player_with_profile(level: &mut ServerLevel, profile: Profile, pos: Vec3, y_rot: f32, x_rot: f32) -> EntityId {
        let mut player = ServerPlayer::new(profile, ClientInfo::default(), Outbound::detached(), BlockPos::ZERO);
        player.entity.set_pos(pos);
        player.entity.y_rot = y_rot;
        player.entity.x_rot = x_rot;
        let id = player.entity.id;
        level.add_player(player);
        level.finish_chunk_work();
        id
    }

    pub fn server_player(server: &mut crate::server::Server, pos: Vec3, y_rot: f32, x_rot: f32) -> EntityId {
        server_player_with_profile(server, Profile { uuid: 1, name: "Tester".into(), properties: Vec::new() }, pos, y_rot, x_rot)
    }

    pub fn server_player_with_profile(server: &mut crate::server::Server, profile: Profile, pos: Vec3, y_rot: f32, x_rot: f32) -> EntityId {
        let id = player_with_profile(server.overworld_mut(), profile, pos, y_rot, x_rot);
        server.player_list.push(id);
        id
    }
}
