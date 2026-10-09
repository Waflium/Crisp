use std::collections::{HashSet, VecDeque};

use crisp_core::{BlockPos, Direction};
use crisp_data::{Block, BlockState, blocks, fluid_tags, registry, registry_id};

use super::BlockBehaviour;
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::level::BlockGetter;
use crate::world::level::block;
use crate::world::level::update_flags::UPDATE_ALL;

const MAX_DEPTH: i32 = 6;
const MAX_COUNT: i32 = 65;

pub struct SpongeBlock;

#[derive(PartialEq, Eq)]
enum TraversalNodeStatus {
    Accept,
    Skip,
}

impl SpongeBlock {
    fn try_absorb_water(level: &mut ServerLevel, pos: BlockPos) {
        if Self::remove_water_breadth_first_search(level, pos) {
            level.set_block(pos, blocks::WET_SPONGE.default_state(), 2);
            let sound = registry_id(registry::SOUND_EVENT, "minecraft:block.sponge.absorb");
            level.play_sound(None, pos, sound, SoundSource::Blocks, 1.0, 1.0);
        }
    }

    fn remove_water_breadth_first_search(level: &mut ServerLevel, start_pos: BlockPos) -> bool {
        breadth_first_traversal(start_pos, MAX_DEPTH, MAX_COUNT, |pos| {
            if pos == start_pos {
                return TraversalNodeStatus::Accept;
            }
            let state = level.block_state(pos);
            let fluid = level.fluid_state(pos);
            if !fluid.is(&fluid_tags::WATER) {
                return TraversalNodeStatus::Skip;
            }
            let behaviour = block::of(state);
            if behaviour.is_bucket_pickup(state) && behaviour.pickup_block(None, level, pos, state).is_some() {
                return TraversalNodeStatus::Accept;
            }
            if state.block().is_instance_of("LiquidBlock") {
                level.set_block(pos, blocks::AIR.default_state(), UPDATE_ALL);
            } else {
                let plants: [Block; 4] = [blocks::KELP, blocks::KELP_PLANT, blocks::SEAGRASS, blocks::TALL_SEAGRASS];
                if !plants.iter().any(|&b| state.is_block(b)) {
                    return TraversalNodeStatus::Skip;
                }
                level.drop_resources(state, pos, None, None);
                level.set_block(pos, blocks::AIR.default_state(), UPDATE_ALL);
            }
            TraversalNodeStatus::Accept
        }) > 1
    }
}

fn breadth_first_traversal(start_pos: BlockPos, max_depth: i32, max_count: i32, mut process: impl FnMut(BlockPos) -> TraversalNodeStatus) -> i32 {
    let mut nodes: VecDeque<(BlockPos, i32)> = VecDeque::from([(start_pos, 0)]);
    let mut visited: HashSet<i64> = HashSet::new();
    let mut count = 0;
    while let Some((pos, depth)) = nodes.pop_front() {
        if !visited.insert(pos.as_long()) || process(pos) == TraversalNodeStatus::Skip {
            continue;
        }
        count += 1;
        if count >= max_count {
            return count;
        }
        if depth < max_depth {
            for direction in Direction::VALUES {
                nodes.push_back((pos.relative(direction), depth + 1));
            }
        }
    }
    count
}

impl BlockBehaviour for SpongeBlock {
    fn on_place(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, old_state: BlockState, _moved_by_piston: bool) {
        if !old_state.is_block(state.block()) {
            Self::try_absorb_water(level, pos);
        }
    }

    fn neighbor_changed(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos, _block: Block, _moved_by_piston: bool) {
        Self::try_absorb_water(level, pos);
    }
}

#[cfg(test)]
mod tests {
    use crisp_core::{BlockPos, Vec3};
    use crisp_data::blocks;
    use crisp_data::properties::LEVEL;

    use crate::server::level::testing;
    use crate::world::level::BlockGetter;
    use crate::world::level::update_flags::UPDATE_ALL;

    #[test]
    fn sponge_soaks_up_a_pool_and_turns_wet() {
        let mut level = testing::level();
        testing::player(&mut level, Vec3::new(40.5, -60.0, 40.5), 0.0, 0.0);
        level.set_block(BlockPos::new(0, -60, 0), blocks::WATER.default_state(), UPDATE_ALL);
        for _ in 0..20 {
            level.tick();
        }
        assert_eq!(level.block_state(BlockPos::new(4, -60, 0)), blocks::WATER.default_state().set(LEVEL, 4));
        let sponge = BlockPos::new(0, -61, 0);
        level.set_block(sponge, blocks::SPONGE.default_state(), UPDATE_ALL);
        assert_eq!(level.block_state(sponge), blocks::WET_SPONGE.default_state());
        for _ in 0..20 {
            level.tick();
        }
        for x in -6..=6 {
            for z in -6..=6 {
                assert!(level.block_state(BlockPos::new(x, -60, z)).is_air(), "water left at {x}, {z}");
            }
        }
    }
}
