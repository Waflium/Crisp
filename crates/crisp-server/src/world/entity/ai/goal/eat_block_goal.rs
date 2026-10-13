use crisp_core::random::RandomSource;
use crisp_data::{BlockState, block_tags, blocks, game_rule};

use super::goal::reduced_tick_delay;
use crate::server::level::{ServerLevel, events};
use crate::world::entity::EntityId;
use crate::world::level::BlockGetter;
use crate::world::level::update_flags::UPDATE_CLIENTS;

const EAT_ANIMATION_TICKS: i32 = 40;

fn is_edible(state: BlockState) -> bool {
    state.is(&block_tags::EDIBLE_FOR_SHEEP)
}

#[derive(Default)]
pub struct EatBlockGoal {
    eat_animation_tick: i32,
}

impl EatBlockGoal {
    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        let bound = reduced_tick_delay(if level.is_baby(mob) { 50 } else { 1000 });
        if level.entity_mut(mob).random.next_int_bounded(bound) != 0 {
            return false;
        }
        let pos = level.entity(mob).block_position();
        if is_edible(level.block_state(pos)) { true } else { level.block_state(pos.below()).is_block(blocks::GRASS_BLOCK) }
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        self.eat_animation_tick = reduced_tick_delay(EAT_ANIMATION_TICKS);
        level.broadcast_entity_event(mob, crate::world::entity::entity_event::EAT_GRASS);
        level.mob_mut(mob).expect("a mob").ai.navigation.stop();
    }

    pub fn stop(&mut self) {
        self.eat_animation_tick = 0;
    }

    pub fn can_continue_to_use(&self) -> bool {
        self.eat_animation_tick > 0
    }

    pub fn get_eat_animation_tick(&self) -> i32 {
        self.eat_animation_tick
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        self.eat_animation_tick = (self.eat_animation_tick - 1).max(0);
        if self.eat_animation_tick == reduced_tick_delay(4) {
            let pos = level.entity(mob).block_position();
            if is_edible(level.block_state(pos)) {
                if level.game_rules.get_bool(game_rule::MOB_GRIEFING) {
                    level.destroy_block(pos, false, None, 512);
                }
                level.ate(mob);
            } else {
                let below = pos.below();
                if level.block_state(below).is_block(blocks::GRASS_BLOCK) {
                    if level.game_rules.get_bool(game_rule::MOB_GRIEFING) {
                        level.level_event(None, events::PARTICLES_AND_SOUND_DESTROY_BLOCK, below, blocks::GRASS_BLOCK.default_state().id() as i32);
                        level.set_block(below, blocks::DIRT.default_state(), UPDATE_CLIENTS);
                    }
                    level.ate(mob);
                }
            }
        }
    }
}
