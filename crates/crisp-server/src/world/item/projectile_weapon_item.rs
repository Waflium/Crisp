use crisp_data::{EquipmentSlot, Item, item_tags, items};
use crisp_protocol::nbt::Nbt;

use super::ItemStack;
use super::component as dc;
use super::enchantment::enchantment_helper;
use crate::server::level::ServerLevel;
use crate::world::InteractionHand;
use crate::world::entity::EntityId;
use crate::world::entity::projectile::projectile::Projectile;
use crate::world::item::{arrow_item, crossbow_item};

pub fn arrow_only(item_stack: &ItemStack) -> bool {
    item_tags::ARROWS.contains(item_stack.get_item().0 as usize)
}

pub fn arrow_or_firework(item_stack: &ItemStack) -> bool {
    arrow_only(item_stack) || item_stack.is(items::FIREWORK_ROCKET)
}

pub fn is_projectile_weapon(item: Item) -> bool {
    item.is_instance_of("ProjectileWeaponItem")
}

pub fn get_supported_held_projectiles(item: Item) -> fn(&ItemStack) -> bool {
    if item.is_instance_of("CrossbowItem") { arrow_or_firework } else { arrow_only }
}

pub fn get_all_supported_projectiles(_item: Item) -> fn(&ItemStack) -> bool {
    arrow_only
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ProjectileSlot {
    Inventory(usize),
    Created,
}

pub struct HeldProjectile {
    pub stack: ItemStack,
    pub slot: ProjectileSlot,
}

impl HeldProjectile {
    pub fn empty() -> HeldProjectile {
        HeldProjectile { stack: ItemStack::empty(), slot: ProjectileSlot::Created }
    }

    pub fn is_empty(&self) -> bool {
        self.stack.is_empty()
    }
}

impl ServerLevel {
    pub fn get_held_projectile(&self, entity: EntityId, valid: fn(&ItemStack) -> bool) -> HeldProjectile {
        let player = self.player(entity);
        for hand in [InteractionHand::OffHand, InteractionHand::MainHand] {
            if let Some(stack) = player.item_in_hand(hand).filter(|stack| valid(stack)) {
                return HeldProjectile { stack, slot: ProjectileSlot::Inventory(player.hand_slot(hand)) };
            }
        }
        HeldProjectile::empty()
    }

    pub fn get_projectile(&self, player: EntityId, held_weapon: &ItemStack) -> HeldProjectile {
        let item = held_weapon.get_item();
        if !is_projectile_weapon(item) {
            return HeldProjectile::empty();
        }
        let held_projectile = self.get_held_projectile(player, get_supported_held_projectiles(item));
        if !held_projectile.is_empty() {
            return held_projectile;
        }
        let supported_projectiles = get_all_supported_projectiles(item);
        let inventory = &self.player(player).inventory;
        for index in 0..41 {
            let slot = super::inventory_slot(index);
            if let Some(stack) = inventory.slots[slot].as_ref().filter(|stack| supported_projectiles(stack)) {
                return HeldProjectile { stack: stack.clone(), slot: ProjectileSlot::Inventory(slot) };
            }
        }
        if self.has_infinite_materials(player) { HeldProjectile { stack: ItemStack::new(items::ARROW, 1), slot: ProjectileSlot::Created } } else { HeldProjectile::empty() }
    }

    pub fn draw(&mut self, weapon: &ItemStack, mut projectile: HeldProjectile, shooter: EntityId) -> Vec<ItemStack> {
        if projectile.is_empty() {
            return Vec::new();
        }
        let num_projectiles = enchantment_helper::process_projectile_count(self, weapon, shooter, 1);
        let mut drawn = Vec::with_capacity(num_projectiles as usize);
        let mut projectile_copy = HeldProjectile { stack: projectile.stack.copy(), slot: ProjectileSlot::Created };
        for i in 0..num_projectiles {
            let drawn_stack = if i == 0 { self.use_ammo(weapon, &mut projectile, shooter, false) } else { self.use_ammo(weapon, &mut projectile_copy, shooter, true) };
            if !drawn_stack.is_empty() {
                drawn.push(drawn_stack);
            }
        }
        drawn
    }

    fn use_ammo(&mut self, weapon: &ItemStack, projectile: &mut HeldProjectile, holder: EntityId, force_infinite: bool) -> ItemStack {
        let ammo_to_use = if !force_infinite && !self.has_infinite_materials(holder) { enchantment_helper::process_ammo_use(self, weapon, &projectile.stack, 1) } else { 0 };
        if ammo_to_use > projectile.stack.count {
            return ItemStack::empty();
        }
        if ammo_to_use == 0 {
            let mut copy = projectile.stack.copy_with_count(1);
            copy.set(dc::INTANGIBLE_PROJECTILE, Some(Nbt::Compound(Vec::new())));
            return copy;
        }
        let used = projectile.stack.split(ammo_to_use);
        if let ProjectileSlot::Inventory(slot) = projectile.slot {
            let stack = projectile.stack.clone();
            self.player_mut(holder).inventory.slots[slot] = (!stack.is_empty()).then_some(stack);
        }
        used
    }

    #[allow(clippy::too_many_arguments)]
    pub fn projectile_weapon_shoot(
        &mut self,
        shooter: EntityId,
        hand: InteractionHand,
        projectiles: &[ItemStack],
        power: f32,
        uncertainty: f32,
        is_crit: bool,
        target_override: Option<EntityId>,
    ) {
        let slot = if hand == InteractionHand::MainHand { EquipmentSlot::Mainhand } else { EquipmentSlot::Offhand };
        let Some(weapon) = self.item_by_slot(shooter, slot) else { return };
        let max_angle = enchantment_helper::process_projectile_spread(self, &weapon, shooter, 0.0);
        let angle_step = if projectiles.len() == 1 { 0.0 } else { 2.0 * max_angle / (projectiles.len() - 1) as f32 };
        let angle_offset = ((projectiles.len() - 1) % 2) as f32 * angle_step / 2.0;
        let mut direction = 1.0f32;
        for (i, projectile) in projectiles.iter().enumerate() {
            if projectile.is_empty() {
                continue;
            }
            let angle = angle_offset + direction * ((i + 1) / 2) as f32 * angle_step;
            direction = -direction;
            let weapon = self.item_by_slot(shooter, slot).unwrap_or_else(ItemStack::empty);
            let projectile_entity = self.create_projectile(shooter, &weapon, projectile, is_crit);
            let item = weapon.get_item();
            self.spawn_projectile_from_weapon(item, projectile_entity, projectile, shooter, i, power, uncertainty, angle, target_override);
            self.hurt_and_break(shooter, slot, get_durability_use(item, projectile));
            if self.item_by_slot(shooter, slot).is_none_or(|weapon| weapon.is_empty()) {
                break;
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn spawn_projectile_from_weapon(
        &mut self,
        weapon: Item,
        projectile_entity: Projectile,
        projectile: &ItemStack,
        shooter: EntityId,
        index: usize,
        power: f32,
        uncertainty: f32,
        angle: f32,
        target_override: Option<EntityId>,
    ) {
        if weapon.is_instance_of("CrossbowItem") {
            let [x, y, z] = crossbow_item::shot_vector(self, shooter, angle, target_override);
            self.spawn_projectile(projectile_entity, projectile, |p| p.shoot(x as f64, y as f64, z as f64, power, uncertainty));
            crossbow_item::play_shoot_sound(self, shooter, index);
        } else {
            let (movement, on_ground) = (self.known_movement(shooter), self.entity(shooter).on_ground);
            let (x_rot, y_rot) = (self.entity(shooter).x_rot, self.entity(shooter).y_rot);
            self.spawn_projectile(projectile_entity, projectile, |p| p.shoot_from_rotation(movement, on_ground, x_rot, y_rot + angle, 0.0, power, uncertainty));
        }
    }

    fn create_projectile(&mut self, shooter: EntityId, weapon: &ItemStack, projectile: &ItemStack, is_crit: bool) -> Projectile {
        let item = weapon.get_item();
        if item.is_instance_of("CrossbowItem") {
            return crossbow_item::create_projectile(self, shooter, weapon, projectile, is_crit);
        }
        self.projectile_weapon_create_projectile(shooter, weapon, projectile, is_crit)
    }

    pub fn projectile_weapon_create_projectile(&mut self, shooter: EntityId, weapon: &ItemStack, projectile: &ItemStack, is_crit: bool) -> Projectile {
        let projectile_item = projectile.get_item();
        let arrow_item = if projectile_item.is_instance_of("ArrowItem") { projectile_item } else { items::ARROW };
        let mut arrow = arrow_item::create_arrow(self, arrow_item, projectile, shooter, Some(weapon));
        if is_crit {
            arrow.set_crit_arrow(true);
        }
        arrow
    }
}

fn get_durability_use(weapon: Item, projectile: &ItemStack) -> i32 {
    if weapon.is_instance_of("CrossbowItem") && projectile.is(items::FIREWORK_ROCKET) { 3 } else { 1 }
}
