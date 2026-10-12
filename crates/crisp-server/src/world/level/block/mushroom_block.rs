use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, Direction};
use crisp_data::{Block, BlockState, block_tags};

use super::BlockBehaviour;
use super::vegetation_block::{self, Vegetation, at_most_matched, fields};
use crate::server::level::ServerLevel;
use crate::world::level::{BlockGetter, LevelAccessor, LevelReader};
use crate::world::level::levelgen::feature::feature::{self, Feature};
use crate::world::level::update_flags::{UPDATE_ALL, UPDATE_CLIENTS};

pub struct MushroomBlock {
    feature: Option<&'static Feature>,
}

fn random_offset(level: &mut ServerLevel, pos: BlockPos) -> BlockPos {
    let dx = level.random.next_int_bounded(3) - 1;
    let dy = level.random.next_int_bounded(2) - level.random.next_int_bounded(2);
    let dz = level.random.next_int_bounded(3) - 1;
    pos.offset(dx, dy, dz)
}

impl Vegetation for MushroomBlock {
    fn may_place_on(&self, state: BlockState, _level: &mut dyn LevelReader, _pos: BlockPos) -> bool {
        state.is_solid_render()
    }
}

impl BlockBehaviour for MushroomBlock {
    fn update_shape(&self, state: BlockState, level: &mut dyn LevelAccessor, pos: BlockPos, _: Direction, _: BlockPos, _: BlockState) -> BlockState {
        vegetation_block::update_shape(state, level, pos)
    }

    fn can_survive(&self, _state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> bool {
        let below_pos = pos.below();
        let below = level.block_state(below_pos);
        if below.is(&block_tags::OVERRIDES_MUSHROOM_LIGHT_REQUIREMENT) {
            return true;
        }
        level.raw_brightness(pos, 0) < 13 && self.may_place_on(below, level, below_pos)
    }

    fn random_tick(&self, state: BlockState, level: &mut ServerLevel, mut pos: BlockPos) {
        if level.random.next_int_bounded(25) != 0 {
            return;
        }
        if !at_most_matched(level, pos.offset(-4, -1, -4), pos.offset(4, 1, 4), state.block(), 4) {
            return;
        }
        let mut offset = random_offset(level, pos);
        for _ in 0..4 {
            if level.block_state(offset).is_air() && self.can_survive(state, level, offset) {
                pos = offset;
            }
            offset = random_offset(level, pos);
        }
        if level.block_state(offset).is_air() && self.can_survive(state, level, offset) {
            level.set_block(offset, state, UPDATE_CLIENTS);
        }
    }

    fn is_valid_bonemeal_target(&self, level: &mut ServerLevel, pos: BlockPos, _state: BlockState) -> bool {
        match self.feature {
            Some(Feature::HugeMushroom(mushroom_feature)) => {
                let min_height = 4 + mushroom_feature.foliage_radius;
                level.is_inside_build_height(pos.relative_n(Direction::Up, min_height).y)
            }
            _ => false,
        }
    }

    fn is_bonemeal_success(&self, level: &mut ServerLevel, _pos: BlockPos, _state: BlockState) -> bool {
        (level.random.next_float() as f64) < 0.4
    }

    fn perform_bonemeal(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState) {
        self.grow_mushroom(level, pos, state);
    }
}

impl MushroomBlock {
    pub fn new(block: Block) -> MushroomBlock {
        let fields = fields(block);
        MushroomBlock { feature: feature::feature(fields["feature"].as_str().expect("mushrooms grow into a feature")) }
    }

    pub fn grow_mushroom(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState) -> bool {
        let Some(feature) = self.feature else { return false };
        level.remove_block(pos, false);
        if level.with_level_random(|level, random| feature.place(level, random, pos)) {
            return true;
        }
        level.set_block(pos, state, UPDATE_ALL);
        false
    }
}
