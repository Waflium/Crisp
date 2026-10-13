use crisp_core::Vec3;
use crisp_data::{Item, items};
use crisp_protocol::nbt::Nbt;

use crate::server::level::ServerLevel;
use crate::world::entity::data::{DataValue, entity_defaults};
use crate::world::entity::entity_reference::EntityReference;
use crate::world::entity::projectile::projectile::{Projectile, ProjectileKind};
use crate::world::entity::{EntityDimensions, EntityId};
use crate::world::item::ItemStack;

pub const DATA_ITEM_STACK: usize = 8;
pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.25, 0.25);

pub fn get_default_item(kind: &ProjectileKind) -> Item {
    match kind {
        ProjectileKind::Snowball(_) => items::SNOWBALL,
        ProjectileKind::ThrownEgg(_) => items::EGG,
        ProjectileKind::ThrownEnderpearl(_) => items::ENDER_PEARL,
        ProjectileKind::ThrownExperienceBottle(_) => items::EXPERIENCE_BOTTLE,
        ProjectileKind::ThrownSplashPotion(_) => items::SPLASH_POTION,
        ProjectileKind::ThrownLingeringPotion(_) => items::LINGERING_POTION,
        _ => unreachable!("not a throwable item projectile"),
    }
}

pub fn new(entity_type: &str, kind: ProjectileKind) -> Projectile {
    let mut data = entity_defaults();
    data.push(DataValue::ItemStack(Some(ItemStack::new(get_default_item(&kind), 1))));
    Projectile::new(entity_type, DIMENSIONS, data, kind)
}

pub fn at(entity_type: &str, x: f64, y: f64, z: f64, item_stack: &ItemStack, kind: ProjectileKind) -> Projectile {
    let mut projectile = new(entity_type, kind);
    projectile.entity.set_pos(Vec3::new(x, y, z));
    projectile.set_item(item_stack);
    projectile
}

pub fn from_owner(level: &ServerLevel, entity_type: &str, owner: EntityId, item_stack: &ItemStack, kind: ProjectileKind) -> Projectile {
    let o = level.entity(owner);
    let mut projectile = at(entity_type, o.x(), o.eye_y() - 0.1f32 as f64, o.z(), item_stack, kind);
    projectile.owner = Some(EntityReference::of(o.uuid));
    projectile
}

impl Projectile {
    pub fn set_item(&mut self, source: &ItemStack) {
        self.entity.data.set(DATA_ITEM_STACK, DataValue::ItemStack(Some(source.copy_with_count(1))));
    }

    pub fn get_item(&self) -> ItemStack {
        match self.entity.data.get(DATA_ITEM_STACK) {
            DataValue::ItemStack(Some(item)) => item.clone(),
            _ => ItemStack::empty(),
        }
    }

    pub fn throwable_item_projectile_add_additional_save_data(&self, output: &mut Vec<(String, Nbt)>) {
        output.push(("Item".into(), self.get_item().to_nbt()));
    }

    pub fn throwable_item_projectile_read_additional_save_data(&mut self, input: &Nbt) {
        let item = input.get("Item").and_then(|item| ItemStack::from_nbt(item).ok()).unwrap_or_else(|| ItemStack::new(get_default_item(&self.kind), 1));
        self.set_item(&item);
    }
}
