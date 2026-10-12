use crisp_core::{BlockPos, Direction};
use crisp_data::BlockState;
use crisp_data::properties::HAS_RECORD;

use super::{BlockBehaviour, BlockHitResult};
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;
use crate::world::item::component::values::typed_entity_data;
use crate::world::item::component::{self as dc};
use crate::world::item::jukebox_playable;
use crate::world::level::LevelReader;
use crate::world::level::block::entity::BlockEntityKind;
use crate::world::level::block::entity::jukebox_block_entity::SONG_ITEM_TAG_ID;
use crate::world::level::update_flags::UPDATE_CLIENTS;
use crate::world::{InteractionHand, InteractionResult};

pub struct JukeboxBlock;

impl BlockBehaviour for JukeboxBlock {
    fn set_placed_by(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState, _placer: Option<EntityId>, item: ItemStack) {
        if let Some(block_entity_data) = item.get(dc::BLOCK_ENTITY_DATA)
            && typed_entity_data(block_entity_data).1.get(SONG_ITEM_TAG_ID).is_some()
        {
            level.set_block(pos, state.set(HAS_RECORD, true), UPDATE_CLIENTS);
        }
    }

    fn use_without_item(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, _player: EntityId, _hit: &BlockHitResult) -> InteractionResult {
        if state.get(HAS_RECORD) && level.jukebox(pos).is_some() {
            level.jukebox_pop_out_the_item(pos);
            InteractionResult::SUCCESS
        } else {
            InteractionResult::Pass
        }
    }

    fn use_item_on(
        &self,
        item: ItemStack,
        state: BlockState,
        level: &mut ServerLevel,
        pos: BlockPos,
        player: EntityId,
        hand: InteractionHand,
        _hit: &BlockHitResult,
    ) -> InteractionResult {
        if state.get(HAS_RECORD) {
            return InteractionResult::TryWithEmptyHand;
        }
        let mut to_insert = item;
        let result = jukebox_playable::try_insert_into_jukebox(level, pos, &mut to_insert, player);
        if result.consumes_action() {
            level.player_mut(player).update_item_in_hand(hand, &to_insert);
        }
        if !result.consumes_action() { InteractionResult::TryWithEmptyHand } else { result }
    }

    fn affect_neighbors_after_removal(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, _moved_by_piston: bool) {
        crate::world::containers::update_neighbours_after_destroy(level, state, pos);
    }

    fn own_signal(&self, _state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> i32 {
        match level.get_block_entity(pos).map(|entity| &entity.kind) {
            Some(BlockEntityKind::Jukebox(jukebox)) if jukebox.jukebox_song_player.is_playing() => 15,
            _ => 0,
        }
    }

    fn get_analog_output_signal(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos, _direction: Direction) -> i32 {
        level.jukebox(pos).map_or(0, |jukebox| jukebox.get_comparator_output())
    }
}
