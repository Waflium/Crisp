use crisp_core::random::RandomSource;

use super::EntityId;
use super::ageable_mob::AgeableMob;
use crate::server::level::ServerLevel;
use crate::world::damagesource::DamageSource;

pub mod chicken;

#[derive(Default)]
pub struct Animal {
    pub ageable: AgeableMob,
    pub in_love: i32,
    pub love_cause: Option<u128>,
}

impl ServerLevel {
    pub fn animal_custom_server_ai_step(&mut self, id: EntityId) {
        let mob = self.mob_mut(id).expect("an animal");
        let age = mob.ageable().expect("an animal").age;
        if age != 0 {
            mob.animal_mut().expect("an animal").in_love = 0;
        }
    }

    pub fn animal_actually_hurt(&mut self, id: EntityId, source: &DamageSource, damage: f32) {
        self.mob_mut(id).expect("an animal").animal_mut().expect("an animal").in_love = 0;
        self.living_entity_actually_hurt(id, source, damage);
    }

    pub fn animal_get_base_experience_reward(&mut self, id: EntityId) -> i32 {
        1 + self.entity_mut(id).random.next_int_bounded(3)
    }

    pub fn animal_ai_step(&mut self, id: EntityId) {
        self.ageable_mob_ai_step(id);
        let mob = self.mob_mut(id).expect("an animal");
        let age = mob.ageable().expect("an animal").age;
        let animal = mob.animal_mut().expect("an animal");
        if age != 0 {
            animal.in_love = 0;
        }
        if animal.in_love > 0 {
            animal.in_love -= 1;
            if animal.in_love % 10 == 0 {
                let e = &mut mob.entity;
                e.random.next_gaussian();
                e.random.next_gaussian();
                e.random.next_gaussian();
                e.random_x(1.0);
                e.random_y();
                e.random_z(1.0);
            }
        }
    }
}
