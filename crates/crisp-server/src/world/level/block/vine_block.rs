use crisp_core::random::RandomSource;
use crisp_core::shapes::{self, VoxelShape};
use crisp_core::{BlockPos, Direction};
use crisp_data::properties::{BooleanProperty, EAST, NORTH, SOUTH, UP, WEST};
use crisp_data::{Block, BlockState, SupportType, blocks};

use super::BlockBehaviour;
use super::vegetation_block::at_most_matched;
use crate::server::level::ServerLevel;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::{BlockGetter, LevelAccessor, LevelReader};
use crate::world::level::update_flags::UPDATE_CLIENTS;

pub struct VineBlock;

pub fn property_for_face(direction: Direction) -> BooleanProperty {
    match direction {
        Direction::Up => UP,
        Direction::North => NORTH,
        Direction::East => EAST,
        Direction::South => SOUTH,
        Direction::West => WEST,
        Direction::Down => unreachable!("vines have no bottom face"),
    }
}

pub fn can_attach_to(direction_towards_neighbour: Direction, neighbour_state: BlockState) -> bool {
    let face = direction_towards_neighbour.opposite();
    neighbour_state.is_face_sturdy(face, SupportType::Full) || shapes::is_face_full(&VoxelShape::from_boxes(neighbour_state.collision_shape()), face)
}

pub fn is_acceptable_neighbour(level: &mut dyn LevelReader, neighbour_pos: BlockPos, direction_to_neighbour: Direction) -> bool {
    let state = level.block_state(neighbour_pos);
    can_attach_to(direction_to_neighbour, state)
}

fn count_faces(state: BlockState) -> usize {
    [UP, NORTH, EAST, SOUTH, WEST].into_iter().filter(|&p| state.get(p)).count()
}

fn has_horizontal_connection(state: BlockState) -> bool {
    state.get(NORTH) || state.get(EAST) || state.get(SOUTH) || state.get(WEST)
}

fn can_support_at_face(level: &mut dyn LevelReader, pos: BlockPos, direction: Direction) -> bool {
    if direction == Direction::Down {
        return false;
    }
    if is_acceptable_neighbour(level, pos.relative(direction), direction) {
        return true;
    }
    if direction == Direction::Up {
        return false;
    }
    let above_state = level.block_state(pos.above());
    above_state.is_block(blocks::VINE) && above_state.get(property_for_face(direction))
}

