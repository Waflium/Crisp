use crisp_core::random::RandomSource;
use crisp_core::{AABB, Vec3};
use crisp_data::packets::play::clientbound;
use crisp_data::{fluid_tags, registry, registry_id};

use super::data::{DataValue, entity_defaults};
use super::{Entity, EntityDimensions, EntityId, LevelEntity, MoverType};
use crate::network::connection::encode;
use crate::server::level::ServerLevel;
use crate::world::damagesource::DamageSource;
use crate::world::item::enchantment::enchantment_effect_components::REPAIR_WITH_XP;
use crate::world::level::BlockGetter;

pub const LIFETIME: i32 = 6000;
const ORB_GROUPS_PER_AREA: i32 = 40;
const DATA_VALUE: usize = 8;

pub struct ExperienceOrb {
    pub entity: Entity,
    pub age: i32,
    pub health: i32,
    pub count: i32,
    pub following_player: Option<EntityId>,
}

impl ExperienceOrb {
    pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.5, 0.5).with_eye_height(0.425);

    pub fn of_type() -> ExperienceOrb {
        let mut data = entity_defaults();
        data.push(DataValue::Int(0));
        let entity = Entity::new(registry_id(registry::ENTITY_TYPE, "minecraft:experience_orb"), Self::DIMENSIONS, data);
        ExperienceOrb { entity, age: 0, health: 5, count: 1, following_player: None }
    }

    pub fn value(&self) -> i32 {
        match self.entity.data.get(DATA_VALUE) {
            DataValue::Int(value) => *value,
            _ => unreachable!("the value is an int"),
        }
    }

    pub fn set_value(&mut self, value: i32) {
        self.entity.data.set(DATA_VALUE, DataValue::Int(value));
    }

    fn can_merge(&self, id: EntityId, value: i32) -> bool {
        !self.entity.is_removed() && (self.entity.id - id) % ORB_GROUPS_PER_AREA == 0 && self.value() == value
    }
}

pub fn experience_value(max_value: i32) -> i32 {
    const SIZES: [i32; 10] = [2477, 1237, 617, 307, 149, 73, 37, 17, 7, 3];
    SIZES.into_iter().find(|&size| max_value >= size).unwrap_or(1)
}

impl ServerLevel {
    fn experience_orb(&mut self, id: EntityId) -> Option<&mut ExperienceOrb> {
        match self.entities.get_mut(&id) {
            Some(LevelEntity::ExperienceOrb(orb)) => Some(orb),
            _ => None,
        }
    }

    pub fn new_experience_orb(&self, pos: Vec3, roughly: Vec3, value: i32) -> ExperienceOrb {
        let mut orb = ExperienceOrb::of_type();
        let e = &mut orb.entity;
        e.set_pos(pos);
        let y_rot = e.random.next_float() * 360.0;
        e.set_y_rot(y_rot);
        let mut random_movement = Vec3::new(
            (e.random.next_double() * 0.2 - 0.1) * 2.0,
            e.random.next_double() * 0.2 * 2.0,
            (e.random.next_double() * 0.2 - 0.1) * 2.0,
        );
        if roughly.length_sqr() > 0.0 && roughly.dot(random_movement) < 0.0 {
            random_movement = random_movement.scale(-1.0);
        }
        let size = e.bounding_box().size();
        e.set_pos(pos.add_vec(roughly.normalize().scale(size * 0.5)));
        e.set_delta_movement(random_movement);
        if !self.no_collision(&e.bounding_box()) {
            let half_height = ExperienceOrb::DIMENSIONS.height as f64 / 2.0;
            let center = e.position().add(0.0, half_height, 0.0);
            let (width, height) = (ExperienceOrb::DIMENSIONS.width as f64, ExperienceOrb::DIMENSIONS.height as f64);
            if let Some(free) = self.find_free_position(&crate::world::phys::shapes::collision_context::CollisionContext::of_detached(e), &AABB::of_size(center, size, size, size), center, width, height, width) {
                e.set_pos(free.add(0.0, -half_height, 0.0));
            }
        }
        orb.set_value(value);
        orb
    }

    pub fn award_experience(&mut self, pos: Vec3, amount: i32) {
        self.award_experience_with_direction(pos, Vec3::ZERO, amount);
    }

