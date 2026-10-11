use crisp_core::shapes::{BooleanOp, VoxelShape, join_unoptimized};
use crisp_core::{AABB, Axis, BlockPos, ChunkPos, Vec3};
use crisp_data::properties::LAYERS;
use crisp_data::{BlockState, blocks};

use super::ServerLevel;
use crate::world::entity::relative;
use crate::world::entity::relative::PositionMoveRotation;
use crate::world::level::biome::{self, Precipitation};
use crate::world::level::block;
use crate::world::level::chunk::{Biome, ChunkGenerator, HeightmapType};
use crate::world::level::light_layer::LightLayer;
use crate::world::level::update_flags::UPDATE_ALL;
use crate::world::level::BlockGetter;

impl ServerLevel {
    pub fn get_biome(&self, pos: BlockPos) -> Biome {
        let (x, y, z) = biome::zoomed_quart(self.biome_zoom_seed, pos.x, pos.y, pos.z);
        self.get_noise_biome(x, y, z)
    }

    pub fn get_noise_biome(&self, quart_x: i32, quart_y: i32, quart_z: i32) -> Biome {
        let chunk = ChunkPos::new(quart_x >> 2, quart_z >> 2);
        if let Some(chunk) = self.chunks.loaded_chunk(chunk) {
            return chunk.noise_biome(quart_x, quart_y, quart_z);
        }
        match &self.chunks.generator {
            ChunkGenerator::Flat(flat) => flat.biome,
            ChunkGenerator::Noise(noise) => noise.chunk_biomes(chunk, self.chunks.dimension).get(quart_x, quart_y, quart_z),
        }
    }

    pub fn sea_level(&self) -> i32 {
        self.chunks.generator.sea_level()
    }

    pub fn heightmap_pos(&mut self, kind: HeightmapType, pos: BlockPos) -> BlockPos {
        let chunk = self.chunks.chunk(pos.chunk());
        BlockPos::new(pos.x, chunk.height_at(kind, pos.x, pos.z) + 1, pos.z)
    }

    pub fn can_see_sky(&mut self, pos: BlockPos) -> bool {
        self.brightness(LightLayer::Sky, pos) >= 15
    }

    pub fn is_inside_build_height(&self, y: i32) -> bool {
        y >= self.chunks.min_y() && y <= self.chunks.max_y()
    }

    pub fn precipitation_at(&mut self, pos: BlockPos) -> Precipitation {
        if !self.is_raining() || !self.can_see_sky(pos) || self.heightmap_pos(HeightmapType::MotionBlocking, pos).y > pos.y {
            return Precipitation::None;
        }
        let sea_level = self.sea_level();
        self.get_biome(pos).precipitation_at(pos, sea_level)
    }

    pub fn is_raining_at(&mut self, pos: BlockPos) -> bool {
        self.precipitation_at(pos) == Precipitation::Rain
    }

    pub(super) fn tick_precipitation(&mut self, pos: BlockPos) {
        let top_pos = self.heightmap_pos(HeightmapType::MotionBlocking, pos);
        let below_pos = top_pos.below();
        let biome = self.get_biome(top_pos);
        if biome.should_freeze(self, below_pos, true) {
            self.set_block(below_pos, blocks::ICE.default_state(), UPDATE_ALL);
        }
        if self.is_raining() {
            let max_height = self.game_rules.get_int(crisp_data::game_rule::MAX_SNOW_ACCUMULATION_HEIGHT);
            if max_height > 0 && biome.should_snow(self, top_pos) {
                let state = self.block_state(top_pos);
                if state.is_block(blocks::SNOW) {
                    let current_layers = state.get(LAYERS);
                    if current_layers < max_height.min(8) {
                        let new_state = state.set(LAYERS, current_layers + 1);
                        self.push_entities_up(state, new_state, top_pos);
                        self.set_block(top_pos, new_state, UPDATE_ALL);
                    }
                } else {
                    self.set_block(top_pos, blocks::SNOW.default_state(), UPDATE_ALL);
                }
            }
            let precipitation = biome.precipitation_at(below_pos, self.sea_level());
            if precipitation != Precipitation::None {
                let below_state = self.block_state(below_pos);
                block::of(below_state).handle_precipitation(below_state, self, below_pos, precipitation);
            }
        }
    }

