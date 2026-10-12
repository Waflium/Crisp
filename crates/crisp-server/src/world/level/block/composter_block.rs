use crisp_core::{BlockPos, Direction, Vec3};
use crisp_data::properties::LEVEL_COMPOSTER;
use crisp_data::{BlockState, items, registry, registry_id};

use super::{BlockBehaviour, BlockHitResult};
use crate::server::level::events::COMPOSTER_FILL;
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::entity::{EntityId, ItemEntity, LevelEntity};
use crate::world::item::ItemStack;
use crate::world::item::component::{self as dc};
use crate::world::level::BlockGetter;
use crate::world::level::storage::loot::loot_context::LootContextBuilder;
use crate::world::level::storage::loot::loot_params::LootParams;
use crate::world::level::storage::loot::providers::number::ints::resolvable_int::ResolvableInt;
use crate::world::level::update_flags::UPDATE_ALL;
use crate::world::{InteractionHand, InteractionResult};

pub struct ComposterBlock;

impl BlockBehaviour for ComposterBlock {
    fn on_place(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, _old_state: BlockState, _moved_by_piston: bool) {
        if state.get(LEVEL_COMPOSTER) == 7 {
            level.schedule_tick(pos, state.block(), 20);
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
        let fill_level = state.get(LEVEL_COMPOSTER);
        if fill_level < 8 && item.has(dc::COMPOSTABLE) {
            if fill_level < 7 {
                let new_state = add_layer(Some(player), state, level, pos, &item);
                level.level_event(None, COMPOSTER_FILL, pos, if state != new_state { 1 } else { 0 });
                level.award_stat(player, crate::stats::stats::ITEM_USED.get(item.item.0), 1);
                let mut item = item;
                item.consume(1, level, Some(player));
                level.player_mut(player).update_item_in_hand(hand, &item);
            }
            InteractionResult::SUCCESS
        } else {
            InteractionResult::TryWithEmptyHand
        }
    }

    fn use_without_item(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, _player: EntityId, _hit: &BlockHitResult) -> InteractionResult {
        if state.get(LEVEL_COMPOSTER) == 8 {
            extract_produce(level, pos);
            InteractionResult::SUCCESS
        } else {
            InteractionResult::Pass
        }
    }

    fn tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        if state.get(LEVEL_COMPOSTER) == 7 {
            level.set_block(pos, state.cycle(LEVEL_COMPOSTER), UPDATE_ALL);
            level.play_sound(None, pos, registry_id(registry::SOUND_EVENT, "minecraft:block.composter.ready"), SoundSource::Blocks, 1.0, 1.0);
        }
    }

    fn get_analog_output_signal(&self, state: BlockState, _level: &mut ServerLevel, _pos: BlockPos, _direction: Direction) -> i32 {
        state.get(LEVEL_COMPOSTER)
    }
}

pub fn extract_produce(level: &mut ServerLevel, pos: BlockPos) {
    let item_pos = Vec3::at_lower_corner_with_offset(pos, 0.5, 1.01, 0.5).offset_random_xz(&mut level.random, 0.7);
    let mut entity = ItemEntity::new(item_pos, ItemStack::new(items::BONE_MEAL, 1));
    entity.set_default_pickup_delay();
    level.add_fresh_entity(LevelEntity::Item(entity));
    empty(level, pos);
    level.play_sound(None, pos, registry_id(registry::SOUND_EVENT, "minecraft:block.composter.empty"), SoundSource::Blocks, 1.0, 1.0);
}

pub fn empty(level: &mut ServerLevel, pos: BlockPos) {
    let new_state = level.block_state(pos).set(LEVEL_COMPOSTER, 0);
    level.set_block(pos, new_state, UPDATE_ALL);
}

pub fn insert_through_container(level: &mut ServerLevel, pos: BlockPos, contents: &ItemStack) {
    if !contents.is_empty() && contents.has(dc::COMPOSTABLE) {
        let state = level.block_state(pos);
        let new_state = add_layer(None, state, level, pos, contents);
        level.level_event(None, COMPOSTER_FILL, pos, if new_state != state { 1 } else { 0 });
    }
}

fn add_layer(source_entity: Option<EntityId>, state: BlockState, level: &mut ServerLevel, pos: BlockPos, item: &ItemStack) -> BlockState {
    let fill_level = state.get(LEVEL_COMPOSTER);
    let layers_to_add = {
        let mut params = LootParams::new(level);
        params.block_state = Some(state);
        params.origin = Some(Vec3::at_center_of(pos));
        params.interacting_entity = source_entity;
        let mut context = LootContextBuilder::new(params).create(None);
        ResolvableInt::get_from_item(item, dc::COMPOSTABLE, |compostable| compostable.get("layers"), &mut context, 0)
    };
    if layers_to_add > 0 {
        let new_level = (fill_level + layers_to_add).clamp(0, 7);
        let new_state = state.set(LEVEL_COMPOSTER, new_level);
        level.set_block(pos, new_state, UPDATE_ALL);
        if new_level == 7 {
            level.schedule_tick(pos, state.block(), 20);
        }
        new_state
    } else {
        state
    }
}
