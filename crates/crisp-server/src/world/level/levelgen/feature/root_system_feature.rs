use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, Direction};
use crisp_data::{BlockState, fluid_tags};
use serde_json::Value;

use super::stateproviders::block_state_provider::BlockStateProviderHolder;
use crate::server::level::world_gen_region::WorldGenRegion;
use crate::world::level::block;
use crate::world::level::chunk::HeightmapType;
use crate::world::level::levelgen::blockpredicates::block_predicate::{BlockPredicate, BlockSet};
use crate::world::level::levelgen::placement::placed_feature::PlacedFeatureHolder;

pub struct RootSystemFeature {
    tree_feature: PlacedFeatureHolder,
    required_vertical_space_for_tree: i32,
    level_test_distance: i32,
    max_level_deviation: i32,
    root_radius: i32,
    root_replaceable: BlockSet,
    root_state_provider: BlockStateProviderHolder,
    root_placement_attempts: i32,
    root_column_max_height: i32,
    hanging_root_radius: i32,
    hanging_roots_vertical_span: i32,
    hanging_root_state_provider: BlockStateProviderHolder,
    hanging_root_placement_attempts: i32,
    allowed_vertical_water_for_tree: i32,
    allowed_tree_position: BlockPredicate,
}

impl RootSystemFeature {
    pub fn parse(json: &Value) -> Result<RootSystemFeature, String> {
        let field = |key: &str| json.get(key).ok_or_else(|| format!("missing {key}"));
        let int = |key: &str| field(key)?.as_i64().map(|v| v as i32).ok_or_else(|| format!("bad {key}"));
        Ok(RootSystemFeature {
            tree_feature: PlacedFeatureHolder::parse(field("feature")?)?,
            required_vertical_space_for_tree: int("required_vertical_space_for_tree")?,
            level_test_distance: int("level_test_distance")?,
            max_level_deviation: int("max_level_deviation")?,
            root_radius: int("root_radius")?,
            root_replaceable: BlockSet::parse(field("root_replaceable")?)?,
            root_state_provider: BlockStateProviderHolder::parse(field("root_state_provider")?)?,
            root_placement_attempts: int("root_placement_attempts")?,
            root_column_max_height: int("root_column_max_height")?,
            hanging_root_radius: int("hanging_root_radius")?,
            hanging_roots_vertical_span: int("hanging_roots_vertical_span")?,
            hanging_root_state_provider: BlockStateProviderHolder::parse(field("hanging_root_state_provider")?)?,
            hanging_root_placement_attempts: int("hanging_root_placement_attempts")?,
            allowed_vertical_water_for_tree: int("allowed_vertical_water_for_tree")?,
            allowed_tree_position: BlockPredicate::parse(field("allowed_tree_position")?)?,
        })
    }

    pub fn place<R: RandomSource>(&self, level: &mut WorldGenRegion, random: &mut R, origin: BlockPos) -> bool {
        if !level.get_block_state(origin).is_air() {
            return false;
        }
        let mut working_pos = origin;
        if self.place_dirt_and_tree(level, random, &mut working_pos, origin) {
            self.place_roots(level, random, origin);
        }
        true
    }

    fn space_for_tree(&self, level: &mut WorldGenRegion, pos: BlockPos) -> bool {
        let mut column_up_pos = pos;
        for i in 1..=self.required_vertical_space_for_tree {
            column_up_pos = column_up_pos.above();
            let state = level.get_block_state(column_up_pos);
            if !is_allowed_tree_space(state, i, self.allowed_vertical_water_for_tree) {
                return false;
            }
        }
        if self.level_test_distance > 0 {
            for i in 0..4 {
                let corner_pos = pos.relative_n(Direction::from_2d_data_value(i), self.level_test_distance);
                let below = level.get_block_state(corner_pos.offset(0, -self.max_level_deviation, 0));
                let above = level.get_block_state(corner_pos.offset(0, self.max_level_deviation, 0));
                if below.is_air() || !above.is_air() {
                    return false;
                }
            }
        }
        true
    }

    fn place_dirt_and_tree<R: RandomSource>(&self, level: &mut WorldGenRegion, random: &mut R, working_pos: &mut BlockPos, pos: BlockPos) -> bool {
        for y in 0..self.root_column_max_height {
            *working_pos = working_pos.above();
            if level.get_height(HeightmapType::WorldSurface, working_pos.x, working_pos.z) < working_pos.y {
                return false;
            }
            if self.allowed_tree_position.test(level, *working_pos) && self.space_for_tree(level, *working_pos) {
                let below_pos = working_pos.below();
                if level.get_fluid_state(below_pos).is(&fluid_tags::LAVA) || !level.get_block_state(below_pos).is_solid() {
                    return false;
                }
                if self.tree_feature.value().place(level, random, *working_pos) {
                    self.place_dirt(pos, pos.y + y, level, random);
                    return true;
                }
            }
        }
        false
    }

    fn place_dirt<R: RandomSource>(&self, origin: BlockPos, target_height: i32, level: &mut WorldGenRegion, random: &mut R) {
        for y in origin.y..target_height {
            self.place_rooted_dirt(level, random, origin.x, origin.z, y);
        }
    }

    fn place_rooted_dirt<R: RandomSource>(&self, level: &mut WorldGenRegion, random: &mut R, origin_x: i32, origin_z: i32, y: i32) {
        for _ in 0..self.root_placement_attempts {
            let dx = random.next_int_bounded(self.root_radius) - random.next_int_bounded(self.root_radius);
            let dz = random.next_int_bounded(self.root_radius) - random.next_int_bounded(self.root_radius);
            let working_pos = BlockPos::new(origin_x + dx, y, origin_z + dz);
            if self.root_replaceable.contains(level.get_block_state(working_pos).block()) {
                let state = self.root_state_provider.value().get_state(level, random, working_pos);
                level.set_block(working_pos, state, 2);
            }
        }
    }

    fn place_roots<R: RandomSource>(&self, level: &mut WorldGenRegion, random: &mut R, pos: BlockPos) {
        for _ in 0..self.hanging_root_placement_attempts {
            let dx = random.next_int_bounded(self.hanging_root_radius) - random.next_int_bounded(self.hanging_root_radius);
            let dy = random.next_int_bounded(self.hanging_roots_vertical_span) - random.next_int_bounded(self.hanging_roots_vertical_span);
            let dz = random.next_int_bounded(self.hanging_root_radius) - random.next_int_bounded(self.hanging_root_radius);
            let working_pos = pos.offset(dx, dy, dz);
            if level.get_block_state(working_pos).is_air() {
                let target_state = self.hanging_root_state_provider.value().get_state(level, random, working_pos);
                if block::of(target_state).can_survive(target_state, level, working_pos) && level.get_block_state(working_pos.above()).is_face_sturdy_full(Direction::Down) {
                    level.set_block(working_pos, target_state, 2);
                }
            }
        }
    }
}

fn is_allowed_tree_space(state: BlockState, blocks_above_origin: i32, allowed_vertical_water_height: i32) -> bool {
    if state.is_air() {
        return true;
    }
    let blocks_above_ground = blocks_above_origin + 1;
    blocks_above_ground <= allowed_vertical_water_height && state.fluid_state().is(&fluid_tags::WATER)
}
