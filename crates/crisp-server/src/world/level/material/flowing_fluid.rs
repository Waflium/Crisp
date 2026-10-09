use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};

use crisp_core::shapes::{BooleanOp, VoxelShape, join_is_not_empty};
use crisp_core::{AABB, AxisDirection, BlockPos, Direction};
use crisp_data::{BlockState, Fluid, FluidState, block_tags, blocks};

use super::{can_be_replaced_with, create_legacy_block};
use crate::server::level::ServerLevel;
use crate::world::level::BlockGetter;
use crate::world::level::block;
use crate::world::level::update_flags::UPDATE_ALL;

const CACHE_SIZE: usize = 200;

thread_local! {
    static OCCLUSION_CACHE: RefCell<VecDeque<(BlockState, BlockState, Direction, bool)>> = RefCell::new(VecDeque::with_capacity(CACHE_SIZE));
}

pub trait FlowingFluid: Sync {
    fn flowing(&self) -> Fluid;

    fn source(&self) -> Fluid;

    fn can_convert_to_source(&self, level: &ServerLevel) -> bool;

    fn before_destroying_block(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState);

    fn slope_find_distance(&self, level: &ServerLevel) -> i32;

    fn drop_off(&self, level: &ServerLevel) -> i32;

    fn tick_delay(&self, level: &ServerLevel) -> i32;

    fn random_tick(&self, _level: &mut ServerLevel, _pos: BlockPos, _fluid: FluidState) {}

    fn spread_delay(&self, level: &mut ServerLevel, _pos: BlockPos, _old: FluidState, _new: FluidState) -> i32 {
        self.tick_delay(level)
    }

    fn spread_to(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState, direction: Direction, target: FluidState) {
        self.flowing_spread_to(level, pos, state, direction, target);
    }

    fn flowing_spread_to(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState, _direction: Direction, target: FluidState) {
        let behaviour = block::of(state);
        if behaviour.is_liquid_block_container(state) {
            behaviour.place_liquid(level, pos, state, target);
        } else {
            if !state.is_air() {
                self.before_destroying_block(level, pos, state);
            }
            level.set_block(pos, create_legacy_block(target), UPDATE_ALL);
        }
    }

    fn is_same(&self, other: Fluid) -> bool {
        other.source() == self.source()
    }

    fn flowing_state(&self, amount: i32, falling: bool) -> FluidState {
        FluidState { fluid: self.flowing(), amount: amount as u8, falling }
    }

    fn source_state(&self, falling: bool) -> FluidState {
        FluidState { fluid: self.source(), amount: 8, falling }
    }

    fn tick(&self, level: &mut ServerLevel, pos: BlockPos, mut block_state: BlockState, mut fluid: FluidState) {
        if !fluid.is_source() {
            let current = level.block_state(pos);
            let new_fluid = self.new_liquid(level, pos, current);
            let tick_delay = self.spread_delay(level, pos, fluid, new_fluid);
            if new_fluid.is_empty() {
                fluid = new_fluid;
                block_state = blocks::AIR.default_state();
                level.set_block(pos, block_state, UPDATE_ALL);
            } else if new_fluid != fluid {
                fluid = new_fluid;
                block_state = create_legacy_block(fluid);
                level.set_block(pos, block_state, UPDATE_ALL);
                level.schedule_fluid_tick(pos, fluid.fluid, tick_delay);
            }
        }
        self.spread(level, pos, block_state, fluid);
    }

    fn spread(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState, fluid: FluidState) {
        if fluid.is_empty() {
            return;
        }
        let below_pos = pos.below();
        let below_state = level.block_state(below_pos);
        let below_fluid = below_state.fluid_state();
        if self.can_maybe_pass_through(level, pos, state, Direction::Down, below_pos, below_state, below_fluid) {
            let new_below = self.new_liquid(level, below_pos, below_state);
            if can_be_replaced_with(level, below_fluid, below_pos, new_below.fluid, Direction::Down)
                && can_hold_specific_fluid(level, below_pos, below_state, new_below.fluid)
            {
                self.spread_to(level, below_pos, below_state, Direction::Down, new_below);
                if self.source_neighbor_count(level, pos) >= 3 {
                    self.spread_to_sides(level, pos, fluid, state);
                }
                return;
            }
        }
        if fluid.is_source() || !self.is_water_hole(level, pos, state, below_pos, below_state) {
            self.spread_to_sides(level, pos, fluid, state);
        }
    }

