use crisp_core::BlockPos;
use crisp_data::{BlockState, block_tags, blocks, damage_types};

use super::fire_block;
use crate::server::level::ServerLevel;
use crate::server::level::events::SOUND_EXTINGUISH_FIRE;
use crate::world::damagesource::DamageSource;
use crate::world::entity::EntityId;
use crate::world::entity::inside_block::{InsideBlockEffectType, InsideBlockEffects};
use crate::world::level::BlockGetter;

pub fn get_state(level: &mut ServerLevel, pos: BlockPos) -> BlockState {
    let below_state = level.block_state(pos.below());
    if can_survive_on_soul_block(below_state) { blocks::SOUL_FIRE.default_state() } else { fire_block::get_state_for_placement(level, pos) }
}

pub fn can_survive_on_soul_block(state: BlockState) -> bool {
    state.is(&block_tags::SOUL_FIRE_BASE_BLOCKS)
}

pub fn can_be_placed_at(level: &mut ServerLevel, pos: BlockPos) -> bool {
    if !level.block_state(pos).is_air() {
        return false;
    }
    let fire = get_state(level, pos);
    super::of(fire).can_survive(fire, level, pos)
}

pub fn entity_inside(effects: &mut InsideBlockEffects, fire_damage: f32) {
    effects.apply(InsideBlockEffectType::ClearFreeze);
    effects.apply(InsideBlockEffectType::FireIgnite);
    effects.run_after(InsideBlockEffectType::FireIgnite, Box::new(move |level, id: EntityId| level.hurt(id, DamageSource::new(damage_types::IN_FIRE), fire_damage)));
}

pub fn on_place(state: BlockState, level: &mut ServerLevel, pos: BlockPos, old_state: BlockState) {
    if !old_state.is_block(state.block()) && !super::of(state).can_survive(state, level, pos) {
        level.remove_block(pos, false);
    }
}

pub fn player_will_destroy(level: &mut ServerLevel, pos: BlockPos, state: BlockState) -> BlockState {
    level.level_event(None, SOUND_EXTINGUISH_FIRE, pos, 0);
    state
}
