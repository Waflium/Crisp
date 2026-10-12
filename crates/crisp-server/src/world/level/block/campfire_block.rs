use crisp_core::shapes::{BooleanOp, VoxelShape, join_is_not_empty};
use crisp_core::{AABB, BlockPos, Direction};
use crisp_data::properties::{HORIZONTAL_FACING, LIT, SIGNAL_FIRE, WATERLOGGED};
use crisp_data::{Block, BlockState, EquipmentSlot, Fluid, FluidState, block_tags, blocks, damage_types, item_tags, registry, registry_id};

use super::mirror::Mirror;
use super::rotation::Rotation;
use super::simple_waterlogged_block::schedule_water_tick;
use super::vegetation_block::fields;
use super::{BlockBehaviour, BlockHitResult};
use crate::server::level::events::SOUND_EXTINGUISH_FIRE;
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::damagesource::DamageSource;
use crate::world::entity::EntityId;
use crate::world::entity::inside_block::InsideBlockEffects;
use crate::world::item::ItemStack;
use crate::world::item::context::BlockPlaceContext;
use crate::world::item::crafting::recipe_manager::RecipeManager;
use crate::world::item::crafting::recipe_property_set::CAMPFIRE_INPUT;
use crate::world::level::BlockGetter;
use crate::world::level::LevelAccessor;
use crate::world::level::block::entity::BlockEntityKind;
use crate::world::level::material::tick_delay;
use crate::world::level::update_flags::UPDATE_ALL;
use crate::world::{InteractionHand, InteractionResult};

pub struct CampfireBlock {
    fire_damage: f32,
}

impl CampfireBlock {
    pub fn new(block: Block) -> CampfireBlock {
        CampfireBlock { fire_damage: fields(block)["fireDamage"].as_f64().unwrap_or(0.0) as f32 }
    }

    fn is_smoke_source(&self, block_state: BlockState) -> bool {
        block_state.is_block(blocks::HAY_BLOCK)
    }
}

impl BlockBehaviour for CampfireBlock {
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
        if matches!(level.get_block_entity(pos).map(|e| &e.kind), Some(BlockEntityKind::Campfire(_))) {
            let mut item_in_hand = level.player(player).item_in_hand(hand).unwrap_or_else(ItemStack::empty);
            if RecipeManager::get().property_set(CAMPFIRE_INPUT).test(&item_in_hand) {
                if level.campfire_place_food(pos, Some(player), &mut item_in_hand) {
                    level.player_mut(player).set_item_in_hand(hand, Some(item_in_hand));
                    return InteractionResult::SUCCESS_SERVER;
                }
                return InteractionResult::CONSUME;
            }
        }
        if item_stack.get_item().is(&item_tags::DOUSES_CAMPFIRES) && state.get(LIT) {
            level.level_event(None, SOUND_EXTINGUISH_FIRE, pos, 0);
            level.set_block(pos, state.set(LIT, false), UPDATE_ALL);
            let slot = if hand == InteractionHand::MainHand { EquipmentSlot::Mainhand } else { EquipmentSlot::Offhand };
            level.hurt_and_break(player, slot, 1);
            InteractionResult::SUCCESS
        } else {
            InteractionResult::TryWithEmptyHand
        }
    }

    fn entity_inside(&self, state: BlockState, level: &mut ServerLevel, _pos: BlockPos, entity: EntityId, _effects: &mut InsideBlockEffects, _is_precise: bool) {
        if state.get(LIT) && level.is_living(entity) {
            level.hurt_server(entity, DamageSource::new(damage_types::CAMPFIRE), self.fire_damage);
        }
    }

    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        let pos = ctx.clicked_pos();
        let replaced_water = level.fluid_state(pos).fluid == Fluid::Water;
        let below = level.block_state(pos.below());
        Some(
            block
                .default_state()
                .set(WATERLOGGED, replaced_water)
                .set(SIGNAL_FIRE, self.is_smoke_source(below))
                .set(LIT, !replaced_water)
                .set(HORIZONTAL_FACING, ctx.horizontal_direction()),
        )
    }

    fn update_shape(&self, state: BlockState, level: &mut dyn LevelAccessor, pos: BlockPos, direction: Direction, _: BlockPos, neighbour_state: BlockState) -> BlockState {
        schedule_water_tick(state, level, pos);
        if direction == Direction::Down { state.set(SIGNAL_FIRE, self.is_smoke_source(neighbour_state)) } else { state }
    }

    fn place_liquid(&self, level: &mut dyn LevelAccessor, pos: BlockPos, state: BlockState, fluid_state: FluidState) -> bool {
        if !state.get(WATERLOGGED) && fluid_state.fluid == Fluid::Water {
            let is_lit = state.get(LIT);
            if is_lit {
                level.play_sound(pos, registry_id(registry::SOUND_EVENT, "minecraft:block.fire.extinguish"), SoundSource::Blocks, 1.0, 1.0);
            }
            level.set_block(pos, state.set(WATERLOGGED, true).set(LIT, false), UPDATE_ALL);
            let delay = tick_delay(level, fluid_state.fluid);
            level.schedule_fluid_tick(pos, fluid_state.fluid, delay);
            true
        } else {
            false
        }
    }

    fn rotate(&self, state: BlockState, rotation: Rotation) -> BlockState {
        state.set(HORIZONTAL_FACING, rotation.rotate(state.get(HORIZONTAL_FACING)))
    }

    fn mirror(&self, state: BlockState, mirror: Mirror) -> BlockState {
        super::rotate(state, mirror.get_rotation(state.get(HORIZONTAL_FACING)))
    }
}

fn shape_virtual_post() -> VoxelShape {
    VoxelShape::from_boxes(&[AABB::new(0.375, 0.0, 0.375, 0.625, 1.0, 0.625)])
}

pub fn is_smokey_pos(level: &mut ServerLevel, pos: BlockPos) -> bool {
    for i in 1..=5 {
        let pos_to_check = pos.offset(0, -i, 0);
        let block_state = level.block_state(pos_to_check);
        if is_lit_campfire(block_state) {
            return true;
        }
        let smoke_blocked = join_is_not_empty(&shape_virtual_post(), &VoxelShape::from_boxes(block_state.collision_shape()), BooleanOp::And);
        if smoke_blocked {
            let below_state = level.block_state(pos_to_check.below());
            return is_lit_campfire(below_state);
        }
    }
    false
}

pub fn can_light(state: BlockState) -> bool {
    state.is(&block_tags::CAMPFIRES) && state.has(WATERLOGGED) && state.has(LIT) && !state.get(WATERLOGGED) && !state.get(LIT)
}

pub fn is_lit_campfire(block_state: BlockState) -> bool {
    block_state.has(LIT) && block_state.is(&block_tags::CAMPFIRES) && block_state.get(LIT)
}