    fn spread_to_sides(&self, level: &mut ServerLevel, pos: BlockPos, fluid: FluidState, state: BlockState) {
        let mut neighbor = fluid.amount as i32 - self.drop_off(level);
        if fluid.falling {
            neighbor = 7;
        }
        if neighbor > 0 {
            for (direction, new_fluid) in self.get_spread(level, pos, state) {
                let neighbor_pos = pos.relative(direction);
                let neighbor_state = level.block_state(neighbor_pos);
                self.spread_to(level, neighbor_pos, neighbor_state, direction, new_fluid);
            }
        }
    }

    fn new_liquid(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState) -> FluidState {
        let mut highest_neighbor = 0;
        let mut neighbour_sources = 0;
        for direction in Direction::PLANE_HORIZONTAL {
            let relative_pos = pos.relative(direction);
            let block_state = level.block_state(relative_pos);
            let fluid = block_state.fluid_state();
            if self.is_same(fluid.fluid) && can_pass_through_wall(direction, pos, state, relative_pos, block_state) {
                if fluid.is_source() {
                    neighbour_sources += 1;
                }
                highest_neighbor = highest_neighbor.max(fluid.amount as i32);
            }
        }
        if neighbour_sources >= 2 && self.can_convert_to_source(level) {
            let below_state = level.block_state(pos.below());
            let below_fluid = below_state.fluid_state();
            if below_state.is_solid() || self.is_source_block_of_this_type(below_fluid) {
                return self.source_state(false);
            }
        }
        let above_pos = pos.above();
        let above_state = level.block_state(above_pos);
        let above_fluid = above_state.fluid_state();
        if !above_fluid.is_empty() && self.is_same(above_fluid.fluid) && can_pass_through_wall(Direction::Up, pos, state, above_pos, above_state) {
            return self.flowing_state(8, true);
        }
        let amount = highest_neighbor - self.drop_off(level);
        if amount <= 0 { FluidState::EMPTY } else { self.flowing_state(amount, false) }
    }

    #[allow(clippy::too_many_arguments)]
    fn slope_distance(&self, level: &mut ServerLevel, pos: BlockPos, pass: i32, from: Direction, state: BlockState, context: &mut SpreadContext) -> i32 {
        let mut lowest = 1000;
        for direction in Direction::PLANE_HORIZONTAL {
            if direction == from {
                continue;
            }
            let test_pos = pos.relative(direction);
            let test_state = context.block_state(level, test_pos);
            let test_fluid = test_state.fluid_state();
            if self.can_pass_through(level, self.flowing(), pos, state, direction, test_pos, test_state, test_fluid) {
                if context.is_hole(self, level, test_pos) {
                    return pass;
                }
                if pass < self.slope_find_distance(level) {
                    let v = self.slope_distance(level, test_pos, pass + 1, direction.opposite(), test_state, context);
                    if v < lowest {
                        lowest = v;
                    }
                }
            }
        }
        lowest
    }

    fn is_water_hole(&self, level: &mut ServerLevel, top_pos: BlockPos, top_state: BlockState, bottom_pos: BlockPos, bottom_state: BlockState) -> bool {
        if !can_pass_through_wall(Direction::Down, top_pos, top_state, bottom_pos, bottom_state) {
            return false;
        }
        if self.is_same(bottom_state.fluid_state().fluid) {
            return true;
        }
        can_hold_fluid(level, bottom_pos, bottom_state, self.flowing())
    }

