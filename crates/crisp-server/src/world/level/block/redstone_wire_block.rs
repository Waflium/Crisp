use std::cell::Cell;

use crisp_core::{BlockPos, Direction};
use crisp_data::properties::{EAST_REDSTONE, NORTH_REDSTONE, POWER, Property, RedstoneSide, SOUTH_REDSTONE, WEST_REDSTONE};
use crisp_data::{Block, BlockState, blocks};

use super::{BlockBehaviour, BlockHitResult};
use crate::server::level::ServerLevel;
use crate::world::InteractionResult;
use crate::world::entity::EntityId;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::{BlockGetter, LevelReader};
use crate::world::level::redstone::default_redstone_wire_evaluator::DefaultRedstoneWireEvaluator;

const PLANE_VERTICAL: [Direction; 2] = [Direction::Up, Direction::Down];

thread_local! {
    static SHOULD_SIGNAL: Cell<bool> = const { Cell::new(true) };
}

fn should_signal() -> bool {
    SHOULD_SIGNAL.with(Cell::get)
}

pub fn property(direction: Direction) -> Property<RedstoneSide> {
    match direction {
        Direction::North => NORTH_REDSTONE,
        Direction::East => EAST_REDSTONE,
        Direction::South => SOUTH_REDSTONE,
        Direction::West => WEST_REDSTONE,
        _ => panic!("no redstone side for {direction:?}"),
    }
}

fn is_connected(side: RedstoneSide) -> bool {
    side != RedstoneSide::None
}

fn connected(state: BlockState, direction: Direction) -> bool {
    is_connected(state.get(property(direction)))
}

fn is_cross(state: BlockState) -> bool {
    connected(state, Direction::North) && connected(state, Direction::South) && connected(state, Direction::East) && connected(state, Direction::West)
}

fn is_dot(state: BlockState) -> bool {
    !connected(state, Direction::North) && !connected(state, Direction::South) && !connected(state, Direction::East) && !connected(state, Direction::West)
}

fn should_connect_to_pos(level: &mut ServerLevel, pos: BlockPos) -> bool {
    let state = level.block_state(pos);
    should_connect_to(state, level, pos, None)
}

fn should_connect_to(state: BlockState, level: &mut ServerLevel, pos: BlockPos, direction: Option<Direction>) -> bool {
    super::of(state).should_redstone_wire_connect_to(state, level, pos, direction)
}

fn can_survive_on(state: BlockState) -> bool {
    state.is_face_sturdy_full(Direction::Up) || state.is_block(blocks::HOPPER)
}

pub fn get_block_signal(level: &mut ServerLevel, pos: BlockPos) -> i32 {
    SHOULD_SIGNAL.with(|s| s.set(false));
    let block_signal = level.get_best_neighbor_signal(pos);
    SHOULD_SIGNAL.with(|s| s.set(true));
    block_signal
}

pub struct RedstoneWireBlock {
    pub block: Block,
    cross_state: BlockState,
    evaluator: DefaultRedstoneWireEvaluator,
}

impl RedstoneWireBlock {
    pub fn new(block: Block) -> RedstoneWireBlock {
        let cross_state = block
            .default_state()
            .set(NORTH_REDSTONE, RedstoneSide::Side)
            .set(EAST_REDSTONE, RedstoneSide::Side)
            .set(SOUTH_REDSTONE, RedstoneSide::Side)
            .set(WEST_REDSTONE, RedstoneSide::Side);
        RedstoneWireBlock { block, cross_state, evaluator: DefaultRedstoneWireEvaluator { wire_block: block } }
    }

    fn get_connection_state(&self, level: &mut ServerLevel, state: BlockState, pos: BlockPos) -> BlockState {
        let was_dot = is_dot(state);
        let mut state = self.get_missing_connections(level, self.block.default_state().set(POWER, state.get(POWER)), pos);
        if was_dot && is_dot(state) {
            return state;
        }
        let north = connected(state, Direction::North);
        let south = connected(state, Direction::South);
        let east = connected(state, Direction::East);
        let west = connected(state, Direction::West);
        let north_south_empty = !north && !south;
        let east_west_empty = !east && !west;
        if !west && north_south_empty {
            state = state.set(WEST_REDSTONE, RedstoneSide::Side);
        }
        if !east && north_south_empty {
            state = state.set(EAST_REDSTONE, RedstoneSide::Side);
        }
        if !north && east_west_empty {
            state = state.set(NORTH_REDSTONE, RedstoneSide::Side);
        }
        if !south && east_west_empty {
            state = state.set(SOUTH_REDSTONE, RedstoneSide::Side);
        }
        state
    }

