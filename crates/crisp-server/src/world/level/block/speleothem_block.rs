use crisp_core::random::RandomSource;
use crisp_core::{AxisDirection, BlockPos, Direction};
use crisp_data::properties::{EnumProperty, SPELEOTHEM_THICKNESS, SpeleothemThickness, VERTICAL_DIRECTION, WATERLOGGED};
use crisp_data::{Block, BlockState, Fluid, SupportType, block_tags, blocks, damage_types};

use super::simple_waterlogged_block::schedule_water_tick;
use super::vegetation_block::fields;
use crate::server::level::ServerLevel;
use crate::world::damagesource::DamageSource;
use crate::world::entity::EntityId;
use crate::world::entity::item::falling_block_entity;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::{BlockGetter, LevelAccessor, LevelReader};
use crate::world::level::update_flags::{UPDATE_ALL, UPDATE_LIMIT};

pub const TIP_DIRECTION: EnumProperty<Direction> = VERTICAL_DIRECTION;
pub const THICKNESS: EnumProperty<SpeleothemThickness> = SPELEOTHEM_THICKNESS;
const DELAY_BEFORE_FALLING: i32 = 2;
const GROWTH_PROBABILITY_PER_RANDOM_TICK: f32 = 0.011377778;
const MAX_GROWTH_LENGTH: i32 = 7;
const MAX_STALAGMITE_SEARCH_RANGE_WHEN_GROWING: i32 = 10;

#[allow(unused_variables)]
pub trait SpeleothemBlock {
    fn block(&self) -> Block;

    fn block_to_grow_on(&self) -> BlockState;

    fn can_grow(&self, level: &mut dyn LevelReader, pos: BlockPos) -> bool {
        can_grow(self, level, pos)
    }

    fn blocks_stalagmite_scan(&self, level: &mut dyn LevelReader, pos: BlockPos, state: BlockState) -> bool {
        false
    }

    fn get_max_growth_length(&self) -> i32 {
        MAX_GROWTH_LENGTH
    }

    fn get_stalactite_landing_sound(&self) -> i32;
}

pub fn block_to_grow_on(block: Block) -> BlockState {
    BlockState(fields(block)["blockToGrowOn"]["state"].as_u64().expect("speleothems grow on a block") as u16)
}

