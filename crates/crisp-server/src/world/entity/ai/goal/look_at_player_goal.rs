use crisp_core::random::RandomSource;

use super::goal::reduced_tick_delay;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::targeting::targeting_conditions::TargetingConditions;

pub struct LookAtPlayerGoal {
    look_at: Option<EntityId>,
    look_distance: f32,
    look_time: i32,
    probability: f32,
    only_horizontal: bool,
}

impl LookAtPlayerGoal {
    pub fn new(look_distance: f32) -> LookAtPlayerGoal {
        LookAtPlayerGoal { look_at: None, look_distance, look_time: 0, probability: 0.02, only_horizontal: false }
    }

    fn look_at_context(&self) -> TargetingConditions<'static> {
        TargetingConditions::for_non_combat().range(self.look_distance as f64)
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        if level.entity_mut(mob).random.next_float() >= self.probability {
            return false;
        }
        if let Some(target) = level.mob(mob).expect("a mob").target {
            self.look_at = Some(target);
        }
        let e = level.entity(mob);
        let (x, y, z) = (e.x(), e.eye_y(), e.z());
        self.look_at = level.get_nearest_player(&self.look_at_context(), Some(mob), x, y, z);
        self.look_at.is_some()
    }

    pub fn can_continue_to_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        let look_at = self.look_at.expect("a look target");
        if !level.is_alive(look_at) {
            return false;
        }
        let distance = level.entity(mob).distance_to_sqr(level.entity(look_at));
        if distance > (self.look_distance * self.look_distance) as f64 { false } else { self.look_time > 0 }
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        self.look_time = reduced_tick_delay(40 + level.entity_mut(mob).random.next_int_bounded(40));
    }

    pub fn stop(&mut self) {
        self.look_at = None;
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        let look_at = self.look_at.expect("a look target");
        if level.is_alive(look_at) {
            let target_y = if self.only_horizontal { level.entity(mob).eye_y() } else { level.entity(look_at).eye_y() };
            let t = level.entity(look_at);
            let (x, z) = (t.x(), t.z());
            level.set_look_at(mob, x, target_y, z);
            self.look_time -= 1;
        }
    }
}
