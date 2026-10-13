use crisp_core::Vec3;
use crisp_core::random::RandomSource;

use super::throwable_item_projectile;
use crate::server::level::ServerLevel;
use crate::server::level::events::{PARTICLES_SPELL_POTION_SPLASH, SOUND_SPELL_POTION_SPLASH};
use crate::world::entity::EntityId;
use crate::world::entity::projectile::projectile::{Projectile, ProjectileKind};
use crate::world::item::ItemStack;
use crate::world::phys::hit_result::HitResult;

const ENTITY_TYPE: &str = "minecraft:experience_bottle";

#[derive(Default)]
pub struct ThrownExperienceBottle;

impl ThrownExperienceBottle {
    pub fn of_type() -> Projectile {
        throwable_item_projectile::new(ENTITY_TYPE, ProjectileKind::ThrownExperienceBottle(ThrownExperienceBottle))
    }

    pub fn from_owner(level: &ServerLevel, mob: EntityId, item_stack: &ItemStack) -> Projectile {
        throwable_item_projectile::from_owner(level, ENTITY_TYPE, mob, item_stack, ProjectileKind::ThrownExperienceBottle(ThrownExperienceBottle))
    }

    pub fn at(x: f64, y: f64, z: f64, item_stack: &ItemStack) -> Projectile {
        throwable_item_projectile::at(ENTITY_TYPE, x, y, z, item_stack, ProjectileKind::ThrownExperienceBottle(ThrownExperienceBottle))
    }
}

impl ServerLevel {
    pub fn thrown_experience_bottle_on_hit(&mut self, id: EntityId, hit_result: HitResult) {
        self.projectile_base_on_hit(id, hit_result);
        let block_position = self.entity(id).block_position();
        self.level_event(None, PARTICLES_SPELL_POTION_SPLASH, block_position, -13083194);
        if !self.entity(id).is_silent() {
            self.level_event(None, SOUND_SPELL_POTION_SPLASH, block_position, 0);
        }
        let random = &mut self.entity_mut(id).random;
        let xp_count = 3 + random.next_int_bounded(5) + random.next_int_bounded(5);
        match hit_result {
            HitResult::Block { hit, .. } => {
                let direction = hit.direction;
                let block_normal_hit = Vec3::new(direction.step_x() as f64, direction.step_y() as f64, direction.step_z() as f64);
                self.award_experience_with_direction(hit_result.get_location(), block_normal_hit, xp_count);
            }
            HitResult::Entity(_) => {
                let reverse = self.entity(id).delta_movement.scale(-1.0);
                self.award_experience_with_direction(hit_result.get_location(), reverse, xp_count);
            }
        }
        self.entity_mut(id).discard();
    }
}
