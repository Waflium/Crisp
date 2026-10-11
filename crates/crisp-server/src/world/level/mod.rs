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
pub mod saveddata;
pub mod redstone;
pub mod signal;
pub mod storage;
pub mod timers;
pub mod validation;
pub mod world_data_configuration;

use crisp_core::BlockPos;
use crisp_data::{BlockState, FluidState};

pub fn is_in_spawnable_bounds(pos: BlockPos) -> bool {
    (-20_000_000..20_000_000).contains(&pos.y) && (-30_000_000..30_000_000).contains(&pos.x) && (-30_000_000..30_000_000).contains(&pos.z)
}

pub trait BlockGetter {
    fn block_state(&mut self, pos: BlockPos) -> BlockState;

    fn fluid_state(&mut self, pos: BlockPos) -> FluidState {
        self.block_state(pos).fluid_state()
    }
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
