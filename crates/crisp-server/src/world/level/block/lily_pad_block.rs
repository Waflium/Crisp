use crisp_core::{BlockPos, Direction};
use crisp_data::{BlockState, Fluid, block_tags, fluid_tags};

use super::BlockBehaviour;
use super::vegetation_block::{self, Vegetation};
use crate::world::level::{LevelAccessor, LevelReader};

pub struct LilyPadBlock;

impl Vegetation for LilyPadBlock {
    fn may_place_on(&self, state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> bool {
        let fluid = level.fluid_state(pos);
        let fluid_above = level.fluid_state(pos.above());
        (fluid.is(&fluid_tags::SUPPORTS_LILY_PAD) || state.is(&block_tags::SUPPORTS_LILY_PAD)) && fluid_above.fluid == Fluid::Empty
    }
}

impl BlockBehaviour for LilyPadBlock {
    fn entity_inside(
        &self,
        _state: BlockState,
        level: &mut crate::server::level::ServerLevel,
        pos: BlockPos,
        entity: crate::world::entity::EntityId,
        _effects: &mut crate::world::entity::inside_block::InsideBlockEffects,
        _is_precise: bool,
    ) {
        if level.is_boat(entity) {
            level.destroy_block(pos, true, Some(entity), 512);
        }
    }

    fn update_shape(&self, state: BlockState, level: &mut dyn LevelAccessor, pos: BlockPos, _: Direction, _: BlockPos, _: BlockState) -> BlockState {
        vegetation_block::update_shape(state, level, pos)
    }

    fn can_survive(&self, _state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> bool {
        vegetation_block::can_survive(self, level, pos)
    }
}
