use crisp_core::{BlockPos, Direction};
use crisp_data::properties::AGE_25;
use crisp_data::{Block, BlockState, SupportType};

use super::vegetation_block::fields;
use crate::server::level::ServerLevel;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::{BlockGetter, LevelAccessor, LevelReader};
use crate::world::level::update_flags::UPDATE_LIMIT;

pub struct GrowingPlant {
    pub growth_direction: Direction,
    pub schedule_fluid_ticks: bool,
}

impl GrowingPlant {
    pub fn new(block: Block) -> GrowingPlant {
        let fields = fields(block);
        GrowingPlant {
            growth_direction: fields["growthDirection"].as_str().and_then(Direction::by_name).unwrap_or(Direction::Up),
            schedule_fluid_ticks: fields["scheduleFluidTicks"].as_bool().unwrap_or(false),
        }
    }
}

pub fn head_state_for_placement(head: Block, level: &mut dyn LevelAccessor) -> BlockState {
    head.default_state().set(AGE_25, level.get_random().next_int_bounded(25))
}

pub fn get_state_for_placement(
    plant: &GrowingPlant,
    head: Block,
    body: Block,
    ctx: &BlockPlaceContext,
    level: &mut ServerLevel,
    own: impl FnOnce(&mut ServerLevel) -> BlockState,
) -> BlockState {
    let growth_direction_state = level.block_state(ctx.clicked_pos().relative(plant.growth_direction));
    if !growth_direction_state.is_block(head) && !growth_direction_state.is_block(body) { own(level) } else { body.default_state() }
}

pub fn can_survive(plant: &GrowingPlant, head: Block, body: Block, can_attach_to: impl Fn(BlockState) -> bool, level: &mut dyn LevelReader, pos: BlockPos) -> bool {
    let attached_to_pos = pos.relative(plant.growth_direction.opposite());
    let attached_to_state = level.block_state(attached_to_pos);
    if !can_attach_to(attached_to_state) {
        return false;
    }
    attached_to_state.is_block(head) || attached_to_state.is_block(body) || attached_to_state.is_face_sturdy(plant.growth_direction, SupportType::Full)
}

pub fn tick(state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
    if !super::of(state).can_survive(state, level, pos) {
        level.destroy_block(pos, true, None, UPDATE_LIMIT);
    }
}