    pub fn award_experience_with_direction(&mut self, pos: Vec3, rough_direction: Vec3, mut amount: i32) {
        while amount > 0 {
            let value = experience_value(amount);
            amount -= value;
            if !self.try_merge_to_existing(pos, value) {
                let orb = self.new_experience_orb(pos, rough_direction, value);
                self.add_fresh_entity(LevelEntity::ExperienceOrb(orb));
            }
        }
    }

    fn try_merge_to_existing(&mut self, pos: Vec3, value: i32) -> bool {
        let area = AABB::of_size(pos, 1.0, 1.0, 1.0);
        let id = self.random.next_int_bounded(ORB_GROUPS_PER_AREA);
        let found = self.experience_orbs_in(&area, None).into_iter().find(|&orb| self.entities[&orb].as_experience_orb().is_some_and(|o| o.can_merge(id, value)));
        let Some(orb) = found.and_then(|orb| self.experience_orb(orb)) else { return false };
        orb.count += 1;
        orb.age = 0;
        true
    }

    pub fn experience_orbs_in(&self, area: &AABB, except: Option<EntityId>) -> Vec<EntityId> {
        let mut found = Vec::new();
        self.entity_manager.section_storage.for_each_accessible_non_empty_section(area, |section| {
            for &id in section.entities() {
                if Some(id) != except
                    && let Some(LevelEntity::ExperienceOrb(orb)) = self.entities.get(&id)
                    && !orb.entity.is_removed()
                    && orb.entity.bounding_box().intersects(area)
                {
                    found.push(id);
                }
            }
        });
        found
    }

    pub fn tick_experience_orb(&mut self, id: EntityId) {
        self.entity_base_tick(id);
        if self.experience_orb(id).is_none() {
            return;
        }
        let colliding = !self.no_collision(&self.entity(id).bounding_box());
        if self.entity(id).is_eye_in_fluid(&fluid_tags::WATER) {
            let e = self.entity_mut(id);
            let m = e.delta_movement;
            e.set_delta_movement(Vec3::new(m.x * 0.99f32 as f64, (m.y + 5.0e-4f32 as f64).min(0.06f32 as f64), m.z * 0.99f32 as f64));
        } else if !colliding {
            self.apply_gravity(id);
        }
        let pos = self.entity(id).block_position();
        if self.fluid_state(pos).is(&fluid_tags::LAVA) {
            let e = self.entity_mut(id);
            let x = (e.random.next_float() - e.random.next_float()) * 0.2;
            let z = (e.random.next_float() - e.random.next_float()) * 0.2;
            e.set_delta_movement(Vec3::new(x as f64, 0.2f32 as f64, z as f64));
        }
        if self.entity(id).tick_count % 20 == 1 {
            self.scan_for_merges(id);
        }
        self.follow_nearby_player(id);
        let Some(orb) = self.entities[&id].as_experience_orb() else { return };
        if orb.following_player.is_none() && colliding {
            let e = &orb.entity;
            let bb = e.bounding_box();
            let (x, y, z) = (e.x(), (bb.min_y + bb.max_y) / 2.0, e.z());
            if !self.no_collision(&bb.move_vec(e.delta_movement)) {
                self.move_towards_closest_space(id, x, y, z);
                self.entity_mut(id).needs_sync = true;
            }
        }
        let delta = self.entity(id).delta_movement;
        let fall_speed = delta.y;
        self.move_entity(id, MoverType::SelfMove, delta);
        self.apply_effects_from_blocks(id);
        if self.experience_orb(id).is_none() {
            return;
        }
        let mut friction = self.air_drag(id);
        if self.entity(id).on_ground {
            friction *= self.friction_below(id);
        }
        let gravity = self.gravity(id);
        let e = self.entity_mut(id);
        e.set_delta_movement(e.delta_movement.scale(friction as f64));
        if e.vertical_collision_below && fall_speed < -gravity {
            let m = e.delta_movement;
            e.set_delta_movement(Vec3::new(m.x, -fall_speed * 0.4, m.z));
        }
        let Some(orb) = self.experience_orb(id) else { return };
        orb.age += 1;
        if orb.age >= LIFETIME {
            orb.entity.discard();
        }
    }

    fn scan_for_merges(&mut self, id: EntityId) {
        let Some(orb) = self.entities[&id].as_experience_orb() else { return };
        let (value, area) = (orb.value(), orb.entity.bounding_box().inflate_all(0.5));
        for other in self.experience_orbs_in(&area, Some(id)) {
            let [Some(LevelEntity::ExperienceOrb(this)), Some(LevelEntity::ExperienceOrb(that))] = self.entities.get_disjoint_mut([&id, &other]) else { continue };
            if !that.can_merge(id, value) {
                continue;
            }
            this.count += that.count;
            this.age = this.age.min(that.age);
            that.entity.discard();
            self.remove_entity(other);
        }
    }