fn updated_state(mut state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> BlockState {
    let above_pos = pos.above();
    if state.get(UP) {
        state = state.set(UP, is_acceptable_neighbour(level, above_pos, Direction::Down));
    }
    let mut above_state = None;
    for direction in Direction::PLANE_HORIZONTAL {
        let property = property_for_face(direction);
        if state.get(property) {
            let mut can_support = can_support_at_face(level, pos, direction);
            if !can_support {
                let above = match above_state {
                    Some(above) => above,
                    None => *above_state.insert(level.block_state(above_pos)),
                };
                can_support = above.is_block(state.block()) && above.get(property);
            }
            state = state.set(property, can_support);
        }
    }
    state
}

fn can_spread(level: &mut ServerLevel, pos: BlockPos) -> bool {
    at_most_matched(level, pos.offset(-4, -1, -4), pos.offset(4, 1, 4), blocks::VINE, 4)
}

fn copy_random_faces(from: BlockState, mut to: BlockState, level: &mut ServerLevel) -> BlockState {
    for direction in Direction::PLANE_HORIZONTAL {
        if level.random.next_boolean() {
            let property = property_for_face(direction);
            if from.get(property) {
                to = to.set(property, true);
            }
        }
    }
    to
}

impl BlockBehaviour for VineBlock {
    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        let clicked_state = level.block_state(ctx.clicked_pos());
        let clicked_vine = clicked_state.is_block(block);
        let result = if clicked_vine { clicked_state } else { block.default_state() };
        for direction in ctx.nearest_looking_directions() {
            if direction != Direction::Down {
                let face = property_for_face(direction);
                let face_occupied = clicked_vine && clicked_state.get(face);
                if !face_occupied && can_support_at_face(level, ctx.clicked_pos(), direction) {
                    return Some(result.set(face, true));
                }
            }
        }
        clicked_vine.then_some(result)
    }

    fn can_be_replaced(&self, state: BlockState, ctx: &BlockPlaceContext) -> bool {
        if state.is_block(blocks::VINE) { count_faces(state) < 5 } else { super::vegetation_block::block_can_be_replaced(state, ctx) }
    }

    fn update_shape(&self, state: BlockState, level: &mut dyn LevelAccessor, pos: BlockPos, direction: Direction, _: BlockPos, _: BlockState) -> BlockState {
        if direction == Direction::Down {
            return state;
        }
        let updated = updated_state(state, level, pos);
        if count_faces(updated) == 0 { blocks::AIR.default_state() } else { updated }
    }

    fn can_survive(&self, state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> bool {
        count_faces(updated_state(state, level, pos)) > 0
    }

    fn random_tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        if level.random.next_int_bounded(4) != 0 {
            return;
        }
        let block = state.block();
        let test_direction = Direction::VALUES[level.random.next_int_bounded(6) as usize];
        let above_pos = pos.above();
        if test_direction.axis().is_horizontal() && !state.get(property_for_face(test_direction)) {
            if !can_spread(level, pos) {
                return;
            }
            let test_pos = pos.relative(test_direction);
            let edge_state = level.block_state(test_pos);
            if edge_state.is_air() {
                let cw_direction = test_direction.clockwise();
                let ccw_direction = test_direction.counter_clockwise();
                let cw_has_connecting_face = state.get(property_for_face(cw_direction));
                let ccw_has_connecting_face = state.get(property_for_face(ccw_direction));
                let cw_test_pos = test_pos.relative(cw_direction);
                let ccw_test_pos = test_pos.relative(ccw_direction);
                if cw_has_connecting_face && is_acceptable_neighbour(level, cw_test_pos, cw_direction) {
                    level.set_block(test_pos, block.default_state().set(property_for_face(cw_direction), true), UPDATE_CLIENTS);
                } else if ccw_has_connecting_face && is_acceptable_neighbour(level, ccw_test_pos, ccw_direction) {
                    level.set_block(test_pos, block.default_state().set(property_for_face(ccw_direction), true), UPDATE_CLIENTS);
                } else {
                    let opposite = test_direction.opposite();
                    if cw_has_connecting_face && level.block_state(cw_test_pos).is_air() && is_acceptable_neighbour(level, pos.relative(cw_direction), opposite) {
                        level.set_block(cw_test_pos, block.default_state().set(property_for_face(opposite), true), UPDATE_CLIENTS);
                    } else if ccw_has_connecting_face
                        && level.block_state(ccw_test_pos).is_air()
                        && is_acceptable_neighbour(level, pos.relative(ccw_direction), opposite)
                    {
                        level.set_block(ccw_test_pos, block.default_state().set(property_for_face(opposite), true), UPDATE_CLIENTS);
                    } else if (level.random.next_float() as f64) < 0.05 && is_acceptable_neighbour(level, test_pos.above(), Direction::Up) {
                        level.set_block(test_pos, block.default_state().set(UP, true), UPDATE_CLIENTS);
                    }
                }
            } else if is_acceptable_neighbour(level, test_pos, test_direction) {
                level.set_block(pos, state.set(property_for_face(test_direction), true), UPDATE_CLIENTS);
            }
            return;
        }
        if test_direction == Direction::Up && pos.y < level.chunks.max_y() {
            if can_support_at_face(level, pos, test_direction) {
                level.set_block(pos, state.set(UP, true), UPDATE_CLIENTS);
                return;
            }
            if level.block_state(above_pos).is_air() {
                if !can_spread(level, pos) {
                    return;
                }
                let mut above_state = state;
                for direction in Direction::PLANE_HORIZONTAL {
                    if level.random.next_boolean() || !is_acceptable_neighbour(level, above_pos.relative(direction), direction) {
                        above_state = above_state.set(property_for_face(direction), false);
                    }
                }
                if has_horizontal_connection(above_state) {
                    level.set_block(above_pos, above_state, UPDATE_CLIENTS);
                }
                return;
            }
        }
        if pos.y > level.chunks.min_y() {
            let below_pos = pos.below();
            let below_state = level.block_state(below_pos);
            if below_state.is_air() || below_state.is_block(block) {
                let before = if below_state.is_air() { block.default_state() } else { below_state };
                let after = copy_random_faces(state, before, level);
                if before != after && has_horizontal_connection(after) {
                    level.set_block(below_pos, after, UPDATE_CLIENTS);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crisp_core::{BlockPos, Direction, Vec3};
    use crisp_data::properties::{NORTH, UP, WEST};
    use crisp_data::{blocks, items};

    use crate::server::level::testing;
    use crate::world::InteractionHand;
    use crate::world::item::ItemStack;
    use crate::world::level::BlockGetter;
    use crate::world::level::block::{BlockBehaviour, BlockHitResult};
    use crate::world::level::update_flags::UPDATE_ALL;

    const STONE: BlockPos = BlockPos::new(0, -58, 0);

    #[test]
    fn vines_attach_to_the_clicked_face() {
        let mut level = testing::level();
        level.set_block(STONE, blocks::STONE.default_state(), UPDATE_ALL);
        let player = testing::player(&mut level, Vec3::new(2.5, -60.0, 0.5), 90.0, 0.0);
        level.player_mut(player).inventory.slots[36] = Some(ItemStack::new(items::VINE, 1));
        let hit = BlockHitResult { location: Vec3::new(1.0, -57.5, 0.5), direction: Direction::East, block_pos: STONE, inside: false, world_border_hit: false };
        level.use_item_on(player, InteractionHand::MainHand, hit);
        assert_eq!(level.block_state(STONE.east()), blocks::VINE.default_state().set(WEST, true));
    }

    #[test]
    fn vines_lose_faces_without_support_and_hang_from_vines_above() {
        let mut level = testing::level();
        level.set_block(STONE, blocks::STONE.default_state(), UPDATE_ALL);
        let vine = blocks::VINE.default_state().set(NORTH, true);
        level.set_block(STONE.south(), vine, UPDATE_ALL);
        let below = STONE.south().below();
        assert!(super::VineBlock.can_survive(vine, &mut level, below));
        let up_only = blocks::VINE.default_state().set(UP, true);
        assert!(!super::VineBlock.can_survive(up_only, &mut level, below));
        level.set_block(below, vine, UPDATE_ALL);

        level.set_block(STONE, blocks::AIR.default_state(), UPDATE_ALL);
        assert!(level.block_state(STONE.south()).is_air());
        assert!(level.block_state(below).is_air());
    }
}
