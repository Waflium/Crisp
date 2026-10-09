use crisp_core::BlockPos;
use crisp_data::properties::WATERLOGGED;
use crisp_data::{BlockState, Fluid, FluidState, items};

use crate::server::level::ServerLevel;
use crate::world::item::ItemStack;
use crate::world::level::block;
use crate::world::level::material::tick_delay;
use crate::world::level::update_flags::UPDATE_ALL;

pub fn can_place_liquid(fluid: Fluid) -> bool {
    fluid == Fluid::Water
}

pub fn place_liquid(level: &mut ServerLevel, pos: BlockPos, state: BlockState, fluid: FluidState) -> bool {
    if !state.get(WATERLOGGED) && fluid.fluid == Fluid::Water {
        level.set_block(pos, state.set(WATERLOGGED, true), UPDATE_ALL);
        level.schedule_fluid_tick(pos, fluid.fluid, tick_delay(level, fluid.fluid));
        true
    } else {
        false
    }
}

pub fn pickup_block(level: &mut ServerLevel, pos: BlockPos, state: BlockState) -> Option<ItemStack> {
    if !state.get(WATERLOGGED) {
        return None;
    }
    level.set_block(pos, state.set(WATERLOGGED, false), UPDATE_ALL);
    if !block::of(state).can_survive(state, level, pos) {
        level.destroy_block(pos, true, None, crate::world::level::update_flags::UPDATE_LIMIT);
    }
    Some(ItemStack::new(items::WATER_BUCKET, 1))
}

pub fn is_water_source(level: &mut ServerLevel, pos: BlockPos) -> bool {
    use crate::world::level::BlockGetter;
    level.fluid_state(pos).fluid == Fluid::Water
}

pub fn schedule_water_tick(state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
    if state.get(WATERLOGGED) {
        let delay = tick_delay(level, Fluid::Water);
        level.schedule_fluid_tick(pos, Fluid::Water, delay);
    }
}
