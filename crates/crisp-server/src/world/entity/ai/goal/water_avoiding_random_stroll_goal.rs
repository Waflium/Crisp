use crisp_core::Vec3;
use crisp_core::random::RandomSource;

use super::random_stroll_goal::RandomStrollGoal;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::util::land_random_pos;

pub const PROBABILITY: f32 = 0.001;

pub struct WaterAvoidingRandomStrollGoal {
    pub random_stroll_goal: RandomStrollGoal,
    probability: f32,
}

impl WaterAvoidingRandomStrollGoal {
    pub fn new(speed_modifier: f64) -> WaterAvoidingRandomStrollGoal {
        WaterAvoidingRandomStrollGoal { random_stroll_goal: RandomStrollGoal::new(speed_modifier), probability: PROBABILITY }
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        let probability = self.probability;
        self.random_stroll_goal.can_use(level, mob, |level, mob| get_position(level, mob, probability))
    }
}

fn get_position(level: &mut ServerLevel, mob: EntityId, probability: f32) -> Option<Vec3> {
    if level.entity(mob).is_in_water() {
        land_random_pos::get_pos(level, mob, 15, 7).or_else(|| RandomStrollGoal::get_position(level, mob))
    } else if level.entity_mut(mob).random.next_float() >= probability {
        land_random_pos::get_pos(level, mob, 10, 7)
    } else {
        RandomStrollGoal::get_position(level, mob)
    }
}
