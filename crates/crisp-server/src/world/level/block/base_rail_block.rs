use crisp_core::{BlockPos, Direction};
use crisp_data::properties::{Property, RAIL_SHAPE, RAIL_SHAPE_STRAIGHT, RailShape, WATERLOGGED};
use crisp_data::{Block, BlockState, Fluid, block_tags};

use super::can_support_rigid_block;
use super::rail_state::RailState;
use crate::server::level::ServerLevel;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::BlockGetter;

const WATER_TICK_DELAY: i32 = 5;

#[allow(unused_variables)]
pub trait BaseRailBlock {
    fn block(&self) -> Block;

    fn update_state_for_neighbor(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, block: Block) {}
}

pub fn is_straight(block: Block) -> bool {
    !block.is_instance_of("RailBlock")
}

pub fn shape_property(block: Block) -> Property<RailShape> {
    if block.is_instance_of("RailBlock") { RAIL_SHAPE } else { RAIL_SHAPE_STRAIGHT }
}

pub fn is_slope(shape: RailShape) -> bool {
    matches!(shape, RailShape::AscendingEast | RailShape::AscendingWest | RailShape::AscendingNorth | RailShape::AscendingSouth)
}

pub fn is_rail(state: BlockState) -> bool {
    state.is(&block_tags::RAILS) && state.block().is_instance_of("BaseRailBlock")
}

pub fn is_rail_at(level: &mut ServerLevel, pos: BlockPos) -> bool {
    is_rail(level.block_state(pos))
}

pub fn can_survive(level: &mut ServerLevel, pos: BlockPos) -> bool {
    can_support_rigid_block(level, pos.below())
}

pub fn on_place(rail: &(impl BaseRailBlock + ?Sized), state: BlockState, level: &mut ServerLevel, pos: BlockPos, old_state: BlockState, moved_by_piston: bool) {
    if !old_state.is_block(state.block()) {
        update_state(rail, state, level, pos, moved_by_piston);
    }
}

pub fn update_state(rail: &(impl BaseRailBlock + ?Sized), state: BlockState, level: &mut ServerLevel, pos: BlockPos, moved_by_piston: bool) -> BlockState {
    let state = update_dir(level, pos, state, true);
    if is_straight(rail.block()) {
        level.neighbor_changed_with_state(state, pos, rail.block(), moved_by_piston);
    }
    state
}

pub fn neighbor_changed(rail: &(impl BaseRailBlock + ?Sized), state: BlockState, level: &mut ServerLevel, pos: BlockPos, block: Block, moved_by_piston: bool) {
    if level.block_state(pos).is_block(rail.block()) {
        let shape = state.get(shape_property(rail.block()));
        if should_be_removed(pos, level, shape) {
            level.drop_resources(state, pos, None, None);
            level.remove_block(pos, moved_by_piston);
        } else {
            rail.update_state_for_neighbor(state, level, pos, block);
        }
    }
}

fn should_be_removed(pos: BlockPos, level: &mut ServerLevel, shape: RailShape) -> bool {
    if !can_support_rigid_block(level, pos.below()) {
        return true;
    }
    match shape {
        RailShape::AscendingEast => !can_support_rigid_block(level, pos.east()),
        RailShape::AscendingWest => !can_support_rigid_block(level, pos.west()),
        RailShape::AscendingNorth => !can_support_rigid_block(level, pos.north()),
        RailShape::AscendingSouth => !can_support_rigid_block(level, pos.south()),
        _ => false,
    }
}

pub fn update_dir(level: &mut ServerLevel, pos: BlockPos, state: BlockState, first: bool) -> BlockState {
    let current = state.get(shape_property(state.block()));
    let mut rail = RailState::new(pos, state);
    let has_signal = level.has_neighbor_signal(pos);
    rail.place(level, has_signal, first, current).state()
}

pub fn affect_neighbors_after_removal(rail: &(impl BaseRailBlock + ?Sized), state: BlockState, level: &mut ServerLevel, pos: BlockPos, moved_by_piston: bool) {
    if !moved_by_piston {
        if is_slope(state.get(shape_property(rail.block()))) {
            level.update_neighbors_at(pos.above(), rail.block());
        }
        if is_straight(rail.block()) {
            level.update_neighbors_at(pos, rail.block());
            level.update_neighbors_at(pos.below(), rail.block());
        }
    }
}

pub fn get_state_for_placement(block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> BlockState {
    let replaced_fluid_state = level.fluid_state(ctx.clicked_pos());
    let is_water_source = replaced_fluid_state.fluid == Fluid::Water;
    let direction = ctx.horizontal_direction();
    let is_east_west = direction == Direction::East || direction == Direction::West;
    let shape = if is_east_west { RailShape::EastWest } else { RailShape::NorthSouth };
    block.default_state().set(shape_property(block), shape).set(WATERLOGGED, is_water_source)
}

pub fn update_shape(state: BlockState, level: &mut ServerLevel, pos: BlockPos) -> BlockState {
    if state.get(WATERLOGGED) {
        level.schedule_fluid_tick(pos, Fluid::Water, WATER_TICK_DELAY);
    }
    state
}
