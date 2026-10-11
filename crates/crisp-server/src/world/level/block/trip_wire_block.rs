use crisp_core::{AABB, BlockPos, Direction};
use crisp_data::properties::{DISARMED, EAST, HORIZONTAL_FACING, NORTH, POWERED, Property, SOUTH, WEST};
use crisp_data::{Block, BlockState, items};

use super::mirror::Mirror;
use super::rotation::Rotation;
use super::{BlockBehaviour, trip_wire_hook_block};
use crate::server::level::ServerLevel;
use crate::server::level::events::PARTICLES_AND_SOUND_DESTROY_BLOCK;
use crate::world::InteractionHand;
use crate::world::entity::EntityId;
use crate::world::entity::inside_block::InsideBlockEffects;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::{BlockGetter, LevelAccessor};
use crate::world::level::update_flags::UPDATE_NONE;

const RECHECK_PERIOD: i32 = 10;

pub struct TripWireBlock {
    pub block: Block,
    pub hook: Block,
}

fn property(direction: Direction) -> Property<bool> {
    match direction {
        Direction::North => NORTH,
        Direction::East => EAST,
        Direction::South => SOUTH,
        Direction::West => WEST,
        _ => panic!("no connection property for {direction:?}"),
    }
}

pub fn shape_bounds(state: BlockState) -> AABB {
    let shape = state.shape();
    shape[1..].iter().fold(shape[0], |bounds, b| bounds.minmax(b))
}

impl TripWireBlock {
    pub fn new(block: Block) -> TripWireBlock {
        let fields: serde_json::Value = serde_json::from_str(block.info().fields).unwrap_or_default();
        let hook = fields["hook"].as_str().and_then(Block::by_name).unwrap_or(crisp_data::blocks::TRIPWIRE_HOOK);
        TripWireBlock { block, hook }
    }

    pub fn should_connect_to(&self, block_state: BlockState, direction: Direction) -> bool {
        if block_state.is_block(self.hook) { block_state.get(HORIZONTAL_FACING) == direction.opposite() } else { block_state.is_block(self.block) }
    }

    fn update_source(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState) {
        for direction in [Direction::South, Direction::West] {
            for i in 1..42 {
                let test_pos = pos.relative_n(direction, i);
                let block = level.block_state(test_pos);
                if block.is_block(self.hook) {
                    if block.get(HORIZONTAL_FACING) == direction.opposite() {
                        trip_wire_hook_block::calculate_state(level, test_pos, block, false, true, i, Some(state));
                    }
                    break;
                }
                if !block.is_block(self.block) {
                    break;
                }
            }
        }
    }

    fn check_pressed(&self, level: &mut ServerLevel, pos: BlockPos) {
        let state = level.block_state(pos);
        let area = shape_bounds(state).move_to_block(pos);
        let entities = level.non_spectators_touching(&area, false);
        self.check_pressed_by(level, pos, !entities.is_empty());
    }

    fn check_pressed_by(&self, level: &mut ServerLevel, pos: BlockPos, any_entity: bool) {
        let mut state = level.block_state(pos);
        let was_pressed = state.get(POWERED);
        let should_be_pressed = any_entity;
        if should_be_pressed != was_pressed {
            state = state.set(POWERED, should_be_pressed);
            level.set_block_simple(pos, state);
            self.update_source(level, pos, state);
        }
        if should_be_pressed {
            level.schedule_tick(pos, self.block, RECHECK_PERIOD);
        } else if was_pressed {
            level.schedule_tick(pos, self.block, 1);
        }
    }
}

impl BlockBehaviour for TripWireBlock {
    fn rotate(&self, state: BlockState, rotation: Rotation) -> BlockState {
        match rotation {
            Rotation::Clockwise180 => state.set(NORTH, state.get(SOUTH)).set(EAST, state.get(WEST)).set(SOUTH, state.get(NORTH)).set(WEST, state.get(EAST)),
            Rotation::Counterclockwise90 => state.set(NORTH, state.get(EAST)).set(EAST, state.get(SOUTH)).set(SOUTH, state.get(WEST)).set(WEST, state.get(NORTH)),
            Rotation::Clockwise90 => state.set(NORTH, state.get(WEST)).set(EAST, state.get(NORTH)).set(SOUTH, state.get(EAST)).set(WEST, state.get(SOUTH)),
            Rotation::None => state,
        }
    }

    fn mirror(&self, state: BlockState, mirror: Mirror) -> BlockState {
        match mirror {
            Mirror::LeftRight => state.set(NORTH, state.get(SOUTH)).set(SOUTH, state.get(NORTH)),
            Mirror::FrontBack => state.set(EAST, state.get(WEST)).set(WEST, state.get(EAST)),
            Mirror::None => state,
        }
    }

    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        let pos = ctx.clicked_pos();
        let north = self.should_connect_to(level.block_state(pos.north()), Direction::North);
        let east = self.should_connect_to(level.block_state(pos.east()), Direction::East);
        let south = self.should_connect_to(level.block_state(pos.south()), Direction::South);
        let west = self.should_connect_to(level.block_state(pos.west()), Direction::West);
        Some(block.default_state().set(NORTH, north).set(EAST, east).set(SOUTH, south).set(WEST, west))
    }

    fn update_shape(
        &self,
        state: BlockState,
        _level: &mut dyn LevelAccessor,
        _pos: BlockPos,
        direction: Direction,
        _neighbour_pos: BlockPos,
        neighbour_state: BlockState,
    ) -> BlockState {
        if direction.axis().is_horizontal() { state.set(property(direction), self.should_connect_to(neighbour_state, direction)) } else { state }
    }

    fn on_place(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, old_state: BlockState, _moved_by_piston: bool) {
        if !old_state.is_block(state.block()) {
            self.update_source(level, pos, state);
        }
    }

    fn affect_neighbors_after_removal(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, moved_by_piston: bool) {
        if !moved_by_piston {
            self.update_source(level, pos, state.set(POWERED, true));
        }
    }

    fn player_will_destroy(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState, player: EntityId) -> BlockState {
        let main_hand = level.player(player).item_in_hand(InteractionHand::MainHand);
        if main_hand.is_some_and(|stack| !stack.is_empty() && stack.item == items::SHEARS) {
            level.set_block(pos, state.set(DISARMED, true), UPDATE_NONE);
        }
        level.level_event(Some(player), PARTICLES_AND_SOUND_DESTROY_BLOCK, pos, state.id() as i32);
        state
    }

    fn entity_inside(
        &self,
        state: BlockState,
        level: &mut ServerLevel,
        pos: BlockPos,
        _entity: EntityId,
        _effects: &mut InsideBlockEffects,
        _is_precise: bool,
    ) {
        if !state.get(POWERED) && !level.has_scheduled_tick(pos, self.block) {
            self.check_pressed_by(level, pos, true);
        }
    }

    fn tick(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        if level.block_state(pos).get(POWERED) {
            self.check_pressed(level, pos);
        }
    }
}
