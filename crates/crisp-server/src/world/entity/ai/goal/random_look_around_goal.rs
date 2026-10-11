use crisp_core::random::RandomSource;

use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;

#[derive(Default)]
pub struct RandomLookAroundGoal {
    rel_x: f64,
    rel_z: f64,
    look_time: i32,
}

impl RandomLookAroundGoal {
    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        level.entity_mut(mob).random.next_float() < 0.02
    }

    pub fn can_continue_to_use(&self) -> bool {
        self.look_time >= 0
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        let random = &mut level.entity_mut(mob).random;
        let rnd = std::f64::consts::TAU * random.next_double();
        self.rel_x = rnd.cos();
        self.rel_z = rnd.sin();
        self.look_time = 20 + random.next_int_bounded(20);
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        self.look_time -= 1;
        let e = level.entity(mob);
        let (x, y, z) = (e.x() + self.rel_x, e.eye_y(), e.z() + self.rel_z);
        level.set_look_at(mob, x, y, z);
    }
}
