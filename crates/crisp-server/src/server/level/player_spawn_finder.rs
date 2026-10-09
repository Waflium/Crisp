use crisp_core::{AABB, BlockPos, ChunkPos, Direction, Vec3, mth};
use rand::RngExt;

use super::ServerLevel;
use super::chunk_cache::ChunkCache;
use super::ticket::TicketType;
use crate::world::level::BlockGetter;
use crate::world::level::chunk::HeightmapType;

const ABSOLUTE_MAX_ATTEMPTS: i64 = 1024;
const PLAYER_WIDTH: f64 = 0.6;
const PLAYER_HEIGHT: f64 = 1.8;

impl ServerLevel {
    pub fn find_spawn(&mut self, spawn_suggestion: BlockPos, adventure: bool) -> Vec3 {
        if adventure {
            return self.fixup_spawn_height(spawn_suggestion);
        }
        let mut radius = self.game_rules.get_int(crisp_data::game_rule::RESPAWN_RADIUS).max(0);
        let dist_to_border = mth::floor(self.world_border.get_distance_to_border(spawn_suggestion.x as f64, spawn_suggestion.z as f64));
        if dist_to_border < radius {
            radius = dist_to_border;
        }
        if dist_to_border <= 1 {
            radius = 1;
        }
        let square_side = radius as i64 * 2 + 1;
        let candidate_count = ABSOLUTE_MAX_ATTEMPTS.min(square_side * square_side) as i32;
        let coprime = if candidate_count <= 16 { candidate_count - 1 } else { 17 };
        let offset = rand::rng().random_range(0..candidate_count);
        for candidate_index in 0..candidate_count {
            let value = (offset + coprime * candidate_index) % candidate_count;
            let target_x = spawn_suggestion.x + value % (radius * 2 + 1) - radius;
            let target_z = spawn_suggestion.z + value / (radius * 2 + 1) - radius;
            self.chunks.add_ticket_with_radius(TicketType::SpawnSearch, ChunkPos::new(target_x >> 4, target_z >> 4), 0);
            if let Some(spawn_pos) = level_respawn_pos(&mut self.chunks, target_x, target_z)
                && self.no_collision_no_liquid(spawn_pos)
            {
                return Vec3::at_bottom_center_of(spawn_pos);
            }
        }
        self.chunks.add_ticket_with_radius(TicketType::SpawnSearch, spawn_suggestion.chunk(), 0);
        self.fixup_spawn_height(spawn_suggestion)
    }

    fn fixup_spawn_height(&mut self, spawn_pos: BlockPos) -> Vec3 {
        let mut pos = spawn_pos;
        while !self.no_collision_no_liquid(pos) && pos.y < self.chunks.max_y() {
            pos = pos.above();
        }
        pos = pos.below();
        while self.no_collision_no_liquid(pos) && pos.y > self.chunks.min_y() {
            pos = pos.below();
        }
        Vec3::at_bottom_center_of(pos.above())
    }

    fn no_collision_no_liquid(&self, pos: BlockPos) -> bool {
        let (x, y, z) = (pos.x as f64 + 0.5, pos.y as f64, pos.z as f64 + 0.5);
        let half_width = PLAYER_WIDTH / 2.0;
        self.no_block_and_liquid_collision(&AABB::new(x - half_width, y, z - half_width, x + half_width, y + PLAYER_HEIGHT, z + half_width))
    }
}

pub fn initial_spawn(chunks: &mut ChunkCache) -> BlockPos {
    let spawn_chunk = chunks.generator.origin();
    let mut height = chunks.generator.spawn_height(chunks.min_y(), chunks.height());
    if height < chunks.min_y() {
        let (x, z) = (spawn_chunk.min_block_x() + 8, spawn_chunk.min_block_z() + 8);
        height = chunks.chunk(spawn_chunk).height_at(HeightmapType::WorldSurface, x, z);
    }
    let mut spawn = BlockPos::new(spawn_chunk.min_block_x() + 8, height, spawn_chunk.min_block_z() + 8);
    let (mut x_chunk_offset, mut z_chunk_offset, mut dx_chunk, mut dz_chunk) = (0, 0, 0, -1);
    for _ in 0..11 * 11 {
        if (-5..=5).contains(&x_chunk_offset)
            && (-5..=5).contains(&z_chunk_offset)
            && let Some(tested_pos) = spawn_pos_in_chunk(chunks, ChunkPos::new(spawn_chunk.x + x_chunk_offset, spawn_chunk.z + z_chunk_offset))
        {
            spawn = tested_pos;
            break;
        }
        if x_chunk_offset == z_chunk_offset || (x_chunk_offset < 0 && x_chunk_offset == -z_chunk_offset) || (x_chunk_offset > 0 && x_chunk_offset == 1 - z_chunk_offset) {
            (dx_chunk, dz_chunk) = (-dz_chunk, dx_chunk);
        }
        x_chunk_offset += dx_chunk;
        z_chunk_offset += dz_chunk;
    }
    spawn
}

fn spawn_pos_in_chunk(chunks: &mut ChunkCache, pos: ChunkPos) -> Option<BlockPos> {
    for x in pos.min_block_x()..pos.min_block_x() + 16 {
        for z in pos.min_block_z()..pos.min_block_z() + 16 {
            if let Some(spawn) = level_respawn_pos(chunks, x, z) {
                return Some(spawn);
            }
        }
    }
    None
}

fn level_respawn_pos(chunks: &mut ChunkCache, x: i32, z: i32) -> Option<BlockPos> {
    let min_y = chunks.min_y();
    let chunk = chunks.chunk(ChunkPos::new(x >> 4, z >> 4));
    let top = chunk.height_at(HeightmapType::MotionBlocking, x, z);
    if top < min_y {
        return None;
    }
    let surface = chunk.height_at(HeightmapType::WorldSurface, x, z);
    if surface <= top && surface > chunk.height_at(HeightmapType::OceanFloor, x, z) {
        return None;
    }
    for y in (min_y..=top + 1).rev() {
        let pos = BlockPos::new(x, y, z);
        let state = chunks.block_state(pos);
        if !state.fluid_state().is_empty() {
            break;
        }
        if crisp_core::shapes::is_face_full(state.collision_voxel_shape(), Direction::Up) {
            return Some(BlockPos::new(x, y + 1, z));
        }
    }
    None
}
