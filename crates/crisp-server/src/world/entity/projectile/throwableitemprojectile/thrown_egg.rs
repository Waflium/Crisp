use crisp_core::random::RandomSource;

use super::throwable_item_projectile;
use crate::server::level::ServerLevel;
use crate::world::entity::animal::chicken::Chicken;
use crate::world::entity::projectile::projectile::{Projectile, ProjectileKind};
use crate::world::entity::{EntityDimensions, EntityId, LevelEntity};
use crate::world::item::ItemStack;
use crate::world::item::component as dc;
use crate::world::phys::hit_result::HitResult;

const ENTITY_TYPE: &str = "minecraft:egg";
const EVENT_BREAK: u8 = 3;
const ZERO_SIZED_DIMENSIONS: EntityDimensions = EntityDimensions { width: 0.0, height: 0.0, eye_height: 0.0 };

#[derive(Default)]
pub struct ThrownEgg;

impl ThrownEgg {
    pub fn of_type() -> Projectile {
        throwable_item_projectile::new(ENTITY_TYPE, ProjectileKind::ThrownEgg(ThrownEgg))
    }

    pub fn from_owner(level: &ServerLevel, mob: EntityId, item_stack: &ItemStack) -> Projectile {
        throwable_item_projectile::from_owner(level, ENTITY_TYPE, mob, item_stack, ProjectileKind::ThrownEgg(ThrownEgg))
    }

    pub fn at(x: f64, y: f64, z: f64, item_stack: &ItemStack) -> Projectile {
        throwable_item_projectile::at(ENTITY_TYPE, x, y, z, item_stack, ProjectileKind::ThrownEgg(ThrownEgg))
    }
}

impl ServerLevel {
    pub fn thrown_egg_on_hit(&mut self, id: EntityId, hit_result: HitResult) {
        self.projectile_base_on_hit(id, hit_result);
        let random = &mut self.entity_mut(id).random;
        if random.next_int_bounded(8) == 0 {
            let count = if random.next_int_bounded(32) == 0 { 4 } else { 1 };
            for _ in 0..count {
                let mut chicken = Chicken::create();
                chicken.set_age(-24000);
                chicken.refresh_dimensions();
                let e = self.entity(id);
                let (x, y, z, y_rot) = (e.x(), e.y(), e.z(), e.y_rot);
                chicken.entity.snap_to(x, y, z, y_rot, 0.0);
                if let Some(variant) = self.projectile(id).get_item().get(dc::CHICKEN_VARIANT).cloned() {
                    chicken.apply_implicit_component(dc::CHICKEN_VARIANT, &variant);
                }
                if !self.fudge_position_after_size_change_detached(&mut chicken.entity, ZERO_SIZED_DIMENSIONS) {
                    break;
                }
                self.add_fresh_entity(LevelEntity::Mob(Box::new(chicken)));
            }
        }
        self.broadcast_entity_event(id, EVENT_BREAK);
        self.entity_mut(id).discard();
    }
}
