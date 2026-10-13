use crisp_core::Vec3;
use crisp_data::Item;

use super::ItemStack;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::projectile::arrow::abstract_arrow::Pickup;
use crate::world::entity::projectile::arrow::arrow::Arrow;
use crate::world::entity::projectile::arrow::spectral_arrow::SpectralArrow;
use crate::world::entity::projectile::projectile::Projectile;

pub fn create_arrow(level: &mut ServerLevel, item: Item, item_stack: &ItemStack, owner: EntityId, fired_from_weapon: Option<&ItemStack>) -> Projectile {
    if item.is_instance_of("SpectralArrowItem") {
        SpectralArrow::from_owner(level, owner, &item_stack.copy_with_count(1), fired_from_weapon)
    } else {
        Arrow::from_owner(level, owner, &item_stack.copy_with_count(1), fired_from_weapon)
    }
}

pub fn as_projectile(level: &mut ServerLevel, item: Item, position: Vec3, item_stack: &ItemStack) -> Projectile {
    let mut arrow = if item.is_instance_of("SpectralArrowItem") {
        SpectralArrow::at(level, position.x, position.y, position.z, &item_stack.copy_with_count(1), None)
    } else {
        Arrow::at(level, position.x, position.y, position.z, &item_stack.copy_with_count(1), None)
    };
    arrow.abstract_arrow_mut().expect("an arrow").pickup = Pickup::Allowed;
    arrow
}
