use crisp_core::random::RandomSource;
use crisp_core::{AABB, Vec3, mth};
use crisp_data::{fluid_tags, registry, registry_id};

use super::data::{DataValue, ids};
use super::{Entity, EntityDimensions, EntityId, LevelEntity, MoverType};
use crate::server::level::ServerLevel;
use crate::world::item::ItemStack;

pub const LIFETIME: i32 = 6000;
const INFINITE_PICKUP_DELAY: i32 = 32767;
const INFINITE_LIFETIME: i32 = -32768;

pub struct ItemEntity {
    pub entity: Entity,
    pub item: ItemStack,
    pub age: i32,
    pub pickup_delay: i32,
    pub bob_offs: f32,
    pub target: Option<u128>,
    pub health: i32,
    pub thrower: Option<u128>,
}

impl ItemEntity {
    pub const DIMENSIONS: EntityDimensions = EntityDimensions { width: 0.25, height: 0.25, eye_height: 0.2125 };

    fn base() -> ItemEntity {
        let mut data = super::data::entity_defaults();
        data.push(DataValue::ItemStack(None));
        let mut entity = Entity::new(registry_id(registry::ENTITY_TYPE, "minecraft:item"), Self::DIMENSIONS, data);
        let bob_offs = entity.random.next_float() * std::f32::consts::PI * 2.0;
        entity.y_rot = entity.random.next_float() * 360.0;
        ItemEntity { entity, item: ItemStack::new(crisp_data::items::AIR, 0), age: 0, pickup_delay: 0, bob_offs, target: None, health: 5, thrower: None }
    }

    pub fn new(pos: Vec3, item: ItemStack) -> ItemEntity {
        let mut e = Self::base();
        e.entity.set_pos(pos);
        e.set_item(item);
        let dx = e.entity.random.next_double() * 0.2 - 0.1;
        let dz = e.entity.random.next_double() * 0.2 - 0.1;
        e.entity.delta_movement = Vec3::new(dx, 0.2, dz);
        e
    }

    pub fn with_movement(pos: Vec3, item: ItemStack, delta: Vec3) -> ItemEntity {
        let mut e = Self::base();
        e.entity.set_pos(pos);
        e.set_item(item);
        e.entity.delta_movement = delta;
        e
    }

    pub fn set_item(&mut self, item: ItemStack) {
        self.entity.data.set(ids::ITEM, DataValue::ItemStack((item.count > 0).then(|| item.clone())));
        self.item = item;
    }

    pub fn set_default_pickup_delay(&mut self) {
        self.pickup_delay = 10;
    }

    pub fn set_no_pickup_delay(&mut self) {
        self.pickup_delay = 0;
    }

    pub fn make_fake_item(&mut self) {
        self.pickup_delay = INFINITE_PICKUP_DELAY;
        self.age = LIFETIME - 1;
    }

    pub fn has_pickup_delay(&self) -> bool {
        self.pickup_delay > 0
    }

    pub fn is_mergeable(&self) -> bool {
        !self.entity.is_removed()
            && self.pickup_delay != INFINITE_PICKUP_DELAY
            && self.age != INFINITE_LIFETIME
            && self.age < LIFETIME
            && self.item.count < self.item.max_stack_size()
    }

    pub fn merge_area(&self) -> AABB {
        self.entity.bounding_box().inflate(0.5, 0.0, 0.5)
    }

    pub fn try_to_merge(this: &mut ItemEntity, other: &mut ItemEntity) {
        if this.target != other.target || this.item.count + other.item.count > other.item.max_stack_size() || !ItemStack::is_same_item_same_components(&this.item, &other.item) {
            return;
        }
        if other.item.count < this.item.count {
            Self::merge(this, other);
        } else {
            Self::merge(other, this);
        }
    }

    fn merge(to: &mut ItemEntity, from: &mut ItemEntity) {
        let delta = (to.item.max_stack_size().min(64) - to.item.count).min(from.item.count);
        let mut merged = to.item.clone();
        merged.count += delta;
        to.set_item(merged);
        from.item.count -= delta;
        to.pickup_delay = to.pickup_delay.max(from.pickup_delay);
        to.age = to.age.min(from.age);
        if from.item.count <= 0 {
            from.entity.discard();
        }
    }
}