    fn follow_nearby_player(&mut self, id: EntityId) {
        let orb = self.experience_orb(id).expect("ticking orb");
        let position = orb.entity.position();
        let keep = orb
            .following_player
            .filter(|&p| self.players.get(&p).is_some_and(|player| !player.entity.is_removed() && player.entity.position().distance_to_sqr(position) <= 64.0) && !self.is_spectator(p));
        let following = match keep {
            Some(player) => Some(player),
            None => self.nearest_player(position, 8.0).filter(|&p| !self.is_spectator(p) && !self.is_dead_or_dying(p)),
        };
        self.experience_orb(id).expect("ticking orb").following_player = following;
        let Some(player) = following else { return };
        let target = &self.player(player).entity;
        let delta = Vec3::new(target.x() - position.x, target.y() + target.dimensions.eye_height as f64 / 2.0 - position.y, target.z() - position.z);
        let power = 1.0 - delta.length_sqr().sqrt() / 8.0;
        let e = self.entity_mut(id);
        e.set_delta_movement(e.delta_movement.add_vec(delta.normalize().scale(power * power * 0.1)));
    }

    pub fn nearest_player(&self, pos: Vec3, range: f64) -> Option<EntityId> {
        let mut best: Option<(EntityId, f64)> = None;
        for p in self.players.values().filter(|p| !p.entity.is_removed() && !self.is_spectator(p.entity.id)) {
            let distance = p.entity.position().distance_to_sqr(pos);
            if (range < 0.0 || distance < range * range) && best.is_none_or(|(_, b)| distance < b) {
                best = Some((p.entity.id, distance));
            }
        }
        best.map(|(id, _)| id)
    }

    pub fn touch_experience_orbs(&mut self, player: EntityId) {
        let area = self.player_pickup_area(player);
        let orbs = self.experience_orbs_in(&area, None);
        if orbs.is_empty() {
            return;
        }
        let index = self.player_mut(player).entity.random.next_int_bounded(orbs.len() as i32);
        self.experience_orb_player_touch(orbs[index as usize], player);
    }

    fn experience_orb_player_touch(&mut self, id: EntityId, player: EntityId) {
        if self.player(player).experience.take_xp_delay != 0 {
            return;
        }
        self.player_mut(player).experience.take_xp_delay = 2;
        let packet = encode(clientbound::TAKE_ITEM_ENTITY, |p| {
            p.varint(id);
            p.varint(player);
            p.varint(1);
        });
        self.send_to_tracking_players(id, &packet);
        let value = self.experience_orb(id).expect("touched orb").value();
        let remaining = self.repair_player_items(player, value);
        if remaining > 0 {
            self.give_experience_points(player, remaining);
        }
        let orb = self.experience_orb(id).expect("touched orb");
        orb.count -= 1;
        if orb.count == 0 {
            orb.entity.discard();
            self.remove_entity(id);
        }
    }

    fn repair_player_items(&mut self, player: EntityId, amount: i32) -> i32 {
        let Some(selected) = self.get_random_item_with(REPAIR_WITH_XP, player, |s| s.is_damaged()) else { return amount };
        let mut item_stack = selected.item_stack;
        let to_repair = self.modify_durability_to_repair_from_xp(&item_stack, amount);
        let repair = to_repair.min(item_stack.damage_value());
        item_stack.set_damage_value(item_stack.damage_value() - repair);
        if let Some(slot) = selected.in_slot {
            self.set_item_slot(player, slot, item_stack);
        }
        if repair > 0 {
            let remaining = amount - repair.wrapping_mul(amount) / to_repair;
            if remaining > 0 {
                return self.repair_player_items(player, remaining);
            }
        }
        0
    }

    pub(crate) fn experience_orb_hurt_server(&mut self, id: EntityId, source: &DamageSource, damage: f32) -> bool {
        if self.is_invulnerable_to_base(id, source) {
            return false;
        }
        let Some(orb) = self.experience_orb(id) else { return false };
        orb.entity.sync_velocity = true;
        orb.health = (orb.health as f32 - damage) as i32;
        if orb.health <= 0 {
            orb.entity.discard();
        }
        true
    }
}
