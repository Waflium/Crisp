use crisp_core::{Axis, BlockPos, Direction};
use crisp_data::{BlockState, block_tags, blocks, damage_types};

use super::fire_block;
use crate::server::level::ServerLevel;
use crate::server::level::events::SOUND_EXTINGUISH_FIRE;
use crate::world::damagesource::DamageSource;
use crate::world::entity::EntityId;
use crate::world::entity::inside_block::{InsideBlockEffectType, InsideBlockEffects};
use crate::world::level::portal::portal_shape::{self, PortalShape};
use crate::server::{OVERWORLD, THE_NETHER};
use crisp_core::random::RandomSource;
use crate::world::level::{BlockGetter, LevelReader};

pub fn get_state(level: &mut dyn LevelReader, pos: BlockPos) -> BlockState {
    let below_state = level.block_state(pos.below());
    if can_survive_on_soul_block(below_state) { blocks::SOUL_FIRE.default_state() } else { fire_block::get_state_for_placement(level, pos) }
}

pub fn can_survive_on_soul_block(state: BlockState) -> bool {
    state.is(&block_tags::SOUL_FIRE_BASE_BLOCKS)
}

pub fn can_be_placed_at(level: &mut ServerLevel, pos: BlockPos, forward_direction: Direction) -> bool {
    if !level.block_state(pos).is_air() {
        return false;
    }
    let fire = get_state(level, pos);
    super::of(fire).can_survive(fire, level, pos) || is_portal(level, pos, forward_direction)
}

fn in_portal_dimension(level: &ServerLevel) -> bool {
    level.dimension == OVERWORLD || level.dimension == THE_NETHER
}

fn is_portal(level: &mut ServerLevel, pos: BlockPos, forward_direction: Direction) -> bool {
    if !in_portal_dimension(level) {
        return false;
    }
    for face in Direction::VALUES {
        let state = level.block_state(pos.relative(face));
        if portal_shape::frame(state) {
            let preferred_axis = if forward_direction.axis().is_horizontal() {
                forward_direction.counter_clockwise().axis()
            } else {
                [Axis::X, Axis::Z][level.random.next_int_bounded(2) as usize]
            };
            return PortalShape::find_empty_portal_shape(level, pos, preferred_axis).is_some();
        }
    }
    false
}

pub fn entity_inside(effects: &mut InsideBlockEffects, fire_damage: f32) {
    effects.apply(InsideBlockEffectType::ClearFreeze);
    effects.apply(InsideBlockEffectType::FireIgnite);
    effects.run_after(InsideBlockEffectType::FireIgnite, Box::new(move |level, id: EntityId| level.hurt(id, DamageSource::new(damage_types::IN_FIRE), fire_damage)));
}

pub fn on_place(state: BlockState, level: &mut ServerLevel, pos: BlockPos, old_state: BlockState) {
    if old_state.is_block(state.block()) {
        return;
    }
    if in_portal_dimension(level)
        && let Some(shape) = PortalShape::find_empty_portal_shape(level, pos, Axis::X)
    {
        shape.create_portal_blocks(level);
        return;
    }
    if !super::of(state).can_survive(state, level, pos) {
        level.remove_block(pos, false);
    }
}

pub fn player_will_destroy(level: &mut ServerLevel, pos: BlockPos, state: BlockState) -> BlockState {
    level.level_event(None, SOUND_EXTINGUISH_FIRE, pos, 0);
    state
}
