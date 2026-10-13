use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::projectile::projectile::{Projectile, ProjectileKind};
use crate::world::item::ItemStack;
use crate::world::phys::entity_hit_result::EntityHitResult;
use crate::world::phys::hit_result::HitResult;

use super::throwable_item_projectile;

const ENTITY_TYPE: &str = "minecraft:snowball";
const EVENT_BREAK: u8 = 3;

#[derive(Default)]
pub struct Snowball;

impl Snowball {
    pub fn of_type() -> Projectile {
        throwable_item_projectile::new(ENTITY_TYPE, ProjectileKind::Snowball(Snowball))
    }

    pub fn from_owner(level: &ServerLevel, mob: EntityId, item_stack: &ItemStack) -> Projectile {
        throwable_item_projectile::from_owner(level, ENTITY_TYPE, mob, item_stack, ProjectileKind::Snowball(Snowball))
    }

    pub fn at(x: f64, y: f64, z: f64, item_stack: &ItemStack) -> Projectile {
        throwable_item_projectile::at(ENTITY_TYPE, x, y, z, item_stack, ProjectileKind::Snowball(Snowball))
    }
}

impl ServerLevel {
    pub fn snowball_on_hit_entity(&mut self, id: EntityId, hit_result: EntityHitResult) {
        let entity = hit_result.entity;
        let is_blaze = crisp_data::registry::ENTITY_TYPE[self.entity(entity).entity_type as usize] == "minecraft:blaze";
        let damage = if is_blaze { 3 } else { 0 };
        let source = self.thrown(id);
        self.hurt(entity, source, damage as f32);
    }

    pub fn snowball_on_hit(&mut self, id: EntityId, hit_result: HitResult) {
        self.projectile_base_on_hit(id, hit_result);
        self.broadcast_entity_event(id, EVENT_BREAK);
        self.entity_mut(id).discard();
    }
}
