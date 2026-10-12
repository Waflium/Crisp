use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, Direction};
use crisp_data::properties::STAGE;
use crisp_data::{Block, BlockState};

use super::BlockBehaviour;
use super::grower::tree_grower::TreeGrower;
use super::vegetation_block::{self, Vegetation, fields};
use crate::server::level::ServerLevel;
use crate::world::level::{LevelAccessor, LevelReader};
use crate::world::level::update_flags::UPDATE_NONE;

pub fn advance_tree(level: &mut ServerLevel, pos: BlockPos, state: BlockState, tree_grower: &TreeGrower) {
    if state.get(STAGE) == 0 {
        level.set_block(pos, state.cycle(STAGE), UPDATE_NONE);
    } else {
        tree_grower.grow_tree(level, pos, state);
    }
}

pub fn is_valid_bonemeal_target(level: &mut ServerLevel, pos: BlockPos, state: BlockState, tree_grower: &TreeGrower) -> bool {
    if !tree_grower.can_grow(level, pos, state) {
        return false;
    }
    let height_offset = tree_grower.get_minimum_height().unwrap_or(0);
    level.is_inside_build_height(pos.relative_n(Direction::Up, height_offset).y)
}

pub fn is_bonemeal_success(level: &mut ServerLevel) -> bool {
    (level.random.next_float() as f64) < 0.45
}

pub struct SaplingBlock {
    tree_grower: &'static TreeGrower,
}

impl SaplingBlock {
    pub fn new(block: Block) -> SaplingBlock {
        let tree_grower = fields(block)["treeGrower"].as_str().and_then(TreeGrower::by_name).expect("saplings have a tree grower");
        SaplingBlock { tree_grower }
    }
}

impl Vegetation for SaplingBlock {}

impl BlockBehaviour for SaplingBlock {
    fn update_shape(&self, state: BlockState, level: &mut dyn LevelAccessor, pos: BlockPos, _: Direction, _: BlockPos, _: BlockState) -> BlockState {
        vegetation_block::update_shape(state, level, pos)
    }

    fn can_survive(&self, _state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> bool {
        vegetation_block::can_survive(self, level, pos)
    }

    fn random_tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        if level.max_local_raw_brightness(pos.above(), level.sky_darken()) >= 9 && level.random.next_int_bounded(7) == 0 {
            advance_tree(level, pos, state, self.tree_grower);
        }
    }

    fn is_valid_bonemeal_target(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState) -> bool {
        is_valid_bonemeal_target(level, pos, state, self.tree_grower)
    }

    fn is_bonemeal_success(&self, level: &mut ServerLevel, _pos: BlockPos, _state: BlockState) -> bool {
        is_bonemeal_success(level)
    }

    fn perform_bonemeal(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState) {
        advance_tree(level, pos, state, self.tree_grower);
    }
}

#[cfg(test)]
mod tests {
    use crisp_core::BlockPos;
    use crisp_data::{block_tags, blocks};

    use crate::server::level::testing;
    use crate::world::level::BlockGetter;
    use crate::world::level::block;
    use crate::world::level::update_flags::UPDATE_ALL;

    #[test]
    fn a_grown_oak_sapling_becomes_a_tree() {
        let mut level = testing::level();
        let pos = BlockPos::new(0, -60, 0);
        level.set_block(pos, blocks::OAK_SAPLING.default_state(), UPDATE_ALL);
        for _ in 0..64 {
            let state = level.block_state(pos);
            if !state.is_block(blocks::OAK_SAPLING) {
                break;
            }
            block::of(state).perform_bonemeal(&mut level, pos, state);
        }
        assert!(level.block_state(pos).is(&block_tags::LOGS));
        assert!(level.block_state(pos.below()).is_block(blocks::DIRT));
    }
}
