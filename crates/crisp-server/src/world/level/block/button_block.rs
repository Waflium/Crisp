use crisp_core::{BlockPos, Direction};
use crisp_data::properties::POWERED;
use crisp_data::{Block, BlockState, registry, registry_id};

use super::face_attached_horizontal_directional_block as face_attached;
use super::{BlockBehaviour, BlockHitResult};
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::InteractionResult;
use crate::world::entity::EntityId;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::{LevelAccessor, LevelReader};

pub struct ButtonBlock {
    pub block: Block,
    pub ticks_to_stay_pressed: i32,
    pub click_on: i32,
    pub click_off: i32,
}

impl ButtonBlock {
    pub fn new(block: Block) -> ButtonBlock {
        let fields: serde_json::Value = serde_json::from_str(block.info().fields).unwrap_or_default();
        let sound = |name: &str| registry_id(registry::SOUND_EVENT, fields["type"][name].as_str().unwrap_or_default());
        ButtonBlock {
            block,
            ticks_to_stay_pressed: fields["ticksToStayPressed"].as_i64().unwrap_or(20) as i32,
            click_on: sound("buttonClickOn"),
            click_off: sound("buttonClickOff"),
        }
    }

    pub fn press(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, player: Option<EntityId>) {
        level.set_block_simple(pos, state.set(POWERED, true));
        self.update_neighbours(state, level, pos);
        level.schedule_tick(pos, self.block, self.ticks_to_stay_pressed);
        self.play_sound(player, level, pos, true);
    }

    fn play_sound(&self, player: Option<EntityId>, level: &mut ServerLevel, pos: BlockPos, pressed: bool) {
        let (except, sound) = if pressed { (player, self.click_on) } else { (None, self.click_off) };
        level.play_sound(except, pos, sound, SoundSource::Blocks, 1.0, 1.0);
    }

    fn check_pressed(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        let should_be_pressed = false;
        let was_pressed = state.get(POWERED);
        if should_be_pressed != was_pressed {
            level.set_block_simple(pos, state.set(POWERED, should_be_pressed));
            self.update_neighbours(state, level, pos);
            self.play_sound(None, level, pos, should_be_pressed);
        }
    }

    fn update_neighbours(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        let front = face_attached::get_connected_direction(state).opposite();
        level.update_neighbors_at(pos, self.block);
        level.update_neighbors_at(pos.relative(front), self.block);
    }
}

impl BlockBehaviour for ButtonBlock {
    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        face_attached::get_state_for_placement(block, ctx, level)
    }

    fn can_survive(&self, state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> bool {
        face_attached::can_survive(state, level, pos)
    }

    fn update_shape(
        &self,
        state: BlockState,
        level: &mut dyn LevelAccessor,
        pos: BlockPos,
        direction: Direction,
        _neighbour_pos: BlockPos,
        _neighbour_state: BlockState,
    ) -> BlockState {
        face_attached::update_shape(state, level, pos, direction)
    }

    fn use_without_item(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, player: EntityId, _hit: &BlockHitResult) -> InteractionResult {
        if state.get(POWERED) {
            return InteractionResult::CONSUME;
        }
        self.press(state, level, pos, Some(player));
        InteractionResult::SUCCESS
    }

    fn affect_neighbors_after_removal(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, moved_by_piston: bool) {
        if !moved_by_piston && state.get(POWERED) {
            self.update_neighbours(state, level, pos);
        }
    }

    fn get_direct_signal(&self, state: BlockState, _level: &mut dyn LevelReader, _pos: BlockPos, direction: Direction) -> i32 {
        if state.get(POWERED) && face_attached::get_connected_direction(state) == direction { 15 } else { 0 }
    }

    fn own_signal(&self, state: BlockState, _level: &mut dyn LevelReader, _pos: BlockPos) -> i32 {
        if state.get(POWERED) { 15 } else { 0 }
    }

    fn tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        if state.get(POWERED) {
            self.check_pressed(state, level, pos);
        }
    }
}
