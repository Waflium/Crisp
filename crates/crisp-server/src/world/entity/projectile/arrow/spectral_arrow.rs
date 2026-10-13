use crisp_data::{items, mob_effects};
use crisp_protocol::nbt::Nbt;

use super::abstract_arrow::{self, AbstractArrow};
use crate::server::level::ServerLevel;
use crate::world::effect::MobEffectInstance;
use crate::world::entity::EntityId;
use crate::world::entity::projectile::projectile::{Projectile, ProjectileKind};
use crate::world::item::ItemStack;
use crate::world::level::block::entity::int_value;

const ENTITY_TYPE: &str = "minecraft:spectral_arrow";
const DEFAULT_DURATION: i32 = 200;

pub struct SpectralArrow {
    pub abstract_arrow: AbstractArrow,
    pub duration: i32,
}

impl SpectralArrow {
    pub fn of_type() -> Projectile {
        let arrow = SpectralArrow { abstract_arrow: AbstractArrow::new(ItemStack::new(items::SPECTRAL_ARROW, 1)), duration: DEFAULT_DURATION };
        Projectile::new(ENTITY_TYPE, abstract_arrow::DIMENSIONS, abstract_arrow::defaults(), ProjectileKind::SpectralArrow(arrow))
    }

    pub fn at(level: &mut ServerLevel, x: f64, y: f64, z: f64, pickup_item_stack: &ItemStack, fired_from_weapon: Option<&ItemStack>) -> Projectile {
        abstract_arrow::at(level, SpectralArrow::of_type(), x, y, z, pickup_item_stack, fired_from_weapon)
    }

    pub fn from_owner(level: &mut ServerLevel, owner: EntityId, pickup_item_stack: &ItemStack, fired_from_weapon: Option<&ItemStack>) -> Projectile {
        abstract_arrow::from_owner(level, SpectralArrow::of_type(), owner, pickup_item_stack, fired_from_weapon)
    }
}

fn spectral_arrow(projectile: &Projectile) -> &SpectralArrow {
    match &projectile.kind {
        ProjectileKind::SpectralArrow(arrow) => arrow,
        _ => unreachable!("not a spectral arrow"),
    }
}

impl Projectile {
    pub fn spectral_arrow_add_additional_save_data(&self, output: &mut Vec<(String, Nbt)>) {
        output.push(("Duration".into(), Nbt::Int(spectral_arrow(self).duration)));
    }

    pub fn spectral_arrow_read_additional_save_data(&mut self, input: &Nbt) {
        if let ProjectileKind::SpectralArrow(arrow) = &mut self.kind {
            arrow.duration = input.get("Duration").and_then(int_value).unwrap_or(DEFAULT_DURATION);
        }
    }
}

impl ServerLevel {
    pub fn spectral_arrow_do_post_hurt_effects(&mut self, id: EntityId, mob: EntityId) {
        let duration = spectral_arrow(self.projectile(id)).duration;
        let effect_source = self.projectile_get_effect_source(id);
        self.add_effect(mob, MobEffectInstance::new(mob_effects::GLOWING, duration, 0), Some(effect_source));
    }
}
