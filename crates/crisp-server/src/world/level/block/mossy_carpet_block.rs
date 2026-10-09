use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, Direction};
use crisp_data::properties::{BOTTOM, EAST_WALL, EnumProperty, NORTH_WALL, SOUTH_WALL, WEST_WALL, WallSide};
use crisp_data::{Block, BlockState, blocks};

use super::BlockBehaviour;
use super::vine_block::can_attach_to;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::BlockGetter;
use crate::world::level::update_flags::UPDATE_ALL;

pub struct MossyCarpetBlock;

fn property_for_face(direction: Direction) -> EnumProperty<WallSide> {
    match direction {
        Direction::North => NORTH_WALL,
        Direction::East => EAST_WALL,
        Direction::South => SOUTH_WALL,
        Direction::West => WEST_WALL,
        _ => unreachable!("moss carpets only have horizontal sides"),
    }
}

fn has_faces(state: BlockState) -> bool {
    state.get(BOTTOM) || Direction::PLANE_HORIZONTAL.into_iter().any(|d| state.get(property_for_face(d)) != WallSide::None)
}

fn can_support_at_face(level: &mut ServerLevel, pos: BlockPos, direction: Direction) -> bool {
    if direction == Direction::Up {
        return false;
    }
    let neighbour = level.block_state(pos.relative(direction));
    can_attach_to(direction, neighbour)
}

fn updated_state(mut state: BlockState, level: &mut ServerLevel, pos: BlockPos, mut create_sides: bool) -> BlockState {
    let mut above_state = None;
    let mut below_state = None;
    create_sides |= state.get(BOTTOM);
    for direction in Direction::PLANE_HORIZONTAL {
        let property = property_for_face(direction);
        let mut side = if can_support_at_face(level, pos, direction) { if create_sides { WallSide::Low } else { state.get(property) } } else { WallSide::None };
        if side == WallSide::Low {
            let above = match above_state {
                Some(above) => above,
                None => *above_state.insert(level.block_state(pos.above())),
            };
            if above.is_block(blocks::PALE_MOSS_CARPET) && above.get(property) != WallSide::None && !above.get(BOTTOM) {
                side = WallSide::Tall;
            }
            if !state.get(BOTTOM) {
                let below = match below_state {
                    Some(below) => below,
                    None => *below_state.insert(level.block_state(pos.below())),
                };
                if below.is_block(blocks::PALE_MOSS_CARPET) && below.get(property) == WallSide::None {
                    side = WallSide::None;
                }
            }
        }
        state = state.set(property, side);
    }
    state
}

fn create_topper_with_side_chance(level: &mut ServerLevel, pos: BlockPos, mut side_survival_test: impl FnMut(&mut ServerLevel) -> bool) -> BlockState {
    let above = pos.above();
    let above_previous_state = level.block_state(above);
    let is_mossy_carpet_above = above_previous_state.is_block(blocks::PALE_MOSS_CARPET);
    if (!is_mossy_carpet_above || !above_previous_state.get(BOTTOM)) && (is_mossy_carpet_above || above_previous_state.can_be_replaced()) {
        let no_carpet_base_state = blocks::PALE_MOSS_CARPET.default_state().set(BOTTOM, false);
        let mut above_state = updated_state(no_carpet_base_state, level, above, true);
        for direction in Direction::PLANE_HORIZONTAL {
            let property = property_for_face(direction);
            if above_state.get(property) != WallSide::None && !side_survival_test(level) {
                above_state = above_state.set(property, WallSide::None);
            }
        }
        if has_faces(above_state) && above_state != above_previous_state {
            return above_state;
        }
    }
    blocks::AIR.default_state()
}

impl BlockBehaviour for MossyCarpetBlock {
    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        Some(updated_state(block.default_state(), level, ctx.clicked_pos(), true))
    }

    fn set_placed_by(&self, level: &mut ServerLevel, pos: BlockPos, _state: BlockState, _placer: Option<EntityId>, _item: ItemStack) {
        let topper = create_topper_with_side_chance(level, pos, |level| level.random.next_boolean());
        if !topper.is_air() {
            level.set_block(pos.above(), topper, UPDATE_ALL);
        }
    }

    fn can_survive(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) -> bool {
        let below_state = level.block_state(pos.below());
        if state.get(BOTTOM) { !below_state.is_air() } else { below_state.is_block(state.block()) && below_state.get(BOTTOM) }
    }

    fn update_shape(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, _: Direction, _: BlockPos, _: BlockState) -> BlockState {
        if !self.can_survive(state, level, pos) {
            return blocks::AIR.default_state();
        }
        let updated = updated_state(state, level, pos, false);
        if !has_faces(updated) { blocks::AIR.default_state() } else { updated }
    }

    fn is_valid_bonemeal_target(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState) -> bool {
        state.get(BOTTOM) && !create_topper_with_side_chance(level, pos, |_| true).is_air()
    }

    fn is_bonemeal_success(&self, _level: &mut ServerLevel, _pos: BlockPos, _state: BlockState) -> bool {
        true
    }

    fn perform_bonemeal(&self, level: &mut ServerLevel, pos: BlockPos, _state: BlockState) {
        let topper = create_topper_with_side_chance(level, pos, |_| true);
        if !topper.is_air() {
            level.set_block(pos.above(), topper, UPDATE_ALL);
        }
    }
}
