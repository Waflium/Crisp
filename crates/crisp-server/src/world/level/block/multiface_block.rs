use crisp_core::{BlockPos, Direction};
use crisp_data::properties::{BooleanProperty, DOWN, EAST, NORTH, SOUTH, UP, WATERLOGGED, WEST};
use crisp_data::{Block, BlockState, Fluid, blocks};

use super::BlockBehaviour;
use super::simple_waterlogged_block::schedule_water_tick;
use super::vine_block;
use crate::server::level::ServerLevel;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::{BlockGetter, LevelAccessor, LevelReader};

pub struct MultifaceBlock;

pub fn get_face_property(face_direction: Direction) -> BooleanProperty {
    match face_direction {
        Direction::North => NORTH,
        Direction::East => EAST,
        Direction::South => SOUTH,
        Direction::West => WEST,
        Direction::Up => UP,
        Direction::Down => DOWN,
    }
}

pub fn has_face(state: BlockState, face_direction: Direction) -> bool {
    state.try_get(get_face_property(face_direction)).unwrap_or(false)
}

pub fn can_attach_to(direction_towards_neighbour: Direction, neighbour_state: BlockState) -> bool {
    vine_block::can_attach_to(direction_towards_neighbour, neighbour_state)
}

pub fn can_attach_to_at(level: &mut dyn LevelReader, pos: BlockPos, direction_towards_neighbour: Direction) -> bool {
    let block_state = level.block_state(pos.relative(direction_towards_neighbour));
    can_attach_to(direction_towards_neighbour, block_state)
}

fn remove_face(state: BlockState, property: BooleanProperty) -> BlockState {
    let new_state = state.set(property, false);
    if has_any_face(new_state) { new_state } else { blocks::AIR.default_state() }
}

pub fn has_any_face(state: BlockState) -> bool {
    Direction::VALUES.into_iter().any(|direction| has_face(state, direction))
}

fn has_any_vacant_face(state: BlockState) -> bool {
    Direction::VALUES.into_iter().any(|direction| !has_face(state, direction))
}

pub fn is_valid_state_for_placement(block: Block, level: &mut dyn LevelReader, old_state: BlockState, placement_pos: BlockPos, placement_direction: Direction) -> bool {
    if !old_state.is_block(block) || !has_face(old_state, placement_direction) {
        let neighbour_pos = placement_pos.relative(placement_direction);
        let neighbour_state = level.block_state(neighbour_pos);
        can_attach_to(placement_direction, neighbour_state)
    } else {
        false
    }
}

pub fn get_state_for_placement(block: Block, old_state: BlockState, level: &mut dyn LevelReader, placement_pos: BlockPos, placement_direction: Direction) -> Option<BlockState> {
    if !is_valid_state_for_placement(block, level, old_state, placement_pos, placement_direction) {
        return None;
    }
    let new_state = if old_state.is_block(block) {
        old_state
    } else if old_state.fluid_state().is_source_of_type(Fluid::Water) {
        block.default_state().set(WATERLOGGED, true)
    } else {
        block.default_state()
    };
    Some(new_state.set(get_face_property(placement_direction), true))
}

impl BlockBehaviour for MultifaceBlock {
    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        let place_pos = ctx.clicked_pos();
        let old_state = level.block_state(place_pos);
        ctx.nearest_looking_directions().into_iter().find_map(|direction| get_state_for_placement(block, old_state, level, place_pos, direction))
    }

    fn can_be_replaced(&self, state: BlockState, ctx: &BlockPlaceContext) -> bool {
        ctx.item.is_empty() || ctx.item.item != state.block().item() || has_any_vacant_face(state)
    }

    fn update_shape(&self, state: BlockState, level: &mut dyn LevelAccessor, pos: BlockPos, direction: Direction, _: BlockPos, neighbour_state: BlockState) -> BlockState {
        schedule_water_tick(state, level, pos);
        if !has_any_face(state) {
            blocks::AIR.default_state()
        } else if has_face(state, direction) && !can_attach_to(direction, neighbour_state) {
            remove_face(state, get_face_property(direction))
        } else {
            state
        }
    }

    fn can_survive(&self, state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> bool {
        let mut has_at_least_one_face = false;
        for direction_to_neighbour in Direction::VALUES {
            if has_face(state, direction_to_neighbour) {
                if !can_attach_to_at(level, pos, direction_to_neighbour) {
                    return false;
                }
                has_at_least_one_face = true;
            }
        }
        has_at_least_one_face
    }
}
