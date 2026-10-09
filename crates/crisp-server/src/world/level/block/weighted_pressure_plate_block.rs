use crisp_core::{BlockPos, Direction, mth};
use crisp_data::properties::POWER;
use crisp_data::{Block, BlockState};

use super::BlockBehaviour;
use super::base_pressure_plate_block::{self as base, BasePressurePlateBlock, PlateSounds, TOUCH_AABB};
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::inside_block::InsideBlockEffects;

pub struct WeightedPressurePlateBlock {
    pub block: Block,
    sounds: PlateSounds,
    max_weight: i32,
}

impl WeightedPressurePlateBlock {
    pub fn new(block: Block) -> WeightedPressurePlateBlock {
        let fields: serde_json::Value = serde_json::from_str(block.info().fields).unwrap_or_default();
        let max_weight = fields["maxWeight"].as_i64().unwrap_or(15) as i32;
        WeightedPressurePlateBlock { block, sounds: PlateSounds::from_fields(&fields), max_weight }
    }
}

impl BasePressurePlateBlock for WeightedPressurePlateBlock {
    fn block(&self) -> Block {
        self.block
    }

    fn sounds(&self) -> &PlateSounds {
        &self.sounds
    }

    fn get_pressed_time(&self) -> i32 {
        10
    }

    fn get_signal_strength(&self, level: &mut ServerLevel, pos: BlockPos) -> i32 {
        let count = base::get_entity_count(level, &TOUCH_AABB.move_to_block(pos), false).min(self.max_weight);
        if count > 0 {
            let percent = self.max_weight.min(count) as f32 / self.max_weight as f32;
            mth::ceil((percent * 15.0) as f64)
        } else {
            0
        }
    }

    fn get_signal_for_state(&self, state: BlockState) -> i32 {
        state.get(POWER)
    }

    fn set_signal_for_state(&self, state: BlockState, signal: i32) -> BlockState {
        state.set(POWER, signal)
    }
}

impl BlockBehaviour for WeightedPressurePlateBlock {
    fn update_shape(
        &self,
        state: BlockState,
        level: &mut ServerLevel,
        pos: BlockPos,
        direction: Direction,
        _neighbour_pos: BlockPos,
        _neighbour_state: BlockState,
    ) -> BlockState {
        base::update_shape(state, level, pos, direction)
    }

    fn can_survive(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos) -> bool {
        base::can_survive(level, pos)
    }

    fn tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        base::tick(self, state, level, pos);
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
        base::entity_inside(self, state, level, pos);
    }

    fn affect_neighbors_after_removal(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, moved_by_piston: bool) {
        base::affect_neighbors_after_removal(self, state, level, pos, moved_by_piston);
    }

    fn get_direct_signal(&self, state: BlockState, _level: &mut ServerLevel, _pos: BlockPos, direction: Direction) -> i32 {
        base::get_direct_signal(self, state, direction)
    }

    fn own_signal(&self, state: BlockState, _level: &mut ServerLevel, _pos: BlockPos) -> i32 {
        self.get_signal_for_state(state)
    }
}
