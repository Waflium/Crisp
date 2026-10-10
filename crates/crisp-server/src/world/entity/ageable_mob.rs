use super::EntityId;
use crate::server::level::ServerLevel;

#[derive(Default)]
pub struct AgeableMob {
    pub age: i32,
    pub forced_age: i32,
    pub forced_age_timer: i32,
    pub age_lock_particle_timer: i32,
}

impl ServerLevel {
    pub fn set_age(&mut self, id: EntityId, age: i32) {
        let Some(mob) = self.mob_mut(id) else { return };
        if mob.set_age(age) {
            self.refresh_mob_dimensions(id);
        }
    }

    pub fn ageable_mob_ai_step(&mut self, id: EntityId) {
        self.mob_ai_step(id);
        let mob = self.mob_mut(id).expect("an ageable mob");
        let ageable = mob.ageable_mut().expect("an ageable mob");
        if ageable.forced_age_timer > 0 {
            if ageable.forced_age_timer % 4 == 0 {
                let e = &mut mob.entity;
                let x = e.random_x(1.0);
                let y = e.random_y() + 0.5;
                let z = e.random_z(1.0);
                self.send_particles("minecraft:happy_villager", |_| {}, x, y, z, 1, (0.0, 0.0, 0.0), 0.0);
            }
            self.mob_mut(id).expect("an ageable mob").ageable_mut().expect("an ageable mob").forced_age_timer -= 1;
        }
        if self.is_alive(id) {
            let mob = self.mob(id).expect("an ageable mob");
            let age = mob.ageable().expect("an ageable mob").age;
            if mob.is_baby() && !mob.is_age_locked() {
                self.set_age(id, age + 1);
            } else if age > 0 {
                self.set_age(id, age - 1);
            }
        }
        let mob = self.mob_mut(id).expect("an ageable mob");
        let locked = mob.is_age_locked();
        let timer = mob.ageable().expect("an ageable mob").age_lock_particle_timer;
        if timer > 0 {
            if timer % 2 == 0 {
                let y_offset = if locked { 0.2 } else { 0.0 };
                let height = mob.entity.dimensions.height as f64;
                let e = &mut mob.entity;
                let x = e.random_x(1.0);
                let y = e.random_y_scaled(0.2) + height + y_offset;
                let z = e.random_z(1.0);
                let particle = if locked { "minecraft:pause_mob_growth" } else { "minecraft:reset_mob_growth" };
                self.send_particles(particle, |_| {}, x, y, z, 1, (0.0, 0.0, 0.0), 0.0);
            }
            self.mob_mut(id).expect("an ageable mob").ageable_mut().expect("an ageable mob").age_lock_particle_timer -= 1;
        }
    }

    pub fn refresh_mob_dimensions(&mut self, id: EntityId) {
        let Some(mob) = self.mob_mut(id) else { return };
        let old = mob.refresh_dimensions();
        let new = mob.entity.dimensions;
        let e = &mob.entity;
        let small = new.width <= 4.0 && new.height <= 4.0;
        if !e.first_tick && !e.no_physics && small && (new.width > old.width || new.height > old.height) {
            self.fudge_position_after_size_change(id, old);
        }
    }

    fn fudge_position_after_size_change(&mut self, id: EntityId, previous: super::EntityDimensions) -> bool {
        let e = self.entity(id);
        let new = e.dimensions;
        let old_center = e.position().add(0.0, previous.height as f64 / 2.0, 0.0);
        let width_delta = (new.width - previous.width).max(0.0) as f64 + 1.0e-6;
        let height_delta = (new.height - previous.height).max(0.0) as f64 + 1.0e-6;
        let allowed = crisp_core::AABB::of_size(old_center, width_delta, height_delta, width_delta);
        if let Some(free) = self.find_free_position(&allowed, old_center, new.width as f64, new.height as f64, new.width as f64) {
            self.entity_mut(id).set_pos(free.add(0.0, -new.height as f64 / 2.0, 0.0));
            return true;
        }
        if new.width > previous.width && new.height > previous.height {
            let allowed = crisp_core::AABB::of_size(old_center, width_delta, 1.0e-6, width_delta);
            if let Some(free) = self.find_free_position(&allowed, old_center, new.width as f64, previous.height as f64, new.width as f64) {
                self.entity_mut(id).set_pos(free.add(0.0, -previous.height as f64 / 2.0 + 1.0e-6, 0.0));
                return true;
            }
        }
        false
    }
}
