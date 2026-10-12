use crisp_core::{BlockPos, Direction};
use crisp_data::properties::HORIZONTAL_FACING;
use crisp_data::{Block, BlockState, blocks, damage_types};

use super::rotation::Rotation;
use super::{BlockBehaviour, BlockHitResult, falling_block};
use crate::server::level::ServerLevel;
use crate::server::level::events;
use crate::world::InteractionResult;
use crate::world::damagesource::DamageSource;
use crate::world::entity::EntityId;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::LevelAccessor;

const FALL_DAMAGE_PER_DISTANCE: f32 = 2.0;
const FALL_DAMAGE_MAX: i32 = 40;

pub struct AnvilBlock;

pub fn damage(block_state: BlockState) -> Option<BlockState> {
    if block_state.is_block(blocks::ANVIL) {
        Some(blocks::CHIPPED_ANVIL.default_state().set(HORIZONTAL_FACING, block_state.get(HORIZONTAL_FACING)))
    } else if block_state.is_block(blocks::CHIPPED_ANVIL) {
        Some(blocks::DAMAGED_ANVIL.default_state().set(HORIZONTAL_FACING, block_state.get(HORIZONTAL_FACING)))
    } else {
        None
    }
}

impl BlockBehaviour for AnvilBlock {
    fn rotate(&self, state: BlockState, rotation: Rotation) -> BlockState {
        state.set(HORIZONTAL_FACING, rotation.rotate(state.get(HORIZONTAL_FACING)))
    }

    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, _level: &mut ServerLevel) -> Option<BlockState> {
        Some(block.default_state().set(HORIZONTAL_FACING, ctx.horizontal_direction().clockwise()))
    }

    fn use_without_item(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos, player: EntityId, _hit: &BlockHitResult) -> InteractionResult {
        level.open_menu(player, crate::world::inventory::menu_provider::MenuProvider::Anvil(pos));
        InteractionResult::SUCCESS
    }

    fn on_place(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, _old_state: BlockState, _moved_by_piston: bool) {
        falling_block::on_place(level, pos, state.block(), falling_block::DEFAULT_DELAY_AFTER_PLACE);
    }

    fn update_shape(
        &self,
        state: BlockState,
        level: &mut dyn LevelAccessor,
        pos: BlockPos,
        _direction: Direction,
        _neighbour_pos: BlockPos,
        _neighbour_state: BlockState,
    ) -> BlockState {
        falling_block::update_shape(state, level, pos, falling_block::DEFAULT_DELAY_AFTER_PLACE)
    }

    fn tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        falling_block::tick(state, level, pos, |entity| entity.set_hurts_entities(FALL_DAMAGE_PER_DISTANCE, FALL_DAMAGE_MAX));
    }

    fn on_land(&self, level: &mut ServerLevel, pos: BlockPos, _state: BlockState, _replaced_block: BlockState, entity: EntityId) {
        if !level.entity(entity).is_silent() {
            level.level_event(None, events::SOUND_ANVIL_LAND, pos, 0);
        }
    }

    fn on_broken_after_fall(&self, level: &mut ServerLevel, pos: BlockPos, entity: EntityId) {
        if !level.entity(entity).is_silent() {
            level.level_event(None, events::SOUND_ANVIL_BROKEN, pos, 0);
        }
    }

    fn get_fall_damage_source(&self, entity: EntityId) -> DamageSource {
        DamageSource::entity(damage_types::FALLING_ANVIL, entity)
    }
}
