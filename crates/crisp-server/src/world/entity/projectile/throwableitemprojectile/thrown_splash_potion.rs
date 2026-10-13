use crisp_protocol::nbt::Nbt;

use super::abstract_thrown_potion::SPLASH_RANGE_SQ;
use super::throwable_item_projectile;
use crate::server::level::ServerLevel;
use crate::world::effect::MobEffectInstance;
use crate::world::entity::EntityId;
use crate::world::entity::projectile::projectile::{Projectile, ProjectileKind};
use crate::world::entity::projectile::projectile_util::compute_margin;
use crate::world::item::ItemStack;
use crate::world::item::alchemy::potion_contents::PotionContents;
use crate::world::item::component as dc;
use crate::world::phys::hit_result::HitResult;

const ENTITY_TYPE: &str = "minecraft:splash_potion";

#[derive(Default)]
pub struct ThrownSplashPotion;

impl ThrownSplashPotion {
    pub fn of_type() -> Projectile {
        throwable_item_projectile::new(ENTITY_TYPE, ProjectileKind::ThrownSplashPotion(ThrownSplashPotion))
    }

    pub fn from_owner(level: &ServerLevel, owner: EntityId, item_stack: &ItemStack) -> Projectile {
        throwable_item_projectile::from_owner(level, ENTITY_TYPE, owner, item_stack, ProjectileKind::ThrownSplashPotion(ThrownSplashPotion))
    }

    pub fn at(x: f64, y: f64, z: f64, item_stack: &ItemStack) -> Projectile {
        throwable_item_projectile::at(ENTITY_TYPE, x, y, z, item_stack, ProjectileKind::ThrownSplashPotion(ThrownSplashPotion))
    }
}

impl ServerLevel {
    pub fn thrown_splash_potion_on_hit_as_potion(&mut self, id: EntityId, potion_item: &ItemStack, hit_result: HitResult) {
        let empty = Nbt::Compound(Vec::new());
        let contents = PotionContents::new(potion_item.get(dc::POTION_CONTENTS).unwrap_or(&empty));
        let duration_scale = potion_item.get(dc::POTION_DURATION_SCALE).and_then(Nbt::as_f64).map_or(1.0, |scale| scale as f32);
        let mob_effects = contents.get_all_effects();
        let e = self.entity(id);
        let potion_aabb = e.bounding_box().move_vec(hit_result.get_location().subtract(e.position()));
        let effect_aabb = potion_aabb.inflate(4.0, 2.0, 4.0);
        let entities: Vec<EntityId> = self.get_entities(None, &effect_aabb).into_iter().filter(|&entity| self.is_living(entity)).collect();
        let margin = compute_margin(self.entity(id).tick_count);
        if entities.is_empty() {
            return;
        }
        let effect_source = self.projectile_get_effect_source(id);
        for entity in entities {
            if !self.is_affected_by_potions(entity) {
                continue;
            }
            let dist = potion_aabb.distance_to_sqr_aabb(&self.entity(entity).bounding_box().inflate_all(margin as f64));
            if dist >= SPLASH_RANGE_SQ {
                continue;
            }
            let scale = 1.0 - dist.sqrt() / 4.0;
            for effect_instance in &mob_effects {
                let effect = effect_instance.effect;
                if effect.info().instantaneous {
                    let owner = self.projectile_get_owner(id);
                    self.apply_instantaneous_effect(effect, Some(id), owner, entity, effect_instance.amplifier, scale);
                } else {
                    let duration = effect_instance.map_duration(|d| (scale * d as f64 * duration_scale as f64 + 0.5) as i32);
                    let new_effect = MobEffectInstance::with(effect, duration, effect_instance.amplifier, effect_instance.ambient, effect_instance.visible, effect_instance.visible);
                    if !new_effect.ends_within(20) {
                        self.add_effect(entity, new_effect, Some(effect_source));
                    }
                }
            }
        }
    }
}