impl ServerLevel {
    fn item_entity(&mut self, id: EntityId) -> Option<&mut ItemEntity> {
        match self.entities.get_mut(&id) {
            Some(LevelEntity::Item(item)) => Some(item),
            _ => None,
        }
    }

    pub fn tick_item(&mut self, id: EntityId) {
        let Some(item) = self.item_entity(id) else { return };
        if item.item.is_empty() {
            item.entity.discard();
            return;
        }
        self.entity_base_tick(id);
        let Some(item) = self.item_entity(id) else { return };
        if item.pickup_delay > 0 && item.pickup_delay != INFINITE_PICKUP_DELAY {
            item.pickup_delay -= 1;
        }
        let e = &mut item.entity;
        e.xo = e.position();
        let old_movement = e.delta_movement;
        if e.is_in_water() && e.fluid_height(&fluid_tags::WATER) > 0.1f32 as f64 {
            set_fluid_movement(e, 0.99f32 as f64);
        } else if e.is_in_lava() && e.fluid_height(&fluid_tags::LAVA) > 0.1f32 as f64 {
            set_fluid_movement(e, 0.95f32 as f64);
        } else {
            self.apply_gravity(id);
        }
        let bb = self.entity(id).bounding_box();
        let no_physics = !self.no_collision_for(id, &bb.deflate_all(1.0e-7));
        self.entity_mut(id).no_physics = no_physics;
        if no_physics {
            let e = self.entity(id);
            let (x, y, z) = (e.x(), (bb.min_y + bb.max_y) / 2.0, e.z());
            self.move_towards_closest_space(id, x, y, z);
        }
        let e = self.entity(id);
        if e.on_ground && !(e.delta_movement.horizontal_distance_sqr() > 1.0e-5f32 as f64) && (e.tick_count + e.id) % 4 != 0 {
            self.apply_effects_from_blocks_for_last_movements(id);
        } else {
            let delta = e.delta_movement;
            self.move_entity(id, MoverType::SelfMove, delta);
            self.apply_effects_from_blocks(id);
            if self.item_entity(id).is_none() {
                return;
            }
            let air_drag = self.air_drag(id);
            let mut ground_friction = air_drag;
            if self.entity(id).on_ground {
                ground_friction *= self.friction_below(id);
            }
            let e = self.entity_mut(id);
            e.set_delta_movement(e.delta_movement.multiply(ground_friction as f64, air_drag as f64, ground_friction as f64));
            if e.on_ground && e.delta_movement.y < 0.0 {
                e.set_delta_movement(e.delta_movement.multiply(1.0, -0.5, 1.0));
            }
        }
        let e = self.entity(id);
        let moved = mth::floor(e.xo.x) != mth::floor(e.x()) || mth::floor(e.xo.y) != mth::floor(e.y()) || mth::floor(e.xo.z) != mth::floor(e.z());
        let rate = if moved { 2 } else { 40 };
        if e.tick_count % rate == 0 && self.item_entity(id).is_some_and(|i| i.is_mergeable()) {
            self.merge_with_neighbours(id);
        }
        let Some(item) = self.item_entity(id) else { return };
        if item.age != INFINITE_LIFETIME {
            item.age += 1;
        }
        let in_fluid = self.update_fluid_interaction(id);
        let e = self.entity_mut(id);
        e.needs_sync |= in_fluid;
        if e.delta_movement.subtract(old_movement).length_sqr() > 0.01 {
            e.needs_sync = true;
        }
        let Some(item) = self.item_entity(id) else { return };
        if item.age >= LIFETIME {
            item.entity.discard();
        }
    }
}

fn set_fluid_movement(e: &mut Entity, multiplier: f64) {
    let m = e.delta_movement;
    let lift = if m.y < 0.06f32 as f64 { 5.0e-4f32 } else { 0.0 };
    e.set_delta_movement(Vec3::new(m.x * multiplier, m.y + lift as f64, m.z * multiplier));
}
