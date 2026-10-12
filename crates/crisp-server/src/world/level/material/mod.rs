pub mod flowing_fluid;
pub mod lava_fluid;
pub mod map_color;
pub mod water_fluid;

use std::sync::Mutex;

use crisp_core::{AABB, BlockPos, Direction};
use crisp_data::{BlockState, Fluid, FluidState, fluid_tags};

use crate::server::level::ServerLevel;
use crate::world::level::{BlockGetter, LevelReader};
pub use flowing_fluid::FlowingFluid;
use lava_fluid::LavaFluid;
use water_fluid::WaterFluid;

pub fn flowing(fluid: Fluid) -> Option<&'static dyn FlowingFluid> {
    match fluid {
        Fluid::Empty => None,
        Fluid::Water | Fluid::FlowingWater => Some(&WaterFluid),
        Fluid::Lava | Fluid::FlowingLava => Some(&LavaFluid),
    }
}

pub fn tick(level: &mut ServerLevel, pos: BlockPos, state: BlockState) {
    let fluid = state.fluid_state();
    if let Some(behaviour) = flowing(fluid.fluid) {
        behaviour.tick(level, pos, state, fluid);
    }
}

pub fn random_tick(level: &mut ServerLevel, pos: BlockPos, fluid: FluidState) {
    if let Some(behaviour) = flowing(fluid.fluid) {
        behaviour.random_tick(level, pos, fluid);
    }
}

pub fn create_legacy_block(fluid: FluidState) -> BlockState {
    crate::server::level::legacy_block(fluid)
}

pub fn tick_delay(level: &dyn LevelReader, fluid: Fluid) -> i32 {
    flowing(fluid).map_or(0, |f| f.tick_delay(level))
}

fn has_same_above(level: &mut ServerLevel, fluid: FluidState, pos: BlockPos) -> bool {
    fluid.fluid.is_same(level.fluid_state(pos.above()).fluid)
}

pub fn height(level: &mut ServerLevel, fluid: FluidState, pos: BlockPos) -> f32 {
    if fluid.is_empty() {
        return 0.0;
    }
    if has_same_above(level, fluid, pos) { 1.0 } else { fluid.own_height() }
}

static SHAPES: Mutex<Vec<(Fluid, u8, bool, f32)>> = Mutex::new(Vec::new());

pub fn shape(level: &mut ServerLevel, fluid: FluidState, pos: BlockPos) -> Vec<AABB> {
    if fluid.is_empty() {
        return Vec::new();
    }
    if fluid.amount == 9 && has_same_above(level, fluid, pos) {
        return vec![AABB::new(0.0, 0.0, 0.0, 1.0, 1.0, 1.0)];
    }
    let key = (fluid.fluid, fluid.amount, fluid.falling);
    let cached = SHAPES.lock().unwrap().iter().find(|s| (s.0, s.1, s.2) == key).map(|s| s.3);
    let height = match cached {
        Some(height) => height,
        None => {
            let height = height(level, fluid, pos);
            SHAPES.lock().unwrap().push((key.0, key.1, key.2, height));
            height
        }
    };
    vec![AABB::new(0.0, 0.0, 0.0, 1.0, height as f64, 1.0)]
}

pub fn can_be_replaced_with(level: &mut ServerLevel, fluid: FluidState, pos: BlockPos, other: Fluid, direction: Direction) -> bool {
    match fluid.fluid {
        Fluid::Empty => true,
        Fluid::Water | Fluid::FlowingWater => direction == Direction::Down && !other.is(&fluid_tags::WATER),
        Fluid::Lava | Fluid::FlowingLava => height(level, fluid, pos) >= 0.44444445 && other.is(&fluid_tags::WATER),
    }
}

pub fn pickup_sound(fluid: Fluid) -> Option<i32> {
    let name = match fluid {
        Fluid::Empty => return None,
        Fluid::Water | Fluid::FlowingWater => "minecraft:item.bucket.fill",
        Fluid::Lava | Fluid::FlowingLava => "minecraft:item.bucket.fill_lava",
    };
    Some(crisp_data::registry_id(crisp_data::registry::SOUND_EVENT, name))
}