    #[allow(clippy::too_many_arguments)]
    fn can_pass_through(
        &self,
        level: &mut ServerLevel,
        fluid: Fluid,
        source_pos: BlockPos,
        source_state: BlockState,
        direction: Direction,
        test_pos: BlockPos,
        test_state: BlockState,
        test_fluid: FluidState,
    ) -> bool {
        self.can_maybe_pass_through(level, source_pos, source_state, direction, test_pos, test_state, test_fluid)
            && can_hold_specific_fluid(level, test_pos, test_state, fluid)
    }

    #[allow(clippy::too_many_arguments)]
    fn can_maybe_pass_through(
        &self,
        _level: &mut ServerLevel,
        source_pos: BlockPos,
        source_state: BlockState,
        direction: Direction,
        test_pos: BlockPos,
        test_state: BlockState,
        test_fluid: FluidState,
    ) -> bool {
        !self.is_source_block_of_this_type(test_fluid)
            && can_hold_any_fluid(test_state)
            && can_pass_through_wall(direction, source_pos, source_state, test_pos, test_state)
    }

    fn is_source_block_of_this_type(&self, fluid: FluidState) -> bool {
        self.is_same(fluid.fluid) && fluid.is_source()
    }

    fn source_neighbor_count(&self, level: &mut ServerLevel, pos: BlockPos) -> i32 {
        let mut count = 0;
        for direction in Direction::PLANE_HORIZONTAL {
            let test_fluid = level.fluid_state(pos.relative(direction));
            if self.is_source_block_of_this_type(test_fluid) {
                count += 1;
            }
        }
        count
    }

    fn get_spread(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState) -> Vec<(Direction, FluidState)> {
        let mut lowest = 1000;
        let mut result: Vec<(Direction, FluidState)> = Vec::new();
        let mut context: Option<SpreadContext> = None;
        for direction in Direction::PLANE_HORIZONTAL {
            let test_pos = pos.relative(direction);
            let test_state = level.block_state(test_pos);
            let test_fluid = test_state.fluid_state();
            if !self.can_maybe_pass_through(level, pos, state, direction, test_pos, test_state, test_fluid) {
                continue;
            }
            let new_fluid = self.new_liquid(level, test_pos, test_state);
            if !can_hold_specific_fluid(level, test_pos, test_state, new_fluid.fluid) {
                continue;
            }
            let context = context.get_or_insert_with(|| SpreadContext::new(pos));
            let distance = if context.is_hole(self, level, test_pos) {
                0
            } else {
                self.slope_distance(level, test_pos, 1, direction.opposite(), test_state, context)
            };
            if distance < lowest {
                result.clear();
            }
            if distance <= lowest {
                if can_be_replaced_with(level, test_fluid, test_pos, new_fluid.fluid, direction) {
                    result.push((direction, new_fluid));
                }
                lowest = distance;
            }
        }
        result.sort_by_key(|(direction, _)| direction.ordinal());
        result
    }
}

pub struct SpreadContext {
    origin: BlockPos,
    state_cache: HashMap<i16, BlockState>,
    hole_cache: HashMap<i16, bool>,
}

impl SpreadContext {
    fn new(origin: BlockPos) -> SpreadContext {
        SpreadContext { origin, state_cache: HashMap::new(), hole_cache: HashMap::new() }
    }

    fn block_state(&mut self, level: &mut ServerLevel, pos: BlockPos) -> BlockState {
        let key = self.cache_key(pos);
        self.block_state_by_key(level, pos, key)
    }

    fn block_state_by_key(&mut self, level: &mut ServerLevel, pos: BlockPos, key: i16) -> BlockState {
        *self.state_cache.entry(key).or_insert_with(|| level.block_state(pos))
    }