    fn get_missing_connections(&self, level: &mut ServerLevel, mut state: BlockState, pos: BlockPos) -> BlockState {
        let can_connect_up = !level.block_state(pos.above()).is_redstone_conductor();
        for direction in Direction::PLANE_HORIZONTAL {
            if !connected(state, direction) {
                let side = self.get_connecting_side_up(level, pos, direction, can_connect_up);
                state = state.set(property(direction), side);
            }
        }
        state
    }

    fn get_connecting_side(&self, level: &mut ServerLevel, pos: BlockPos, direction: Direction) -> RedstoneSide {
        let can_connect_up = !level.block_state(pos.above()).is_redstone_conductor();
        self.get_connecting_side_up(level, pos, direction, can_connect_up)
    }

    fn get_connecting_side_up(&self, level: &mut ServerLevel, pos: BlockPos, direction: Direction, can_connect_up: bool) -> RedstoneSide {
        let relative_pos = pos.relative(direction);
        let relative_state = level.block_state(relative_pos);
        if can_connect_up {
            let is_placeable_above = relative_state.block().is_instance_of("TrapDoorBlock") || can_survive_on(relative_state);
            if is_placeable_above && should_connect_to_pos(level, relative_pos.above()) {
                if relative_state.is_face_sturdy_full(direction.opposite()) {
                    return RedstoneSide::Up;
                }
                return RedstoneSide::Side;
            }
        }
        if !should_connect_to(relative_state, level, relative_pos, Some(direction))
            && (relative_state.is_redstone_conductor() || !should_connect_to_pos(level, relative_pos.below()))
        {
            RedstoneSide::None
        } else {
            RedstoneSide::Side
        }
    }

    fn check_corner_change_at(&self, level: &mut ServerLevel, pos: BlockPos) {
        if level.block_state(pos).is_block(self.block) {
            level.update_neighbors_at(pos, self.block);
            for direction in Direction::VALUES {
                level.update_neighbors_at(pos.relative(direction), self.block);
            }
        }
    }

    fn update_neighbors_of_neighboring_wires(&self, level: &mut ServerLevel, pos: BlockPos) {
        for direction in Direction::PLANE_HORIZONTAL {
            self.check_corner_change_at(level, pos.relative(direction));
        }
        for direction in Direction::PLANE_HORIZONTAL {
            let target = pos.relative(direction);
            if level.block_state(target).is_redstone_conductor() {
                self.check_corner_change_at(level, target.above());
            } else {
                self.check_corner_change_at(level, target.below());
            }
        }
    }

    fn updates_on_shape_change(&self, level: &mut ServerLevel, pos: BlockPos, old_state: BlockState, new_state: BlockState) {
        for direction in Direction::PLANE_HORIZONTAL {
            let relative_pos = pos.relative(direction);
            if connected(old_state, direction) != connected(new_state, direction) && level.block_state(relative_pos).is_redstone_conductor() {
                level.update_neighbors_at_except_from_facing(relative_pos, new_state.block(), Some(direction.opposite()));
            }
        }
    }
}

impl BlockBehaviour for RedstoneWireBlock {
    fn get_state_for_placement(&self, _block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        Some(self.get_connection_state(level, self.cross_state, ctx.clicked_pos()))
    }

    fn update_shape(
        &self,
        state: BlockState,
        level: &mut ServerLevel,
        pos: BlockPos,
        direction: Direction,
        _neighbour_pos: BlockPos,
        neighbour_state: BlockState,
    ) -> BlockState {
        if direction == Direction::Down {
            return if !can_survive_on(neighbour_state) { blocks::AIR.default_state() } else { state };
        }
        if direction == Direction::Up {
            return self.get_connection_state(level, state, pos);
        }
        let side = self.get_connecting_side(level, pos, direction);
        if is_connected(side) == connected(state, direction) && !is_cross(state) {
            state.set(property(direction), side)
        } else {
            let cross = self.cross_state.set(POWER, state.get(POWER)).set(property(direction), side);
            self.get_connection_state(level, cross, pos)
        }
    }

