use crisp_core::{BlockPos, Direction};
use crisp_data::{Block, BlockState};

use super::BlockBehaviour;
use super::growing_plant_block::{self, GrowingPlant};
use super::vegetation_block::{block_can_be_replaced, schedule_water_tick};
use crate::server::level::ServerLevel;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::{BlockGetter, LevelReader};

#[allow(unused_variables)]
pub trait BodyParts: Send + Sync {
    fn head_block(&self) -> Block;

    fn can_attach_to(&self, state: BlockState) -> bool {
        true
    }

    fn head_can_grow_into(&self, state: BlockState) -> bool;

    fn update_head_after_converted_from_body(&self, body_state: BlockState, head_state: BlockState) -> BlockState {
        head_state
    }
}

pub struct GrowingPlantBodyBlock<P: BodyParts> {
    pub plant: GrowingPlant,
    pub parts: P,
}

impl<P: BodyParts> GrowingPlantBodyBlock<P> {
    pub fn new(block: Block, parts: P) -> GrowingPlantBodyBlock<P> {
        GrowingPlantBodyBlock { plant: GrowingPlant::new(block), parts }
    }

    fn head_pos(&self, level: &mut ServerLevel, pos: BlockPos, body_block: Block) -> Option<BlockPos> {
        let mut forward_pos = pos;
        let forward_state = loop {
            forward_pos = forward_pos.relative(self.plant.growth_direction);
            let state = level.block_state(forward_pos);
            if !state.is_block(body_block) {
                break state;
            }
        };
        forward_state.is_block(self.parts.head_block()).then_some(forward_pos)
    }
}

impl<P: BodyParts> BlockBehaviour for GrowingPlantBodyBlock<P> {
    fn get_clone_item_stack(&self, _level: &ServerLevel, _pos: BlockPos, _state: BlockState, _include_data: bool) -> crate::world::item::ItemStack {
        crate::world::item::ItemStack::new(self.parts.head_block().item(), 1)
    }

    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        Some(growing_plant_block::get_state_for_placement(&self.plant, self.parts.head_block(), block, ctx, level, |_| block.default_state()))
    }

    fn can_be_replaced(&self, state: BlockState, ctx: &BlockPlaceContext) -> bool {
        let result = block_can_be_replaced(state, ctx);
        if result && !ctx.item.is_empty() && ctx.item.item == self.parts.head_block().item() { false } else { result }
    }

    fn can_survive(&self, state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> bool {
        growing_plant_block::can_survive(&self.plant, self.parts.head_block(), state.block(), |s| self.parts.can_attach_to(s), level, pos)
    }

    fn tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        growing_plant_block::tick(state, level, pos);
    }

    fn update_shape(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, direction: Direction, _: BlockPos, neighbour_state: BlockState) -> BlockState {
        if direction == self.plant.growth_direction.opposite() && !self.can_survive(state, level, pos) {
            level.schedule_tick(pos, state.block(), 1);
        }
        let head = self.parts.head_block();
        if direction == self.plant.growth_direction && !neighbour_state.is_block(state.block()) && !neighbour_state.is_block(head) {
            let head_state = growing_plant_block::head_state_for_placement(head, level);
            return self.parts.update_head_after_converted_from_body(state, head_state);
        }
        if self.plant.schedule_fluid_ticks {
            schedule_water_tick(level, pos);
        }
        state
    }

    fn is_valid_bonemeal_target(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState) -> bool {
        let Some(head_pos) = self.head_pos(level, pos, state.block()) else { return false };
        let growth_pos = head_pos.relative(self.plant.growth_direction);
        self.parts.head_can_grow_into(level.block_state(growth_pos)) && !level.chunks.is_outside_build_height(growth_pos.y)
    }

    fn is_bonemeal_success(&self, _level: &mut ServerLevel, _pos: BlockPos, _state: BlockState) -> bool {
        true
    }

    fn perform_bonemeal(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState) {
        if let Some(head_pos) = self.head_pos(level, pos, state.block()) {
            let forward_state = level.block_state(head_pos);
            super::of(forward_state).perform_bonemeal(level, head_pos, forward_state);
        }
    }
}