#[cfg(test)]
mod tests {
    use crisp_core::{BlockPos, Vec3};
    use crisp_data::properties::LEVEL;
    use crisp_data::{BlockState, blocks};

    use crate::server::level::{ServerLevel, testing};
    use crate::world::level::BlockGetter;
    use crate::world::level::update_flags::UPDATE_ALL;

    fn level() -> ServerLevel {
        let mut level = testing::level();
        testing::player(&mut level, Vec3::new(40.5, -60.0, 40.5), 0.0, 0.0);
        level
    }

    fn run(level: &mut ServerLevel, ticks: i32) {
        for _ in 0..ticks {
            level.tick();
        }
    }

    fn water(level: i32) -> BlockState {
        blocks::WATER.default_state().set(LEVEL, level)
    }

    fn lava(level: i32) -> BlockState {
        blocks::LAVA.default_state().set(LEVEL, level)
    }

    fn air() -> BlockState {
        blocks::AIR.default_state()
    }

    #[test]
    fn water_spreads_seven_blocks_one_level_per_five_ticks() {
        let mut level = level();
        let source = BlockPos::new(0, -60, 0);
        level.set_block(source, water(0), UPDATE_ALL);
        run(&mut level, 4);
        assert_eq!(level.block_state(source.east()), air());
        for distance in 1..=7i32 {
            run(&mut level, 1);
            for x in -distance..=distance {
                let z = distance - x.abs();
                for pos in [BlockPos::new(x, -60, z), BlockPos::new(x, -60, -z)] {
                    assert_eq!(level.block_state(pos), water(distance), "{pos:?} at distance {distance}");
                }
            }
            assert_eq!(level.block_state(BlockPos::new(distance + 1, -60, 0)), air());
            run(&mut level, 4);
        }
        run(&mut level, 30);
        assert_eq!(level.block_state(source), water(0));
        assert_eq!(level.block_state(BlockPos::new(7, -60, 0)), water(7));
        assert_eq!(level.block_state(BlockPos::new(-3, -60, 4)), water(7));
        assert_eq!(level.block_state(BlockPos::new(8, -60, 0)), air());
        assert_eq!(level.block_state(BlockPos::new(4, -60, 4)), air());
        assert_eq!(level.block_state(BlockPos::new(0, -59, 0)), air());
    }

    #[test]
    fn falling_water_lands_and_spreads() {
        let mut level = level();
        let source = BlockPos::new(0, -55, 0);
        for side in [source.north(), source.south(), source.west(), source.east()] {
            level.set_block(side, blocks::STONE.default_state(), UPDATE_ALL);
        }
        level.set_block(source, water(0), UPDATE_ALL);
        run(&mut level, 5);
        assert_eq!(level.block_state(source.below()), water(8));
        assert_eq!(level.block_state(source.below().below()), air());
        run(&mut level, 19);
        assert_eq!(level.block_state(BlockPos::new(0, -59, 0)), water(8));
        assert_eq!(level.block_state(BlockPos::new(0, -60, 0)), air());
        run(&mut level, 1);
        assert_eq!(level.block_state(BlockPos::new(0, -60, 0)), water(8));
        assert_eq!(level.block_state(BlockPos::new(1, -60, 0)), air());
        run(&mut level, 5);
        for pos in [BlockPos::new(1, -60, 0), BlockPos::new(-1, -60, 0), BlockPos::new(0, -60, 1), BlockPos::new(0, -60, -1)] {
            assert_eq!(level.block_state(pos), water(1));
        }
        run(&mut level, 30);
        assert_eq!(level.block_state(BlockPos::new(7, -60, 0)), water(7));
        assert_eq!(level.block_state(BlockPos::new(8, -60, 0)), air());
        assert_eq!(level.block_state(BlockPos::new(1, -59, 0)), air());
    }

