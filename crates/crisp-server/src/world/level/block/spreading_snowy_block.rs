use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, Direction};
use crisp_data::properties::{LAYERS, SNOWY};
use crisp_data::{Block, BlockState, blocks, fluid_tags};

use super::BlockBehaviour;
use super::snowy_block;
use super::vegetation_block::{block_field, fields};
use crate::server::level::ServerLevel;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::{BlockGetter, LevelAccessor};
use crate::world::level::lighting::light_dampening_into;
use crate::world::level::update_flags::UPDATE_ALL;

pub struct SpreadingSnowyBlock {
    base_block: Block,
}

impl SpreadingSnowyBlock {
    pub fn new(block: Block) -> SpreadingSnowyBlock {
        SpreadingSnowyBlock { base_block: block_field(&fields(block), "baseBlock") }
    }
}

fn can_stay_alive(state: BlockState, level: &mut ServerLevel, pos: BlockPos) -> bool {
    let above_state = level.block_state(pos.above());
    if above_state.is_block(blocks::SNOW) && above_state.get(LAYERS) == 1 {
        return true;
    }
    if above_state.fluid_state().amount == 8 {
        return false;
    }
    light_dampening_into(state, above_state, Direction::Up, above_state.light_dampening() as i32) < 15
}

fn can_propagate(state: BlockState, level: &mut ServerLevel, pos: BlockPos) -> bool {
    can_stay_alive(state, level, pos) && !level.fluid_state(pos.above()).is(&fluid_tags::WATER)
}

impl BlockBehaviour for SpreadingSnowyBlock {
    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        Some(snowy_block::get_state_for_placement(block, ctx, level))
    }

    fn update_shape(&self, state: BlockState, _level: &mut dyn LevelAccessor, _pos: BlockPos, direction: Direction, _: BlockPos, neighbour_state: BlockState) -> BlockState {
        snowy_block::update_shape(state, direction, neighbour_state)
    }

    fn random_tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        if !can_stay_alive(state, level, pos) {
            level.set_block(pos, self.base_block.default_state(), UPDATE_ALL);
        } else if level.max_local_raw_brightness(pos.above(), level.sky_darken()) >= 9 {
            let default_state = state.block().default_state();
            for _ in 0..4 {
                let dx = level.random.next_int_bounded(3) - 1;
                let dy = level.random.next_int_bounded(5) - 3;
                let dz = level.random.next_int_bounded(3) - 1;
                let test_pos = pos.offset(dx, dy, dz);
                if level.block_state(test_pos).is_block(self.base_block) && can_propagate(default_state, level, test_pos) {
                    let snowy = snowy_block::is_snowy_setting(level.block_state(test_pos.above()));
                    level.set_block(test_pos, default_state.set(SNOWY, snowy), UPDATE_ALL);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crisp_core::BlockPos;
    use crisp_core::random::LegacyRandomSource;
    use crisp_data::blocks;
    use crisp_data::properties::{SLAB_TYPE, SNOWY, SlabType};

    use crate::server::level::testing;
    use crate::world::level::BlockGetter;
    use crate::world::level::block;
    use crate::world::level::update_flags::UPDATE_ALL;

    const GRASS: BlockPos = BlockPos::new(0, -61, 0);

    fn random_tick(level: &mut crate::server::level::ServerLevel, pos: BlockPos) {
        let state = level.block_state(pos);
        block::of(state).random_tick(state, level, pos);
    }

    #[test]
    fn grass_is_snowy_under_snow() {
        let mut level = testing::level();
        level.set_block(GRASS.above(), blocks::SNOW.default_state(), UPDATE_ALL);
        assert!(level.block_state(GRASS).get(SNOWY));
        level.set_block(GRASS.above(), blocks::AIR.default_state(), UPDATE_ALL);
        assert!(!level.block_state(GRASS).get(SNOWY));
    }

    #[test]
    fn grass_dies_under_blocks_that_close_its_top_face() {
        let mut level = testing::level();
        let slab = blocks::STONE_SLAB.default_state();
        level.set_block(GRASS.above(), slab.set(SLAB_TYPE, SlabType::Top), UPDATE_ALL);
        random_tick(&mut level, GRASS);
        assert!(level.block_state(GRASS).is_block(blocks::GRASS_BLOCK));
        level.set_block(GRASS.above(), slab.set(SLAB_TYPE, SlabType::Bottom), UPDATE_ALL);
        random_tick(&mut level, GRASS);
        assert!(level.block_state(GRASS).is_block(blocks::DIRT));
    }

    #[test]
    fn grass_spreads_to_dirt_in_light() {
        let mut level = testing::level();
        for x in -1..=1 {
            for z in -1..=1 {
                if x != 0 || z != 0 {
                    level.set_block(GRASS.offset(x, 0, z), blocks::DIRT.default_state(), UPDATE_ALL);
                }
            }
        }
        level.random = LegacyRandomSource::new(3);
        for _ in 0..50 {
            random_tick(&mut level, GRASS);
        }
        let spread = (-1..=1).flat_map(|x| (-1..=1).map(move |z| GRASS.offset(x, 0, z))).filter(|&p| p != GRASS);
        assert!(spread.into_iter().any(|p| level.block_state(p).is_block(blocks::GRASS_BLOCK)));
    }
}
