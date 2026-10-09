use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, Direction, mth};
use crisp_data::properties::AGE_3;
use crisp_data::{Block, BlockState, blocks};

use super::BlockBehaviour;
use super::ice_block::{IceBlock, melt};
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;
use crate::world::level::BlockGetter;
use crate::world::level::light_layer::LightLayer;

const MAX_AGE: i32 = 3;
const NEIGHBORS_TO_AGE: i32 = 4;
const NEIGHBORS_TO_MELT: i32 = 2;

pub struct FrostedIceBlock;

impl FrostedIceBlock {
    fn slightly_melt(level: &mut ServerLevel, pos: BlockPos, state: BlockState) -> bool {
        let age = state.get(AGE_3);
        if age < MAX_AGE {
            level.set_block(pos, state.set(AGE_3, age + 1), 2);
            false
        } else {
            melt(level, pos);
            true
        }
    }

    fn fewer_neighbours_than(level: &mut ServerLevel, pos: BlockPos, limit: i32) -> bool {
        let mut result = 0;
        for direction in Direction::VALUES {
            if level.block_state(pos.relative(direction)).is_block(blocks::FROSTED_ICE) {
                result += 1;
                if result >= limit {
                    return false;
                }
            }
        }
        true
    }
}

impl BlockBehaviour for FrostedIceBlock {
    fn on_place(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos, _old_state: BlockState, _moved_by_piston: bool) {
        let delay = mth::next_int(&mut level.random, 60, 120);
        level.schedule_tick(pos, blocks::FROSTED_ICE, delay);
    }

    fn tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        if level.random.next_int_bounded(3) == 0 || Self::fewer_neighbours_than(level, pos, NEIGHBORS_TO_AGE) {
            let brightness = if level.dimension == "minecraft:the_end" {
                level.brightness(LightLayer::Block, pos)
            } else {
                level.max_local_raw_brightness(pos, level.sky_darken())
            };
            if brightness > 11 - state.get(AGE_3) - state.light_dampening() as i32 && Self::slightly_melt(level, pos, state) {
                for direction in Direction::VALUES {
                    let neighbor_pos = pos.relative(direction);
                    let neighbour = level.block_state(neighbor_pos);
                    if neighbour.is_block(blocks::FROSTED_ICE) && !Self::slightly_melt(level, neighbor_pos, neighbour) {
                        let delay = mth::next_int(&mut level.random, 20, 40);
                        level.schedule_tick(neighbor_pos, blocks::FROSTED_ICE, delay);
                    }
                }
                return;
            }
        }
        let delay = mth::next_int(&mut level.random, 20, 40);
        level.schedule_tick(pos, blocks::FROSTED_ICE, delay);
    }

    fn neighbor_changed(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos, block: Block, _moved_by_piston: bool) {
        if block == blocks::FROSTED_ICE && Self::fewer_neighbours_than(level, pos, NEIGHBORS_TO_MELT) {
            melt(level, pos);
        }
    }

    fn random_tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        IceBlock.random_tick(state, level, pos);
    }

    fn player_destroy(&self, level: &mut ServerLevel, player: EntityId, pos: BlockPos, state: BlockState, _block_entity: Option<&crate::world::level::block::entity::BlockEntity>, tool: Option<ItemStack>) {
        IceBlock.player_destroy(level, player, pos, state, None, tool);
    }
}

#[cfg(test)]
mod tests {
    use crisp_core::BlockPos;
    use crisp_data::blocks;
    use crisp_data::properties::AGE_3;

    use crate::server::level::testing;
    use crate::world::level::BlockGetter;
    use crate::world::level::block;
    use crate::world::level::update_flags::UPDATE_ALL;

    #[test]
    fn frosted_ice_ages_in_daylight_and_melts() {
        let mut level = testing::level();
        let pos = BlockPos::new(0, -60, 0);
        let ice = blocks::FROSTED_ICE.default_state();
        level.set_block(pos, ice, UPDATE_ALL);
        assert!(level.has_scheduled_tick(pos, blocks::FROSTED_ICE));
        for age in 1..=3 {
            let state = level.block_state(pos);
            block::of(state).tick(state, &mut level, pos);
            assert_eq!(level.block_state(pos), ice.set(AGE_3, age));
        }
        let state = level.block_state(pos);
        block::of(state).tick(state, &mut level, pos);
        assert_eq!(level.block_state(pos), blocks::WATER.default_state());
    }
}
