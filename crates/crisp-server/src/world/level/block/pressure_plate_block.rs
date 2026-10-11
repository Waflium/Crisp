use crisp_core::{BlockPos, Direction};
use crisp_data::properties::POWERED;
use crisp_data::{Block, BlockState};

use super::BlockBehaviour;
use super::base_pressure_plate_block::{self as base, BasePressurePlateBlock, PlateSounds, TOUCH_AABB};
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::inside_block::InsideBlockEffects;
use crate::world::level::LevelReader;

pub struct PressurePlateBlock {
    pub block: Block,
    sounds: PlateSounds,
    mobs_only: bool,
}

impl PressurePlateBlock {
    pub fn new(block: Block) -> PressurePlateBlock {
        let fields: serde_json::Value = serde_json::from_str(block.info().fields).unwrap_or_default();
        let mobs_only = fields["type"]["pressurePlateSensitivity"].as_str() == Some("MOBS");
        PressurePlateBlock { block, sounds: PlateSounds::from_fields(&fields), mobs_only }
    }
}

impl BasePressurePlateBlock for PressurePlateBlock {
    fn block(&self) -> Block {
        self.block
    }

    fn sounds(&self) -> &PlateSounds {
        &self.sounds
    }

    fn get_signal_strength(&self, level: &mut ServerLevel, pos: BlockPos) -> i32 {
        if base::get_entity_count(level, &TOUCH_AABB.move_to_block(pos), self.mobs_only) > 0 { 15 } else { 0 }
    }

    fn get_signal_for_state(&self, state: BlockState) -> i32 {
        if state.get(POWERED) { 15 } else { 0 }
    }

    fn set_signal_for_state(&self, state: BlockState, signal: i32) -> BlockState {
        state.set(POWERED, signal > 0)
    }
}

impl BlockBehaviour for PressurePlateBlock {
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

    fn can_survive(&self, _state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> bool {
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