    fn is_hole<F: FlowingFluid + ?Sized>(&mut self, fluid: &F, level: &mut ServerLevel, pos: BlockPos) -> bool {
        let key = self.cache_key(pos);
        if let Some(&hole) = self.hole_cache.get(&key) {
            return hole;
        }
        let state = self.block_state_by_key(level, pos, key);
        let below = pos.below();
        let below_state = level.block_state(below);
        let hole = fluid.is_water_hole(level, pos, state, below, below_state);
        self.hole_cache.insert(key, hole);
        hole
    }

    fn cache_key(&self, pos: BlockPos) -> i16 {
        let relative_x = pos.x - self.origin.x;
        let relative_z = pos.z - self.origin.z;
        (((relative_x + 128) & 0xFF) << 8 | ((relative_z + 128) & 0xFF)) as i16
    }
}

fn can_hold_any_fluid(state: BlockState) -> bool {
    block::of(state).is_liquid_block_container(state) || state.is(&block_tags::WASHED_AWAY_BY_FLUIDS)
}

fn can_hold_fluid(level: &mut ServerLevel, pos: BlockPos, state: BlockState, fluid: Fluid) -> bool {
    can_hold_any_fluid(state) && can_hold_specific_fluid(level, pos, state, fluid)
}

pub fn can_hold_specific_fluid(level: &mut ServerLevel, pos: BlockPos, state: BlockState, fluid: Fluid) -> bool {
    let behaviour = block::of(state);
    !behaviour.is_liquid_block_container(state) || behaviour.can_place_liquid(None, level, pos, state, fluid)
}

fn is_full_block(shape: &[AABB]) -> bool {
    shape.len() == 1 && shape[0] == AABB::new(0.0, 0.0, 0.0, 1.0, 1.0, 1.0)
}

fn can_pass_through_wall(direction: Direction, _source_pos: BlockPos, source_state: BlockState, _target_pos: BlockPos, target_state: BlockState) -> bool {
    let target_shape = target_state.collision_shape();
    if is_full_block(target_shape) {
        return false;
    }
    let source_shape = source_state.collision_shape();
    if is_full_block(source_shape) {
        return false;
    }
    if source_shape.is_empty() && target_shape.is_empty() {
        return true;
    }
    let cacheable = !source_state.block().info().dynamic_shape && !target_state.block().info().dynamic_shape;
    if cacheable {
        let cached = OCCLUSION_CACHE.with_borrow_mut(|cache| {
            let index = cache.iter().position(|e| e.0 == source_state && e.1 == target_state && e.2 == direction)?;
            let entry = cache.remove(index)?;
            cache.push_front(entry);
            Some(entry.3)
        });
        if let Some(result) = cached {
            return result;
        }
    }
    let result = !merged_face_occludes(source_shape, target_shape, direction);
    if cacheable {
        OCCLUSION_CACHE.with_borrow_mut(|cache| {
            if cache.len() == CACHE_SIZE {
                cache.pop_back();
            }
            cache.push_front((source_state, target_state, direction, result));
        });
    }
    result
}

fn merged_face_occludes(shape: &[AABB], occluder: &[AABB], direction: Direction) -> bool {
    if is_full_block(shape) || is_full_block(occluder) {
        return true;
    }
    let axis = direction.axis();
    let positive = direction.axis_direction() == AxisDirection::Positive;
    let (first, second) = if positive { (shape, occluder) } else { (occluder, shape) };
    let (first, second) = (VoxelShape::from_boxes(first), VoxelShape::from_boxes(second));
    let towards = |sign: AxisDirection| Direction::VALUES.into_iter().find(|d| d.axis() == axis && d.axis_direction() == sign).unwrap();
    let mut faces = Vec::new();
    if (first.max(axis) - 1.0).abs() <= 1.0e-7 {
        faces.extend(first.face_shape(towards(AxisDirection::Positive)).to_aabbs());
    }
    if second.min(axis).abs() <= 1.0e-7 {
        faces.extend(second.face_shape(towards(AxisDirection::Negative)).to_aabbs());
    }
    !join_is_not_empty(&VoxelShape::block(), &VoxelShape::from_boxes(&faces), BooleanOp::OnlyFirst)
}
