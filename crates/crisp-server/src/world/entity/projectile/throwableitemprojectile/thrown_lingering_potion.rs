use super::throwable_item_projectile;
use crate::server::level::ServerLevel;
use crate::world::entity::area_effect_cloud::AreaEffectCloud;
use crate::world::entity::projectile::projectile::{Projectile, ProjectileKind};
use crate::world::entity::{EntityId, LevelEntity};
use crate::world::item::ItemStack;
use crate::world::phys::hit_result::HitResult;

const ENTITY_TYPE: &str = "minecraft:lingering_potion";

#[derive(Default)]
pub struct ThrownLingeringPotion;

impl ThrownLingeringPotion {
    pub fn of_type() -> Projectile {
        throwable_item_projectile::new(ENTITY_TYPE, ProjectileKind::ThrownLingeringPotion(ThrownLingeringPotion))
    }

    pub fn from_owner(level: &ServerLevel, owner: EntityId, item_stack: &ItemStack) -> Projectile {
        throwable_item_projectile::from_owner(level, ENTITY_TYPE, owner, item_stack, ProjectileKind::ThrownLingeringPotion(ThrownLingeringPotion))
    }

    pub fn at(x: f64, y: f64, z: f64, item_stack: &ItemStack) -> Projectile {
        throwable_item_projectile::at(ENTITY_TYPE, x, y, z, item_stack, ProjectileKind::ThrownLingeringPotion(ThrownLingeringPotion))
    }
}

impl ServerLevel {
    pub fn thrown_lingering_potion_on_hit_as_potion(&mut self, id: EntityId, potion_item: &ItemStack, hit_result: HitResult) {
        let position = match hit_result {
            HitResult::Entity(entity_hit_result) => self.entity(entity_hit_result.entity).position(),
            HitResult::Block { .. } => self.entity(id).position(),
        };
        let mut cloud = AreaEffectCloud::new(position.x, position.y, position.z);
        if let Some(owner) = self.projectile_get_owner(id).filter(|&owner| self.is_living(owner)) {
            cloud.set_owner(Some(self.entity(owner).uuid));
        }
        cloud.set_radius(3.0);
        cloud.radius_on_use = -0.5;
        cloud.duration = 600;
        cloud.wait_time = 10;
        cloud.radius_per_tick = -cloud.get_radius() / cloud.duration as f32;
        cloud.apply_components_from_item_stack(potion_item);
        self.add_fresh_entity(LevelEntity::AreaEffectCloud(Box::new(cloud)));
    }
}