    fn update_indirect_neighbour_shapes(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, flags: i32, update_limit: i32) {
        for direction in Direction::PLANE_HORIZONTAL {
            let value = state.get(property(direction));
            if value != RedstoneSide::None && !level.block_state(pos.relative(direction)).is_block(self.block) {
                let below = pos.relative(direction).below();
                if level.block_state(below).is_block(self.block) {
                    let neighbor_pos = below.relative(direction.opposite());
                    let neighbor_state = level.block_state(neighbor_pos);
                    level.neighbor_shape_changed(direction.opposite(), below, neighbor_pos, neighbor_state, flags, update_limit);
                }
                let above = pos.relative(direction).above();
                if level.block_state(above).is_block(self.block) {
                    let neighbor_pos = above.relative(direction.opposite());
                    let neighbor_state = level.block_state(neighbor_pos);
                    level.neighbor_shape_changed(direction.opposite(), above, neighbor_pos, neighbor_state, flags, update_limit);
                }
            }
        }
    }

    fn can_survive(&self, _state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> bool {
        can_survive_on(level.block_state(pos.below()))
    }

    fn on_place(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, old_state: BlockState, _moved_by_piston: bool) {
        if !old_state.is_block(state.block()) {
            self.evaluator.update_power_strength(level, pos, state);
            for direction in PLANE_VERTICAL {
                level.update_neighbors_at(pos.relative(direction), self.block);
            }
            self.update_neighbors_of_neighboring_wires(level, pos);
        }
    }

    fn affect_neighbors_after_removal(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, moved_by_piston: bool) {
        if !moved_by_piston {
            for direction in Direction::VALUES {
                level.update_neighbors_at(pos.relative(direction), self.block);
            }
            self.evaluator.update_power_strength(level, pos, state);
            self.update_neighbors_of_neighboring_wires(level, pos);
        }
    }

    fn neighbor_changed(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, _block: Block, _moved_by_piston: bool) {
        if self.can_survive(state, level, pos) {
            self.evaluator.update_power_strength(level, pos, state);
        } else {
            level.drop_resources(state, pos, None, None);
            level.remove_block(pos, false);
        }
    }

    fn get_direct_signal(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, direction: Direction) -> i32 {
        if !should_signal() { 0 } else { self.get_signal(state, level, pos, direction) }
    }

    fn get_signal(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, direction: Direction) -> i32 {
        if should_signal() && direction != Direction::Down {
            let power = state.get(POWER);
            if power == 0 {
                0
            } else if direction != Direction::Up && !connected(self.get_connection_state(level, state, pos), direction.opposite()) {
                0
            } else {
                power
            }
        } else {
            0
        }
    }

    fn own_signal(&self, state: BlockState, _level: &mut ServerLevel, _pos: BlockPos) -> i32 {
        state.get(POWER)
    }

    fn should_redstone_wire_connect_to(&self, _state: BlockState, _level: &mut ServerLevel, _pos: BlockPos, _direction: Option<Direction>) -> bool {
        true
    }

    fn use_without_item(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, player: EntityId, _hit: &BlockHitResult) -> InteractionResult {
        if !level.may_build(player) {
            return InteractionResult::Pass;
        }
        if is_cross(state) || is_dot(state) {
            let new_state = if is_cross(state) { self.block.default_state() } else { self.cross_state };
            let new_state = new_state.set(POWER, state.get(POWER));
            let new_state = self.get_connection_state(level, new_state, pos);
            if new_state != state {
                level.set_block_simple(pos, new_state);
                self.updates_on_shape_change(level, pos, state, new_state);
                return InteractionResult::SUCCESS;
            }
        }
        InteractionResult::Pass
    }
}

impl ServerLevel {
    pub fn may_build(&self, player: EntityId) -> bool {
        use crate::server::level::game_mode::GameType;
        !matches!(self.player(player).game_mode.game_type, GameType::Adventure | GameType::Spectator)
    }
}
