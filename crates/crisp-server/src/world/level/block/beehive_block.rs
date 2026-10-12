use crisp_core::BlockPos;
use crisp_data::properties::{HORIZONTAL_FACING, LEVEL_HONEY};
use crisp_data::{BlockState, EquipmentSlot, items, registry, registry_id};

use super::mirror::Mirror;
use super::rotation::Rotation;
use super::{BlockBehaviour, BlockHitResult, campfire_block};
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;
use crate::world::item::component::values::block_state_properties_nbt;
use crate::world::item::component::{self as dc};
use crate::world::item::item_utils::has_infinite_materials;
use crate::world::level::block;
use crate::world::level::block::entity::beehive_block_entity::BeeReleaseStatus;
use crate::world::level::update_flags::UPDATE_ALL;
use crate::world::{InteractionHand, InteractionResult};

pub fn drop_honeycomb(level: &mut ServerLevel, tool: &ItemStack, block_state: BlockState, entity: Option<EntityId>, pos: BlockPos) {
    let block_entity = level.get_block_entity(pos).cloned();
    block::drop_from_block_interact_loot_table(level, "minecraft:harvest/beehive", pos, block_state, block_entity.as_ref(), Some(tool), entity, |level, stack| {
        level.pop_resource(pos, stack)
    });
}

pub fn release_bees_and_reset_honey_level(level: &mut ServerLevel, state: BlockState, pos: BlockPos, _player: Option<EntityId>, _bee_release_status: BeeReleaseStatus) {
    reset_honey_level(level, state, pos);
}

pub fn reset_honey_level(level: &mut ServerLevel, state: BlockState, pos: BlockPos) {
    level.set_block(pos, state.set(LEVEL_HONEY, 0), UPDATE_ALL);
}

pub struct BeehiveBlock;

impl BlockBehaviour for BeehiveBlock {
    fn use_item_on(
        &self,
        item_stack: ItemStack,
        state: BlockState,
        level: &mut ServerLevel,
        pos: BlockPos,
        player: EntityId,
        hand: InteractionHand,
        _hit: &BlockHitResult,
    ) -> InteractionResult {
        let honey_level = state.get(LEVEL_HONEY);
        let mut hive_emptied = false;
        if honey_level >= 5 {
            let player_pos = level.entity(player).position();
            if item_stack.is(items::SHEARS) {
                drop_honeycomb(level, &item_stack, state, Some(player), pos);
                level.play_sound_at(None, player_pos.x, player_pos.y, player_pos.z, registry_id(registry::SOUND_EVENT, "minecraft:block.beehive.shear"), SoundSource::Blocks, 1.0, 1.0);
                let slot = if hand == InteractionHand::MainHand { EquipmentSlot::Mainhand } else { EquipmentSlot::Offhand };
                level.hurt_and_break(player, slot, 1);
                hive_emptied = true;
            } else if item_stack.is(items::GLASS_BOTTLE) {
                let creative = has_infinite_materials(level, player);
                let mut item_stack = item_stack;
                item_stack.shrink(1);
                level.play_sound_at(Some(player), player_pos.x, player_pos.y, player_pos.z, registry_id(registry::SOUND_EVENT, "minecraft:item.bottle.fill"), SoundSource::Blocks, 1.0, 1.0);
                let honey_bottle = ItemStack::new(items::HONEY_BOTTLE, 1);
                if item_stack.is_empty() {
                    level.player_mut(player).set_item_in_hand(hand, Some(honey_bottle));
                } else {
                    if !creative {
                        level.player_mut(player).update_item_in_hand(hand, &item_stack);
                    }
                    if level.player_mut(player).pick_up(honey_bottle.clone()) > 0 {
                        level.player_drop(player, honey_bottle, false, crate::util::Prediction::Predicted);
                    }
                }
                hive_emptied = true;
            }
        }
        if hive_emptied {
            if !campfire_block::is_smokey_pos(level, pos) {
                release_bees_and_reset_honey_level(level, state, pos, Some(player), BeeReleaseStatus::Emergency);
            } else {
                reset_honey_level(level, state, pos);
            }
            InteractionResult::SUCCESS
        } else {
            InteractionResult::TryWithEmptyHand
        }
    }

    fn get_analog_output_signal(&self, state: BlockState, _level: &mut ServerLevel, _pos: BlockPos, _direction: crisp_core::Direction) -> i32 {
        state.get(LEVEL_HONEY)
    }

    fn rotate(&self, state: BlockState, rotation: Rotation) -> BlockState {
        state.set(HORIZONTAL_FACING, rotation.rotate(state.get(HORIZONTAL_FACING)))
    }

    fn mirror(&self, state: BlockState, mirror: Mirror) -> BlockState {
        super::rotate(state, mirror.get_rotation(state.get(HORIZONTAL_FACING)))
    }

    fn get_clone_item_stack(&self, _level: &ServerLevel, _pos: BlockPos, state: BlockState, include_data: bool) -> ItemStack {
        let mut stack = ItemStack::new(state.block().item(), 1);
        if include_data {
            stack.set(dc::BLOCK_STATE, Some(block_state_properties_nbt(&[("honey_level", &state.get(LEVEL_HONEY).to_string())])));
        }
        stack
    }
}