    pub fn push_entities_up(&mut self, state: BlockState, new_state: BlockState, pos: BlockPos) {
        let offset_shape = join_unoptimized(&VoxelShape::from_boxes(state.collision_shape()), &VoxelShape::from_boxes(new_state.collision_shape()), BooleanOp::OnlySecond);
        if offset_shape.is_empty() {
            return;
        }
        let (x, y, z) = (pos.x as f64, pos.y as f64, pos.z as f64);
        let bounds = AABB::new(
            offset_shape.min(Axis::X) + x,
            offset_shape.min(Axis::Y) + y,
            offset_shape.min(Axis::Z) + z,
            offset_shape.max(Axis::X) + x,
            offset_shape.max(Axis::Y) + y,
            offset_shape.max(Axis::Z) + z,
        );
        let colliding: Vec<_> = self.all_entities().filter(|&id| self.entity(id).bounding_box().intersects(&bounds)).collect();
        for id in colliding {
            let moving = self.entity(id).bounding_box().move_vec(Vec3::new(0.0, 1.0, 0.0));
            let offset = offset_shape.collide([x, y, z], Axis::Y, &moving, -1.0);
            let dy = 1.0 + offset;
            if let Some(player) = self.players.get_mut(&id) {
                player.teleport_relative(PositionMoveRotation { position: Vec3::new(0.0, dy, 0.0), delta_movement: Vec3::ZERO, y_rot: 0.0, x_rot: 0.0 }, relative::relative::ALL);
            } else {
                let e = self.entity_mut(id);
                let pos = e.position();
                e.set_pos(Vec3::new(pos.x, pos.y + dy, pos.z));
                e.set_old_pos();
                self.entity_moved(id);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crisp_core::{BlockPos, Vec3};
    use crisp_data::blocks;

    use crate::server::level::ServerLevel;
    use crate::server::level::chunk_cache::ChunkCache;
    use crate::server::level::testing;
    use crate::world::level::BlockGetter;
    use crate::world::level::chunk::{Biome, FlatLevelSource};
    use crate::world::level::update_flags::UPDATE_ALL;

    fn snowy_level() -> ServerLevel {
        let source = FlatLevelSource { biome: Biome::by_name("minecraft:snowy_plains").unwrap(), ..FlatLevelSource::default() };
        let chunks = ChunkCache::new(&crisp_data::DIMENSION_TYPES[0], source, None);
        let mut level = ServerLevel::new(crate::server::OVERWORLD, 0, chunks, None, 10, 10);
        testing::player(&mut level, Vec3::new(8.5, -60.0, 8.5), 0.0, 0.0);
        level.weather.raining = true;
        level.rain_level = 1.0;
        level
    }

    #[test]
    fn snow_settles_and_water_freezes() {
        let mut level = snowy_level();
        level.tick_precipitation(BlockPos::new(0, 0, 0));
        assert_eq!(level.block_state(BlockPos::new(0, -60, 0)), blocks::SNOW.default_state());
        let water = BlockPos::new(3, -61, 3);
        level.set_block(water, blocks::WATER.default_state(), UPDATE_ALL);
        level.tick_precipitation(water);
        assert_eq!(level.block_state(water), blocks::ICE.default_state());
        assert!(level.block_state(water.above()).is_air());
        assert_eq!(level.precipitation_at(BlockPos::new(5, -70, 5)), crate::world::level::biome::Precipitation::None);
    }

    #[test]
    fn plains_get_rain() {
        let mut level = testing::level();
        testing::player(&mut level, Vec3::new(8.5, -60.0, 8.5), 0.0, 0.0);
        level.weather.raining = true;
        level.rain_level = 1.0;
        assert!(level.is_raining_at(BlockPos::new(1, -60, 1)));
        level.tick_precipitation(BlockPos::new(1, 0, 1));
        assert!(level.block_state(BlockPos::new(1, -60, 1)).is_air());
    }
}
