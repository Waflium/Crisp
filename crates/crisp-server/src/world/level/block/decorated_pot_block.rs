use crisp_core::{BlockPos, Direction};
use crisp_data::properties::{CRACKED, HORIZONTAL_FACING, WATERLOGGED};
use crisp_data::{Block, BlockState, Fluid, item_tags, registry, registry_id};

use super::mirror::Mirror;
use super::rotation::Rotation;
use super::simple_waterlogged_block::schedule_water_tick;
use super::{BlockBehaviour, BlockHitResult};
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;
use crate::world::item::context::BlockPlaceContext;
use crate::world::item::enchantment::enchantment_helper;
use crate::world::level::block::entity::BlockEntityKind;
use crate::world::level::block::entity::decorated_pot_block_entity::{DecoratedPotBlockEntity, WobbleStyle};
use crate::world::level::storage::loot::loot_params::LootParams;
use crate::world::level::update_flags::{UPDATE_INVISIBLE, UPDATE_SKIP_BLOCK_ENTITY_SIDEEFFECTS};
use crate::world::level::{BlockGetter, LevelAccessor};
use crate::world::{InteractionHand, InteractionResult};

pub const SHERDS_DYNAMIC_DROP_ID: &str = "minecraft:sherds";

fn sound(name: &str) -> i32 {
    registry_id(registry::SOUND_EVENT, name)
}

pub struct DecoratedPotBlock;

impl BlockBehaviour for DecoratedPotBlock {
    fn update_shape(&self, state: BlockState, level: &mut dyn LevelAccessor, pos: BlockPos, _: Direction, _: BlockPos, _: BlockState) -> BlockState {
        schedule_water_tick(state, level, pos);
        state
    }

    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        let replaced_fluid_state = level.fluid_state(ctx.clicked_pos());
        Some(
            block
                .default_state()
                .set(HORIZONTAL_FACING, ctx.horizontal_direction())
                .set(WATERLOGGED, replaced_fluid_state.fluid == Fluid::Water)
                .set(CRACKED, false),
        )
    }

    fn use_item_on(
        &self,
        item_stack: ItemStack,
        _state: BlockState,
        level: &mut ServerLevel,
        pos: BlockPos,
        player: EntityId,
        hand: InteractionHand,
        _hit: &BlockHitResult,
    ) -> InteractionResult {
        if level.decorated_pot(pos).is_none() {
            return InteractionResult::Pass;
        }
        let pot_item = level.decorated_pot_get_the_item(pos);
        if !item_stack.is_empty() && (pot_item.is_empty() || ItemStack::is_same_item_same_components(&pot_item, &item_stack) && pot_item.count < pot_item.max_stack_size()) {
            level.decorated_pot_wobble(pos, WobbleStyle::Positive);
            level.award_stat(player, crate::stats::stats::ITEM_USED.get(item_stack.item.0), 1);
            let mut item_stack = item_stack;
            let awarded_item = if level.has_infinite_materials(player) { item_stack.copy_with_count(1) } else { item_stack.split(1) };
            level.player_mut(player).set_item_in_hand(hand, Some(item_stack));
            let pitch_bend = if level.decorated_pot_get_the_item(pos).is_empty() {
                let pitch_bend = awarded_item.count as f32 / awarded_item.max_stack_size() as f32;
                level.decorated_pot_set_the_item(pos, awarded_item);
                pitch_bend
            } else {
                level.decorated_pot_grow_the_item(pos, 1);
                let pot_item = level.decorated_pot_get_the_item(pos);
                pot_item.count as f32 / pot_item.max_stack_size() as f32
            };
            level.play_sound(None, pos, sound("minecraft:block.decorated_pot.insert"), SoundSource::Blocks, 1.0, 0.7 + 0.5 * pitch_bend);
            level.send_particles("minecraft:dust_plume", |_| {}, pos.x as f64 + 0.5, pos.y as f64 + 1.2, pos.z as f64 + 0.5, 7, (0.0, 0.0, 0.0), 0.0);
            level.block_entity_set_changed(pos);
            InteractionResult::SUCCESS
        } else {
            InteractionResult::TryWithEmptyHand
        }
    }

    fn use_without_item(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos, _player: EntityId, _hit: &BlockHitResult) -> InteractionResult {
        if level.decorated_pot(pos).is_none() {
            return InteractionResult::Pass;
        }
        level.play_sound(None, pos, sound("minecraft:block.decorated_pot.insert_fail"), SoundSource::Blocks, 1.0, 1.0);
        level.decorated_pot_wobble(pos, WobbleStyle::Negative);
        InteractionResult::SUCCESS
    }

    fn affect_neighbors_after_removal(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, _moved_by_piston: bool) {
        crate::world::containers::update_neighbours_after_destroy(level, state, pos);
    }

    fn get_drops(&self, state: BlockState, mut params: LootParams) -> Vec<ItemStack> {
        if let Some(entity) = params.block_entity
            && let BlockEntityKind::DecoratedPot(pot) = &entity.kind
        {
            let sherds = pot.sherds();
            params.dynamic_drops.push((
                SHERDS_DYNAMIC_DROP_ID,
                Box::new(move |output: &mut dyn FnMut(ItemStack)| {
                    for sherd in &sherds {
                        output(sherd.clone());
                    }
                }),
            ));
        }
        super::block_get_drops(state, params)
    }

    fn player_will_destroy(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState, player: EntityId) -> BlockState {
        let destroyed_with = level.player(player).item_in_hand(InteractionHand::MainHand).unwrap_or_else(ItemStack::empty);
        let mut next_state = state;
        if destroyed_with.get_item().is(&item_tags::BREAKS_DECORATED_POTS)
            && !enchantment_helper::has_tag(&destroyed_with, "minecraft:prevents_decorated_pot_shattering")
        {
            next_state = state.set(CRACKED, true);
            level.set_block(pos, next_state, UPDATE_SKIP_BLOCK_ENTITY_SIDEEFFECTS | UPDATE_INVISIBLE);
        }
        super::block_player_will_destroy(level, pos, next_state, player)
    }

    fn get_clone_item_stack(&self, level: &ServerLevel, pos: BlockPos, state: BlockState, _include_data: bool) -> ItemStack {
        match level.chunks.block_entity_ref(pos).map(|entity| &entity.kind) {
            Some(BlockEntityKind::DecoratedPot(pot)) => DecoratedPotBlockEntity::create_decorated_pot_instance(pot.get_decorations()),
            _ => ItemStack::new(state.block().item(), 1),
        }
    }

    fn get_analog_output_signal(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos, _direction: Direction) -> i32 {
        level.get_redstone_signal_from_block_entity(pos)
    }

    fn trigger_event(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos, b0: i32, b1: i32) -> bool {
        let game_time = level.game_time;
        level.decorated_pot(pos).is_some_and(|pot| pot.trigger_event(b0, b1, game_time))
    }

    fn rotate(&self, state: BlockState, rotation: Rotation) -> BlockState {
        state.set(HORIZONTAL_FACING, rotation.rotate(state.get(HORIZONTAL_FACING)))
    }

    fn mirror(&self, state: BlockState, mirror: Mirror) -> BlockState {
        super::rotate(state, mirror.get_rotation(state.get(HORIZONTAL_FACING)))
    }
}
