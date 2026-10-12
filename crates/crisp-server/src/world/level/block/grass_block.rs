use std::sync::LazyLock;

use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, Direction};
use crisp_data::{Block, BlockState, blocks};

use super::BlockBehaviour;
use super::spreading_snowy_block::SpreadingSnowyBlock;
use crate::server::level::ServerLevel;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::biome::biome_generation_settings;
use crate::world::level::levelgen::feature::feature::FeatureHolder;
use crate::world::level::levelgen::placement::placed_feature::{self, placed_feature_index};
use crate::world::level::{BlockGetter, LevelAccessor};

static GRASS_BONEMEAL: LazyLock<Option<u32>> = LazyLock::new(|| placed_feature_index("minecraft:grass_bonemeal"));

pub struct GrassBlock {
    spreading: SpreadingSnowyBlock,
}

impl GrassBlock {
    pub fn new(block: Block) -> GrassBlock {
        GrassBlock { spreading: SpreadingSnowyBlock::new(block) }
    }

    fn stop_bonemeal_spread(&self, level: &mut ServerLevel, test_pos: BlockPos, block: Block) -> bool {
        !level.block_state(test_pos.below()).is_block(block) || level.block_state(test_pos).is_collision_shape_full_block()
    }
}

fn place_bonemeal_effect(level: &mut ServerLevel, test_pos: BlockPos) {
    let grass = blocks::SHORT_GRASS.default_state();
    let test_state = level.block_state(test_pos);
    if test_state.is_block(grass.block()) && level.random.next_float() < 0.1 {
        let bonemealable_block = super::of(grass);
        if bonemealable_block.is_valid_bonemeal_target(level, test_pos, test_state) {
            bonemealable_block.perform_bonemeal(level, test_pos, test_state);
        }
    }
    if test_state.is_air() && !level.chunks.is_outside_build_height(test_pos.y) {
        if level.random.next_float() < 0.125 {
            let biome = level.get_biome(test_pos);
            let features = biome_generation_settings::get(biome).get_bone_meal_features();
            if features.is_empty() {
                return;
            }
            let placement_feature = FeatureHolder::Reference(features[level.random.next_int_bounded(features.len() as i32) as usize]);
            level.with_level_random(|level, random| placement_feature.value().place(level, random, test_pos));
        } else if let Some(grass_feature) = *GRASS_BONEMEAL {
            level.with_level_random(|level, random| placed_feature::get(grass_feature).place(level, random, test_pos));
        }
    }
}

impl BlockBehaviour for GrassBlock {
    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        self.spreading.get_state_for_placement(block, ctx, level)
    }

    fn update_shape(
        &self,
        state: BlockState,
        level: &mut dyn LevelAccessor,
        pos: BlockPos,
        direction: Direction,
        neighbour_pos: BlockPos,
        neighbour_state: BlockState,
    ) -> BlockState {
        self.spreading.update_shape(state, level, pos, direction, neighbour_pos, neighbour_state)
    }

    fn random_tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        self.spreading.random_tick(state, level, pos);
    }

    fn is_valid_bonemeal_target(&self, level: &mut ServerLevel, pos: BlockPos, _state: BlockState) -> bool {
        level.block_state(pos.above()).is_air() && level.is_inside_build_height(pos.y + 1)
    }

    fn is_bonemeal_success(&self, _level: &mut ServerLevel, _pos: BlockPos, _state: BlockState) -> bool {
        true
    }

    fn perform_bonemeal(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState) {
        let above = pos.above();
        'attempts: for attempt in 0..128 {
            let mut test_pos = above;
            let randomize_count = attempt / 16;
            for _ in 0..randomize_count {
                let dx = level.random.next_int_bounded(3) - 1;
                let dy = (level.random.next_int_bounded(3) - 1) * level.random.next_int_bounded(3) / 2;
                let dz = level.random.next_int_bounded(3) - 1;
                test_pos = test_pos.offset(dx, dy, dz);
                if self.stop_bonemeal_spread(level, test_pos, state.block()) {
                    continue 'attempts;
                }
            }
            place_bonemeal_effect(level, test_pos);
        }
    }
}
