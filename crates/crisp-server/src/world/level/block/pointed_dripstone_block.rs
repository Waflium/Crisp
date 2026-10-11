use std::sync::LazyLock;

use crisp_core::random::RandomSource;
use crisp_core::shapes::{BooleanOp, VoxelShape, join_is_not_empty};
use crisp_core::{AABB, AxisDirection, BlockPos, Direction};
use crisp_data::properties::SpeleothemThickness;
use crisp_data::{Block, BlockState, Fluid, blocks, damage_types};

use super::speleothem_block::{self, SpeleothemBlock, THICKNESS, TIP_DIRECTION};
use super::BlockBehaviour;
use crate::server::level::ServerLevel;
use crate::server::level::events::DRIPSTONE_DRIP;
use crate::world::attribute::environment_attributes;
use crate::world::damagesource::DamageSource;
use crate::world::entity::EntityId;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::{BlockGetter, LevelAccessor, LevelReader};
use crate::world::level::update_flags::UPDATE_ALL;

const MAX_SEARCH_LENGTH_WHEN_CHECKING_DRIP_TYPE: i32 = 11;
const MAX_SEARCH_LENGTH_BETWEEN_STALACTITE_TIP_AND_CAULDRON: i32 = 11;
const WATER_TRANSFER_PROBABILITY_PER_RANDOM_TICK: f32 = 0.17578125;
const LAVA_TRANSFER_PROBABILITY_PER_RANDOM_TICK: f32 = 0.05859375;
const STALAGMITE_FALL_DISTANCE_OFFSET: f32 = 2.5;
const STALAGMITE_FALL_DAMAGE_MODIFIER: i32 = 2;
static REQUIRED_SPACE_TO_DRIP_THROUGH_NON_SOLID_BLOCK: LazyLock<VoxelShape> = LazyLock::new(|| VoxelShape::from_boxes(&[AABB::new(0.375, 0.0, 0.375, 0.625, 1.0, 0.625)]));

pub struct PointedDripstoneBlock {
    block: Block,
    block_to_grow_on: BlockState,
}

impl PointedDripstoneBlock {
    pub fn new(block: Block) -> PointedDripstoneBlock {
        PointedDripstoneBlock { block, block_to_grow_on: speleothem_block::block_to_grow_on(block) }
    }
}

impl SpeleothemBlock for PointedDripstoneBlock {
    fn block(&self) -> Block {
        self.block
    }

    fn block_to_grow_on(&self) -> BlockState {
        self.block_to_grow_on
    }

    fn can_grow(&self, level: &mut dyn LevelReader, pos: BlockPos) -> bool {
        let fluid_state = level.block_state(pos.offset(0, 2, 0)).fluid_state();
        speleothem_block::can_grow(self, level, pos) && fluid_state.fluid == Fluid::Water && fluid_state.is_source()
    }

    fn blocks_stalagmite_scan(&self, _level: &mut dyn LevelReader, _pos: BlockPos, state: BlockState) -> bool {
        !can_drip_through(state)
    }
}

impl BlockBehaviour for PointedDripstoneBlock {
    fn can_survive(&self, state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> bool {
        speleothem_block::can_survive(self, state, level, pos)
    }

    fn update_shape(&self, state: BlockState, level: &mut dyn LevelAccessor, pos: BlockPos, direction: Direction, _: BlockPos, _: BlockState) -> BlockState {
        speleothem_block::update_shape(self, state, level, pos, direction)
    }

    fn get_state_for_placement(&self, _block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        speleothem_block::get_state_for_placement(self, ctx, level)
    }

    fn fall_on(&self, level: &mut ServerLevel, state: BlockState, pos: BlockPos, entity: EntityId, fall_distance: f64) {
        if state.get(TIP_DIRECTION) == Direction::Up && state.get(THICKNESS) == SpeleothemThickness::Tip {
            level.cause_fall_damage(
                entity,
                fall_distance + STALAGMITE_FALL_DISTANCE_OFFSET as f64,
                STALAGMITE_FALL_DAMAGE_MODIFIER as f32,
                DamageSource::new(damage_types::STALAGMITE),
            );
        } else {
            super::block_fall_on(level, state, pos, entity, fall_distance);
        }
    }

    fn tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        speleothem_block::tick(self, state, level, pos);
    }

    fn random_tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        let random_value = level.random.next_float();
        maybe_transfer_fluid(state, level, pos, random_value);
        speleothem_block::random_tick(self, state, level, pos);
    }
}