    #[test]
    fn water_flows_only_towards_the_nearest_hole() {
        let mut level = level();
        let hole = BlockPos::new(3, -61, 0);
        level.set_block(hole, air(), UPDATE_ALL);
        level.set_block(BlockPos::new(0, -60, 0), water(0), UPDATE_ALL);
        for (ticks, x) in [(5, 1), (5, 2), (5, 3)] {
            run(&mut level, ticks);
            assert_eq!(level.block_state(BlockPos::new(x, -60, 0)), water(x));
            assert_eq!(level.block_state(BlockPos::new(x + 1, -60, 0)), air());
            assert_eq!(level.block_state(BlockPos::new(x - 1, -60, 1)), air());
            assert_eq!(level.block_state(BlockPos::new(x - 1, -60, -1)), air());
        }
        assert_eq!(level.block_state(BlockPos::new(-1, -60, 0)), air());
        assert_eq!(level.block_state(hole), air());
        run(&mut level, 5);
        assert_eq!(level.block_state(hole), water(8));
    }

    #[test]
    fn water_between_two_sources_becomes_a_source() {
        let mut level = level();
        level.set_block(BlockPos::new(0, -60, 0), water(0), UPDATE_ALL);
        level.set_block(BlockPos::new(2, -60, 0), water(0), UPDATE_ALL);
        run(&mut level, 4);
        assert_eq!(level.block_state(BlockPos::new(1, -60, 0)), air());
        run(&mut level, 1);
        assert_eq!(level.block_state(BlockPos::new(1, -60, 0)), water(0));
        assert_eq!(level.block_state(BlockPos::new(0, -60, 1)), water(1));
        run(&mut level, 40);
        assert_eq!(level.block_state(BlockPos::new(1, -60, 0)), water(0));
        assert_eq!(level.block_state(BlockPos::new(0, -60, 1)), water(1));
    }

    #[test]
    fn lava_spreads_three_blocks_every_thirty_ticks() {
        let mut level = level();
        let source = BlockPos::new(0, -60, 0);
        level.set_block(source, lava(0), UPDATE_ALL);
        run(&mut level, 29);
        assert_eq!(level.block_state(source.east()), air());
        run(&mut level, 1);
        assert_eq!(level.block_state(source.east()), lava(2));
        assert_eq!(level.block_state(source.east().east()), air());
        run(&mut level, 400);
        assert_eq!(level.block_state(BlockPos::new(2, -60, 0)), lava(4));
        assert_eq!(level.block_state(BlockPos::new(3, -60, 0)), lava(6));
        assert_eq!(level.block_state(BlockPos::new(4, -60, 0)), air());
    }

    #[test]
    fn lava_touching_water_hardens() {
        let mut level = level();
        let lava_source = BlockPos::new(0, -60, 0);
        level.set_block(lava_source, lava(0), UPDATE_ALL);
        run(&mut level, 30);
        assert_eq!(level.block_state(BlockPos::new(1, -60, 0)), lava(2));
        level.set_block(BlockPos::new(2, -60, 0), water(0), UPDATE_ALL);
        assert_eq!(level.block_state(BlockPos::new(1, -60, 0)), blocks::COBBLESTONE.default_state());
        assert_eq!(level.block_state(lava_source), lava(0));
        level.set_block(BlockPos::new(0, -60, 1), water(0), UPDATE_ALL);
        assert_eq!(level.block_state(lava_source), blocks::OBSIDIAN.default_state());
    }

    #[test]
    fn lava_flowing_down_into_water_makes_stone() {
        let mut level = level();
        let water_pos = BlockPos::new(0, -60, 0);
        level.set_block(water_pos, water(0), UPDATE_ALL);
        level.set_block(BlockPos::new(0, -58, 0), lava(0), UPDATE_ALL);
        run(&mut level, 30);
        assert_eq!(level.block_state(BlockPos::new(0, -59, 0)), lava(8));
        run(&mut level, 29);
        assert_eq!(level.block_state(water_pos), water(0));
        run(&mut level, 1);
        assert_eq!(level.block_state(water_pos), blocks::STONE.default_state());
    }
}
