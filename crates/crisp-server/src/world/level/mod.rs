pub mod base_spawner;
pub mod biome;
pub mod block;
pub mod border;
pub mod chunk;
pub mod data_pack_config;
pub mod gamerules;
pub mod clip;
pub mod entity;
pub mod light_layer;
pub mod server_explosion;
pub mod collision;
pub mod levelgen;
pub mod lighting;
pub mod material;
pub mod moon_phase;
pub mod noise_column;
pub mod path_navigation_region;
pub mod pathfinder;
pub mod saveddata;
pub mod redstone;
pub mod signal;
pub mod storage;
pub mod timers;
pub mod validation;
pub mod world_data_configuration;
pub mod world_gen_level;

use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, Direction};
use crisp_data::{Block, BlockState, Fluid, FluidState};

use crate::world::ticks::TickPriority;

pub fn is_in_spawnable_bounds(pos: BlockPos) -> bool {
    (-20_000_000..20_000_000).contains(&pos.y) && (-30_000_000..30_000_000).contains(&pos.x) && (-30_000_000..30_000_000).contains(&pos.z)
}

pub trait BlockGetter {
    fn block_state(&mut self, pos: BlockPos) -> BlockState;

    fn fluid_state(&mut self, pos: BlockPos) -> FluidState {
        self.block_state(pos).fluid_state()
    }
}

pub trait LevelReader: BlockGetter {
    fn raw_brightness(&mut self, pos: BlockPos, darkening: i32) -> i32;

    fn get_brightness(&mut self, layer: light_layer::LightLayer, pos: BlockPos) -> i32;

    fn get_sea_level(&self) -> i32;

    fn is_inside_build_height(&self, y: i32) -> bool;

    fn is_fast_lava(&self) -> bool;

    fn get_control_input_signal(&mut self, pos: BlockPos, direction: Direction, only_diodes: bool) -> i32;

    fn get_block_entity(&mut self, pos: BlockPos) -> Option<&mut block::entity::BlockEntity>;

    fn is_water_at(&mut self, pos: BlockPos) -> bool {
        self.fluid_state(pos).is(&crisp_data::fluid_tags::WATER)
    }
}

pub trait ScheduledTickAccess {
    fn schedule_tick_with_priority(&mut self, pos: BlockPos, block: Block, delay: i32, priority: TickPriority);

    fn schedule_fluid_tick(&mut self, pos: BlockPos, fluid: Fluid, delay: i32);

    fn has_scheduled_tick(&self, pos: BlockPos, block: Block) -> bool;

    fn schedule_tick(&mut self, pos: BlockPos, block: Block, delay: i32) {
        self.schedule_tick_with_priority(pos, block, delay, TickPriority::NORMAL);
    }
}

pub trait LevelWriter {
    fn set_block(&mut self, pos: BlockPos, state: BlockState, flags: i32) -> bool;
}

pub trait LevelAccessor: LevelReader + ScheduledTickAccess + LevelWriter {
    fn get_random(&mut self) -> &mut dyn RandomSource;

    fn mark_pos_for_post_processing(&mut self, pos: BlockPos);

    fn play_sound(&mut self, _pos: BlockPos, _sound: i32, _source: crate::server::level::SoundSource, _volume: f32, _pitch: f32) {}
}

pub mod update_flags {
    pub const UPDATE_NEIGHBORS: i32 = 1;
    pub const UPDATE_CLIENTS: i32 = 2;
    pub const UPDATE_INVISIBLE: i32 = 4;
    pub const UPDATE_IMMEDIATE: i32 = 8;
    pub const UPDATE_KNOWN_SHAPE: i32 = 16;
    pub const UPDATE_SUPPRESS_DROPS: i32 = 32;
    pub const UPDATE_MOVE_BY_PISTON: i32 = 64;
    pub const UPDATE_SKIP_SHAPE_UPDATE_ON_WIRE: i32 = 128;
    pub const UPDATE_SKIP_BLOCK_ENTITY_SIDEEFFECTS: i32 = 256;
    pub const UPDATE_SKIP_ON_PLACE: i32 = 512;
    pub const UPDATE_NONE: i32 = UPDATE_INVISIBLE | UPDATE_SKIP_BLOCK_ENTITY_SIDEEFFECTS;
    pub const UPDATE_ALL: i32 = UPDATE_NEIGHBORS | UPDATE_CLIENTS;
    pub const UPDATE_ALL_IMMEDIATE: i32 = UPDATE_ALL | UPDATE_IMMEDIATE;
    pub const UPDATE_LIMIT: i32 = 512;
}