pub fn maybe_transfer_fluid(state: BlockState, level: &mut ServerLevel, pos: BlockPos, random_value: f32) {
    if random_value > WATER_TRANSFER_PROBABILITY_PER_RANDOM_TICK && random_value > LAVA_TRANSFER_PROBABILITY_PER_RANDOM_TICK {
        return;
    }
    if !speleothem_block::is_stalactite_start_pos(state, level, pos) {
        return;
    }
    let Some(fluid_info) = get_fluid_above_stalactite(level, pos, state) else { return };
    let fluid = fluid_info.fluid;
    let transfer_probability = match fluid {
        Fluid::Water => WATER_TRANSFER_PROBABILITY_PER_RANDOM_TICK,
        Fluid::Lava => LAVA_TRANSFER_PROBABILITY_PER_RANDOM_TICK,
        _ => return,
    };
    if random_value >= transfer_probability {
        return;
    }
    let Some(stalactite_tip_pos) = speleothem_block::find_tip(state, level, pos, MAX_SEARCH_LENGTH_WHEN_CHECKING_DRIP_TYPE, false) else { return };
    if fluid_info.source_state.is_block(blocks::MUD) && fluid == Fluid::Water {
        let new_state = blocks::CLAY.default_state();
        level.set_block(fluid_info.pos, new_state, UPDATE_ALL);
        level.push_entities_up(fluid_info.source_state, new_state, fluid_info.pos);
        level.level_event(None, DRIPSTONE_DRIP, stalactite_tip_pos, 0);
    } else if let Some(cauldron_pos) = find_fillable_cauldron_below_stalactite_tip(level, stalactite_tip_pos, fluid) {
        level.level_event(None, DRIPSTONE_DRIP, stalactite_tip_pos, 0);
        let fall_distance = stalactite_tip_pos.y - cauldron_pos.y;
        let delay = 50 + fall_distance;
        let cauldron_state = level.block_state(cauldron_pos);
        level.schedule_tick(cauldron_pos, cauldron_state.block(), delay);
    }
}

fn find_root_block(level: &mut dyn LevelAccessor, pos: BlockPos, drip_stone_state: BlockState, max_search_length: i32) -> Option<BlockPos> {
    let tip_direction = drip_stone_state.get(TIP_DIRECTION);
    let block = drip_stone_state.block();
    speleothem_block::find_block_vertical(
        level,
        pos,
        tip_direction.opposite().axis_direction(),
        |_, state| state.is_block(block) && state.get(TIP_DIRECTION) == tip_direction,
        |state| !state.is_block(block),
        max_search_length,
    )
}

fn find_fillable_cauldron_below_stalactite_tip(level: &mut dyn LevelAccessor, stalactite_tip_pos: BlockPos, fluid: Fluid) -> Option<BlockPos> {
    speleothem_block::find_block_vertical(
        level,
        stalactite_tip_pos,
        AxisDirection::Negative,
        |_, state| can_drip_through(state),
        |state| state.block().is_instance_of("AbstractCauldronBlock") && super::of(state).can_receive_stalactite_drip(fluid),
        MAX_SEARCH_LENGTH_BETWEEN_STALACTITE_TIP_AND_CAULDRON,
    )
}

pub fn find_stalactite_tip_above_cauldron(level: &mut dyn LevelAccessor, cauldron_pos: BlockPos) -> Option<BlockPos> {
    speleothem_block::find_block_vertical(
        level,
        cauldron_pos,
        AxisDirection::Positive,
        |_, state| can_drip_through(state),
        speleothem_block::is_free_hanging_stalactite,
        MAX_SEARCH_LENGTH_BETWEEN_STALACTITE_TIP_AND_CAULDRON,
    )
}

pub fn get_cauldron_fill_fluid_type(level: &mut ServerLevel, stalactite_pos: BlockPos) -> Fluid {
    let stalactite_state = level.block_state(stalactite_pos);
    get_fluid_above_stalactite(level, stalactite_pos, stalactite_state).map(|fluid_source| fluid_source.fluid).filter(|&fluid| can_fill_cauldron(fluid)).unwrap_or(Fluid::Empty)
}

fn get_fluid_above_stalactite(level: &mut ServerLevel, stalactite_pos: BlockPos, stalactite_state: BlockState) -> Option<FluidInfo> {
    if !speleothem_block::is_stalactite(stalactite_state) {
        return None;
    }
    let root_pos = find_root_block(level, stalactite_pos, stalactite_state, MAX_SEARCH_LENGTH_WHEN_CHECKING_DRIP_TYPE)?;
    let above_pos = root_pos.above();
    let above_state = level.block_state(above_pos);
    let fluid = if above_state.is_block(blocks::MUD) && !level.environment_attributes().get_value_at(environment_attributes::WATER_EVAPORATES, above_pos) {
        Fluid::Water
    } else {
        level.fluid_state(above_pos).fluid
    };
    Some(FluidInfo { pos: above_pos, fluid, source_state: above_state })
}

fn can_fill_cauldron(fluid_above: Fluid) -> bool {
    fluid_above == Fluid::Lava || fluid_above == Fluid::Water
}

fn can_drip_through(state: BlockState) -> bool {
    if state.is_air() {
        return true;
    }
    if state.is_solid_render() {
        return false;
    }
    if !state.fluid_state().is_empty() {
        return false;
    }
    !join_is_not_empty(&REQUIRED_SPACE_TO_DRIP_THROUGH_NON_SOLID_BLOCK, state.collision_voxel_shape(), BooleanOp::And)
}

struct FluidInfo {
    pos: BlockPos,
    fluid: Fluid,
    source_state: BlockState,
}