pub fn can_survive(this: &(impl SpeleothemBlock + ?Sized), state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> bool {
    is_valid_speleothem_placement(this, level, pos, state.get(TIP_DIRECTION))
}

pub fn update_shape(this: &(impl SpeleothemBlock + ?Sized), state: BlockState, level: &mut dyn LevelAccessor, pos: BlockPos, direction_to_neighbour: Direction) -> BlockState {
    schedule_water_tick(state, level, pos);
    if direction_to_neighbour != Direction::Up && direction_to_neighbour != Direction::Down {
        return state;
    }
    let tip_direction = state.get(TIP_DIRECTION);
    if tip_direction == Direction::Down && level.has_scheduled_tick(pos, this.block()) {
        return state;
    }
    if direction_to_neighbour == tip_direction.opposite() && !can_survive(this, state, level, pos) {
        if tip_direction == Direction::Down {
            level.schedule_tick(pos, this.block(), DELAY_BEFORE_FALLING);
        } else {
            level.schedule_tick(pos, this.block(), 1);
        }
        state
    } else {
        let merge_opposing_tips = state.get(THICKNESS) == SpeleothemThickness::TipMerge;
        let new_thickness = calculate_speleothem_thickness(this, level, pos, tip_direction, merge_opposing_tips);
        state.set(THICKNESS, new_thickness)
    }
}

pub fn get_state_for_placement(this: &(impl SpeleothemBlock + ?Sized), ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
    let pos = ctx.clicked_pos();
    let default_tip_direction = ctx.nearest_looking_vertical_direction().opposite();
    let tip_direction = calculate_tip_direction(this, level, pos, default_tip_direction)?;
    let merge_opposing_tips = !ctx.is_secondary_use_active();
    let thickness = calculate_speleothem_thickness(this, level, pos, tip_direction, merge_opposing_tips);
    Some(this.block().default_state().set(TIP_DIRECTION, tip_direction).set(THICKNESS, thickness).set(WATERLOGGED, level.fluid_state(pos).fluid == Fluid::Water))
}

fn calculate_tip_direction(this: &(impl SpeleothemBlock + ?Sized), level: &mut dyn LevelReader, pos: BlockPos, default_tip_direction: Direction) -> Option<Direction> {
    if is_valid_speleothem_placement(this, level, pos, default_tip_direction) {
        Some(default_tip_direction)
    } else if is_valid_speleothem_placement(this, level, pos, default_tip_direction.opposite()) {
        Some(default_tip_direction.opposite())
    } else {
        None
    }
}

pub fn calculate_speleothem_thickness(
    this: &(impl SpeleothemBlock + ?Sized),
    level: &mut dyn LevelReader,
    pos: BlockPos,
    tip_direction: Direction,
    merge_opposing_tips: bool,
) -> SpeleothemThickness {
    let base_direction = tip_direction.opposite();
    let in_front_state = level.block_state(pos.relative(tip_direction));
    if is_speleothem_with_direction(in_front_state, base_direction) && in_front_state.is_block(this.block()) {
        if !merge_opposing_tips && in_front_state.get(THICKNESS) != SpeleothemThickness::TipMerge { SpeleothemThickness::Tip } else { SpeleothemThickness::TipMerge }
    } else if !is_speleothem_with_direction(in_front_state, tip_direction) {
        SpeleothemThickness::Tip
    } else {
        let in_front_thickness = in_front_state.get(THICKNESS);
        if in_front_thickness != SpeleothemThickness::Tip && in_front_thickness != SpeleothemThickness::TipMerge {
            let behind_state = level.block_state(pos.relative(base_direction));
            if !is_speleothem_with_direction(behind_state, tip_direction) { SpeleothemThickness::Base } else { SpeleothemThickness::Middle }
        } else {
            SpeleothemThickness::Frustum
        }
    }
}

fn is_valid_speleothem_placement(this: &(impl SpeleothemBlock + ?Sized), level: &mut dyn LevelReader, pos: BlockPos, tip_direction: Direction) -> bool {
    let behind_pos = pos.relative(tip_direction.opposite());
    let behind_state = level.block_state(behind_pos);
    behind_state.is_face_sturdy(tip_direction, SupportType::Full) || is_speleothem_with_direction(behind_state, tip_direction) && behind_state.is_block(this.block())
}

fn is_speleothem_with_direction(block_state: BlockState, tip_direction: Direction) -> bool {
    block_state.is(&block_tags::SPELEOTHEMS) && block_state.get(TIP_DIRECTION) == tip_direction
}

pub fn tick(this: &(impl SpeleothemBlock + ?Sized), state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
    if is_stalagmite(state) && !can_survive(this, state, level, pos) {
        level.destroy_block(pos, true, None, UPDATE_LIMIT);
    } else {
        spawn_falling_stalactite(state, level, pos);
    }
}

fn spawn_falling_stalactite(state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
    let mut fall_pos = pos;
    let mut fall_state = state;
    while is_stalactite(fall_state) {
        let entity = falling_block_entity::fall(level, fall_pos, fall_state);
        if is_tip(fall_state, true) {
            let size = (1 + pos.y - fall_pos.y).max(6);
            let damage_per_fall_distance = 1.0 * size as f32;
            level.falling_block(entity).set_hurts_entities(damage_per_fall_distance, 40);
            break;
        }
        fall_pos = fall_pos.below();
        fall_state = level.block_state(fall_pos);
    }
}

pub fn on_broken_after_fall(this: &(impl SpeleothemBlock + ?Sized), level: &mut ServerLevel, pos: BlockPos, entity: EntityId) {
    if !level.entity(entity).is_silent() {
        level.level_event(None, this.get_stalactite_landing_sound(), pos, 0);
    }
}

pub fn get_fall_damage_source(entity: EntityId) -> DamageSource {
    DamageSource::entity(damage_types::FALLING_STALACTITE, entity)
}

fn is_stalagmite(state: BlockState) -> bool {
    is_speleothem_with_direction(state, Direction::Up)
}

pub fn is_stalactite(state: BlockState) -> bool {
    is_speleothem_with_direction(state, Direction::Down)
}

fn is_tip(state: BlockState, include_merged_tip: bool) -> bool {
    if !state.is(&block_tags::SPELEOTHEMS) {
        return false;
    }
    let thickness = state.get(THICKNESS);
    thickness == SpeleothemThickness::Tip || include_merged_tip && thickness == SpeleothemThickness::TipMerge
}

pub fn random_tick(this: &(impl SpeleothemBlock + ?Sized), state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
    if level.random.next_float() < GROWTH_PROBABILITY_PER_RANDOM_TICK && is_stalactite_start_pos(state, level, pos) {
        grow_stalactite_or_stalagmite_if_possible(this, state, level, pos);
    }
}

pub fn is_stalactite_start_pos(state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> bool {
    is_stalactite(state) && !level.block_state(pos.above()).is_block(state.block())
}

pub fn grow_stalactite_or_stalagmite_if_possible(this: &(impl SpeleothemBlock + ?Sized), stalactite_start_state: BlockState, level: &mut ServerLevel, stalactite_start_pos: BlockPos) {
    if this.can_grow(level, stalactite_start_pos) {
        let Some(stalactite_tip_pos) = find_tip(stalactite_start_state, level, stalactite_start_pos, this.get_max_growth_length(), false) else { return };
        let stalactite_tip_state = level.block_state(stalactite_tip_pos);
        if is_free_hanging_stalactite(stalactite_tip_state) && can_tip_grow(this, stalactite_tip_state, level, stalactite_tip_pos) {
            if level.random.next_boolean() {
                grow(this, level, stalactite_tip_pos, Direction::Down);
            } else {
                grow_stalagmite_below(this, level, stalactite_tip_pos);
            }
        }
    }
}

pub fn can_grow(this: &(impl SpeleothemBlock + ?Sized), level: &mut dyn LevelReader, pos: BlockPos) -> bool {
    level.block_state(pos.above()).is_block(this.block_to_grow_on().block())
}

pub fn find_tip(speleothem_state: BlockState, level: &mut dyn LevelAccessor, speleothem_pos: BlockPos, max_search_length: i32, include_merged_tip: bool) -> Option<BlockPos> {
    if is_tip(speleothem_state, include_merged_tip) {
        return Some(speleothem_pos);
    }
    let search_direction = speleothem_state.get(TIP_DIRECTION);
    let block = speleothem_state.block();
    find_block_vertical(
        level,
        speleothem_pos,
        search_direction.axis_direction(),
        |_, state| state.is_block(block) && state.get(TIP_DIRECTION) == search_direction,
        |speleothem| is_tip(speleothem, include_merged_tip),
        max_search_length,
    )
}

pub fn find_block_vertical(
    level: &mut dyn LevelAccessor,
    pos: BlockPos,
    axis_direction: AxisDirection,
    mut path_predicate: impl FnMut(BlockPos, BlockState) -> bool,
    mut target_predicate: impl FnMut(BlockState) -> bool,
    max_steps: i32,
) -> Option<BlockPos> {
    let direction = Direction::from_axis_and_direction(crisp_core::Axis::Y, axis_direction);
    let mut mutable_pos = pos;
    for _ in 1..max_steps {
        mutable_pos = mutable_pos.relative(direction);
        let state = level.block_state(mutable_pos);
        if target_predicate(state) {
            return Some(mutable_pos);
        }
        if !level.is_inside_build_height(mutable_pos.y) || !path_predicate(mutable_pos, state) {
            return None;
        }
    }
    None
}

fn can_tip_grow(this: &(impl SpeleothemBlock + ?Sized), tip_state: BlockState, level: &mut ServerLevel, tip_pos: BlockPos) -> bool {
    let grow_direction = tip_state.get(TIP_DIRECTION);
    let grow_pos = tip_pos.relative(grow_direction);
    let state_at_grow_pos = level.block_state(grow_pos);
    if !state_at_grow_pos.fluid_state().is_empty() {
        false
    } else {
        state_at_grow_pos.is_air() || is_unmerged_tip_with_direction(this, state_at_grow_pos, grow_direction.opposite())
    }
}

fn is_unmerged_tip_with_direction(this: &(impl SpeleothemBlock + ?Sized), state: BlockState, tip_direction: Direction) -> bool {
    is_tip(state, false) && state.get(TIP_DIRECTION) == tip_direction && state.is_block(this.block())
}

fn grow(this: &(impl SpeleothemBlock + ?Sized), level: &mut ServerLevel, grow_from_pos: BlockPos, grow_to_direction: Direction) {
    let target_pos = grow_from_pos.relative(grow_to_direction);
    let existing_state_at_target_pos = level.block_state(target_pos);
    if is_unmerged_tip_with_direction(this, existing_state_at_target_pos, grow_to_direction.opposite()) {
        create_merged_tips(this, existing_state_at_target_pos, level, target_pos);
    } else if existing_state_at_target_pos.is_air() || existing_state_at_target_pos.is_block(blocks::WATER) {
        create_speleothem(this, level, target_pos, grow_to_direction, SpeleothemThickness::Tip);
    }
}

fn create_speleothem(this: &(impl SpeleothemBlock + ?Sized), level: &mut dyn LevelAccessor, pos: BlockPos, direction: Direction, thickness: SpeleothemThickness) {
    let state = this.block().default_state().set(TIP_DIRECTION, direction).set(THICKNESS, thickness).set(WATERLOGGED, level.fluid_state(pos).fluid == Fluid::Water);
    level.set_block(pos, state, UPDATE_ALL);
}

fn create_merged_tips(this: &(impl SpeleothemBlock + ?Sized), tip_state: BlockState, level: &mut dyn LevelAccessor, tip_pos: BlockPos) {
    let (stalactite_pos, stalagmite_pos) = if tip_state.get(TIP_DIRECTION) == Direction::Up { (tip_pos.above(), tip_pos) } else { (tip_pos, tip_pos.below()) };
    create_speleothem(this, level, stalactite_pos, Direction::Down, SpeleothemThickness::TipMerge);
    create_speleothem(this, level, stalagmite_pos, Direction::Up, SpeleothemThickness::TipMerge);
}

fn grow_stalagmite_below(this: &(impl SpeleothemBlock + ?Sized), level: &mut ServerLevel, pos_above_stalagmite: BlockPos) {
    let mut pos = pos_above_stalagmite;
    for _ in 0..MAX_STALAGMITE_SEARCH_RANGE_WHEN_GROWING {
        pos = pos.below();
        let state = level.block_state(pos);
        if !state.fluid_state().is_empty() {
            return;
        }
        if is_unmerged_tip_with_direction(this, state, Direction::Up) && can_tip_grow(this, state, level, pos) {
            grow(this, level, pos, Direction::Up);
            return;
        }
        if is_valid_speleothem_placement(this, level, pos, Direction::Up) && !level.is_water_at(pos.below()) {
            grow(this, level, pos.below(), Direction::Up);
            return;
        }
        if this.blocks_stalagmite_scan(level, pos, state) {
            return;
        }
    }
}

pub fn is_free_hanging_stalactite(state: BlockState) -> bool {
    is_stalactite(state) && state.get(THICKNESS) == SpeleothemThickness::Tip && !state.get(WATERLOGGED)
}
