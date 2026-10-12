use crisp_core::BlockPos;
use crisp_core::random::RandomSource;
use crisp_data::properties::{EGGS, HATCH};
use crisp_data::{Block, BlockState, block_tags, blocks, game_rule, registry, registry_id};

use super::{BlockBehaviour, block_fall_on};
use crate::server::level::events::{PARTICLES_AND_SOUND_DESTROY_BLOCK, PARTICLES_TURTLE_EGG_PLACEMENT};
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::attribute::environment_attributes::TURTLE_EGG_HATCH_CHANCE;
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::block::entity::BlockEntity;
use crate::world::level::update_flags::{UPDATE_CLIENTS, UPDATE_LIMIT};
use crate::world::level::BlockGetter;

pub const MAX_HATCH_LEVEL: i32 = 2;
pub const MAX_EGGS: i32 = 4;

pub fn on_sand(level: &mut ServerLevel, pos: BlockPos) -> bool {
    is_sand(level, pos.below())
}

pub fn is_sand(level: &mut ServerLevel, pos: BlockPos) -> bool {
    level.block_state(pos).is(&block_tags::SAND)
}

fn sound(name: &str) -> i32 {
    registry_id(registry::SOUND_EVENT, name)
}

pub struct TurtleEggBlock {
    block: Block,
}

impl TurtleEggBlock {
    pub fn new(block: Block) -> TurtleEggBlock {
        TurtleEggBlock { block }
    }

    fn destroy_egg(&self, level: &mut ServerLevel, state: BlockState, pos: BlockPos, entity: EntityId, randomness: i32) {
        if state.is_block(blocks::TURTLE_EGG) && self.can_destroy_egg(level, pos, entity) && level.random.next_int_bounded(randomness) == 0 {
            self.decrease_eggs(level, pos, state);
        }
    }

    fn decrease_eggs(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState) {
        let pitch = 0.9 + level.random.next_float() * 0.2;
        level.play_sound(None, pos, sound("minecraft:entity.turtle.egg_break"), SoundSource::Blocks, 0.7, pitch);
        let number_of_eggs = state.get(EGGS);
        if number_of_eggs <= 1 {
            level.destroy_block(pos, false, None, UPDATE_LIMIT);
        } else {
            level.set_block(pos, state.set(EGGS, number_of_eggs - 1), UPDATE_CLIENTS);
            level.level_event(None, PARTICLES_AND_SOUND_DESTROY_BLOCK, pos, state.id() as i32);
        }
    }

    fn should_update_hatch_level(&self, level: &mut ServerLevel, pos: BlockPos) -> bool {
        let chance = level.environment_attributes().get_value_at(TURTLE_EGG_HATCH_CHANCE, pos);
        chance > 0.0 && level.random.next_float() < chance
    }

    fn can_destroy_egg(&self, level: &ServerLevel, pos: BlockPos, entity: EntityId) -> bool {
        let entity_type = level.entity_ref(entity).map_or("", |e| registry::ENTITY_TYPE[e.entity_type as usize]);
        if entity_type == "minecraft:turtle" || entity_type == "minecraft:bat" {
            false
        } else if level.is_player(entity) {
            !level.is_under_spawn_protection(pos, entity)
        } else {
            level.is_living(entity) && level.game_rules.get_bool(game_rule::MOB_GRIEFING)
        }
    }
}

impl BlockBehaviour for TurtleEggBlock {
    fn step_on(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState, entity: EntityId) {
        if !level.entity(entity).is_shift_key_down() {
            self.destroy_egg(level, state, pos, entity, 100);
        }
    }

    fn fall_on(&self, level: &mut ServerLevel, state: BlockState, pos: BlockPos, entity: EntityId, fall_distance: f64) {
        let is_zombie = level.entity_ref(entity).is_some_and(|e| {
            matches!(
                registry::ENTITY_TYPE[e.entity_type as usize],
                "minecraft:zombie" | "minecraft:husk" | "minecraft:drowned" | "minecraft:zombie_villager" | "minecraft:zombified_piglin"
            )
        });
        if !is_zombie {
            self.destroy_egg(level, state, pos, entity, 3);
        }
        block_fall_on(level, state, pos, entity, fall_distance);
    }

    fn random_tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        if self.should_update_hatch_level(level, pos) && on_sand(level, pos) {
            let hatch = state.get(HATCH);
            if hatch < MAX_HATCH_LEVEL {
                let pitch = 0.9 + level.random.next_float() * 0.2;
                level.play_sound(None, pos, sound("minecraft:entity.turtle.egg_crack"), SoundSource::Blocks, 0.7, pitch);
                level.set_block(pos, state.set(HATCH, hatch + 1), UPDATE_CLIENTS);
            } else {
                let pitch = 0.9 + level.random.next_float() * 0.2;
                level.play_sound(None, pos, sound("minecraft:entity.turtle.egg_hatch"), SoundSource::Blocks, 0.7, pitch);
                level.remove_block(pos, false);
                for _ in 0..state.get(EGGS) {
                    level.level_event(None, PARTICLES_AND_SOUND_DESTROY_BLOCK, pos, state.id() as i32);
                }
            }
        }
    }

    fn on_place(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos, _old_state: BlockState, _moved_by_piston: bool) {
        if on_sand(level, pos) {
            level.level_event(None, PARTICLES_TURTLE_EGG_PLACEMENT, pos, 15);
        }
    }

    fn player_destroy(&self, level: &mut ServerLevel, player: EntityId, pos: BlockPos, state: BlockState, block_entity: Option<&BlockEntity>, tool: Option<ItemStack>) {
        level.drop_resources_with_entity(state, pos, block_entity, Some(player), tool);
        self.decrease_eggs(level, pos, state);
    }

    fn can_be_replaced(&self, state: BlockState, ctx: &BlockPlaceContext) -> bool {
        if !ctx.is_secondary_use_active() && ctx.item.item == self.block.item() && state.get(EGGS) < MAX_EGGS {
            return true;
        }
        state.can_be_replaced() && (ctx.item.is_empty() || ctx.item.item != state.block().item())
    }

    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        let state = level.block_state(ctx.clicked_pos());
        if state.is_block(block) { Some(state.set(EGGS, (state.get(EGGS) + 1).min(MAX_EGGS))) } else { Some(block.default_state()) }
    }
}
