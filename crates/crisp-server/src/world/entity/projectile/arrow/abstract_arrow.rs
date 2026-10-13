use crisp_core::random::RandomSource;
use crisp_core::{AABB, Vec3, mth};
use crisp_data::packets::play::clientbound;
use crisp_data::{BlockState, damage_types, registry};
use crisp_protocol::nbt::Nbt;

use crate::advancements::triggers::criteria_triggers;
use crate::server::level::ServerLevel;
use crate::world::damagesource::DamageSource;
use crate::world::entity::attributes::Attribute;
use crate::world::entity::data::{DataValue, entity_defaults};
use crate::world::entity::entity_reference::EntityReference;
use crate::world::entity::projectile::projectile::{Projectile, ProjectileKind, lerp_rotation};
use crate::world::entity::projectile::projectile_deflection::ProjectileDeflection;
use crate::world::entity::projectile::projectile_util::compute_margin;
use crate::world::entity::{EntityDimensions, EntityId, sound_event};
use crate::world::item::ItemStack;
use crate::world::item::component as dc;
use crate::world::item::enchantment::enchantment_helper;
use crate::world::level::BlockGetter;
use crate::world::level::block::BlockHitResult;
use crate::world::level::block::entity::int_value;
use crate::world::level::chunk::serialization::{state_from_nbt, state_to_nbt};
use crate::world::level::clip::{ClipBlock, ClipContext, ClipFluid};
use crate::world::phys::entity_hit_result::EntityHitResult;
use crate::world::phys::hit_result::HitResult;

const ARROW_BASE_DAMAGE: f64 = 2.0;
const SHAKE_TIME: i32 = 7;
const WATER_INERTIA: f32 = 0.6;
const ID_FLAGS: usize = 8;
const PIERCE_LEVEL: usize = 9;
const IN_GROUND: usize = 10;
const FLAG_CRIT: i8 = 1;
const FLAG_NOPHYSICS: i8 = 2;
const GAME_EVENT_PLAY_ARROW_HIT_SOUND: u8 = 6;
pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.5, 0.5).with_eye_height(0.13);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Pickup {
    Disallowed,
    Allowed,
    CreativeOnly,
}

impl Pickup {
    pub fn by_ordinal(ordinal: i32) -> Pickup {
        match ordinal {
            1 => Pickup::Allowed,
            2 => Pickup::CreativeOnly,
            _ => Pickup::Disallowed,
        }
    }
}

pub struct AbstractArrow {
    last_state: Option<BlockState>,
    pub in_ground_time: i32,
    pub pickup: Pickup,
    pub shake_time: i32,
    pub life: i32,
    base_damage: f64,
    sound_event: i32,
    piercing_ignore_entity_ids: Option<Vec<EntityId>>,
    pierced_and_killed_entities: Option<Vec<EntityId>>,
    pub pickup_item_stack: ItemStack,
    pub fired_from_weapon: Option<ItemStack>,
}

impl AbstractArrow {
    pub fn new(default_pickup_item: ItemStack) -> AbstractArrow {
        AbstractArrow {
            last_state: None,
            in_ground_time: 0,
            pickup: Pickup::Disallowed,
            shake_time: 0,
            life: 0,
            base_damage: ARROW_BASE_DAMAGE,
            sound_event: get_default_hit_ground_sound_event(),
            piercing_ignore_entity_ids: None,
            pierced_and_killed_entities: None,
            pickup_item_stack: default_pickup_item,
            fired_from_weapon: None,
        }
    }

    pub fn set_sound_event(&mut self, sound_event: i32) {
        self.sound_event = sound_event;
    }

    pub fn set_base_damage(&mut self, base_damage: f64) {
        self.base_damage = base_damage;
    }

    fn reset_pierced_entities(&mut self) {
        if let Some(entities) = &mut self.pierced_and_killed_entities {
            entities.clear();
        }
        if let Some(ids) = &mut self.piercing_ignore_entity_ids {
            ids.clear();
        }
    }
}

pub fn get_default_hit_ground_sound_event() -> i32 {
    sound_event("minecraft:entity.arrow.hit")
}

pub fn defaults() -> Vec<DataValue> {
    let mut data = entity_defaults();
    data.push(DataValue::Byte(0));
    data.push(DataValue::Byte(0));
    data.push(DataValue::Boolean(false));
    data
}

pub fn get_default_pickup_item(kind: &ProjectileKind) -> ItemStack {
    match kind {
        ProjectileKind::SpectralArrow(_) => ItemStack::new(crisp_data::items::SPECTRAL_ARROW, 1),
        _ => ItemStack::new(crisp_data::items::ARROW, 1),
    }
}

pub fn at(level: &mut ServerLevel, mut projectile: Projectile, x: f64, y: f64, z: f64, pickup_item_stack: &ItemStack, fired_from_weapon: Option<&ItemStack>) -> Projectile {
    let mut pickup_item_stack = pickup_item_stack.clone();
    projectile.abstract_arrow_mut().expect("an arrow").pickup_item_stack = pickup_item_stack.copy();
    projectile.apply_implicit_components(&pickup_item_stack);
    if pickup_item_stack.remove(dc::INTANGIBLE_PROJECTILE).is_some() {
        projectile.abstract_arrow_mut().expect("an arrow").pickup = Pickup::CreativeOnly;
    }
    projectile.entity.set_pos(Vec3::new(x, y, z));
    if let Some(fired_from_weapon) = fired_from_weapon {
        let stored = projectile.abstract_arrow().expect("an arrow").pickup_item_stack.clone();
        projectile.abstract_arrow_mut().expect("an arrow").fired_from_weapon = Some(fired_from_weapon.copy());
        let pierce_level = enchantment_helper::get_piercing_count(level, fired_from_weapon, &stored);
        if pierce_level > 0 {
            projectile.set_pierce_level(pierce_level as i8);
        }
    }
    projectile
}

pub fn from_owner(level: &mut ServerLevel, projectile: Projectile, mob: EntityId, pickup_item_stack: &ItemStack, fired_from_weapon: Option<&ItemStack>) -> Projectile {
    let o = level.entity(mob);
    let (x, y, z) = (o.x(), o.eye_y() - 0.1f32 as f64, o.z());
    let mut projectile = at(level, projectile, x, y, z, pickup_item_stack, fired_from_weapon);
    let uuid = level.entity(mob).uuid;
    projectile.owner = Some(EntityReference::of(uuid));
    if level.is_player(mob) {
        let arrow = projectile.abstract_arrow_mut().expect("an arrow");
        if arrow.pickup == Pickup::Disallowed {
            arrow.pickup = Pickup::Allowed;
        }
    }
    projectile
}

fn flags(projectile: &Projectile) -> i8 {
    match projectile.entity.data.get(ID_FLAGS) {
        DataValue::Byte(flags) => *flags,
        _ => 0,
    }
}

fn java_signum(v: f64) -> f64 {
    if v == 0.0 || v.is_nan() { v } else { v.signum() }
}

impl Projectile {
    pub fn is_in_ground(&self) -> bool {
        matches!(self.entity.data.get(IN_GROUND), DataValue::Boolean(true))
    }

    pub fn set_in_ground(&mut self, in_ground: bool) {
        self.entity.data.set(IN_GROUND, DataValue::Boolean(in_ground));
    }

    pub fn set_crit_arrow(&mut self, crit_arrow: bool) {
        self.set_flag(FLAG_CRIT, crit_arrow);
    }

    pub fn is_crit_arrow(&self) -> bool {
        flags(self) & FLAG_CRIT != 0
    }

    pub fn set_pierce_level(&mut self, pierce_level: i8) {
        self.entity.data.set(PIERCE_LEVEL, DataValue::Byte(pierce_level));
    }

    pub fn get_pierce_level(&self) -> i8 {
        match self.entity.data.get(PIERCE_LEVEL) {
            DataValue::Byte(level) => *level,
            _ => 0,
        }
    }

    fn set_flag(&mut self, flag: i8, value: bool) {
        let flags = flags(self);
        self.entity.data.set(ID_FLAGS, DataValue::Byte(if value { flags | flag } else { flags & !flag }));
    }

    pub fn set_no_physics(&mut self, no_physics: bool) {
        self.entity.no_physics = no_physics;
        self.set_flag(FLAG_NOPHYSICS, no_physics);
    }

    pub fn is_no_physics(&self) -> bool {
        self.entity.no_physics
    }

    pub fn set_base_damage_from_mob(&mut self, power: f32, difficulty_id: i32) {
        let base_damage = (power * 2.0) as f64 + self.entity.random.triangle(difficulty_id as f64 * 0.11, 0.57425);
        self.abstract_arrow_mut().expect("an arrow").set_base_damage(base_damage);
    }

    pub fn get_pickup_item(&self) -> ItemStack {
        self.abstract_arrow().expect("an arrow").pickup_item_stack.copy()
    }

    pub fn set_pickup_item_stack(&mut self, item_stack: ItemStack) {
        let item_stack = if !item_stack.is_empty() { item_stack } else { get_default_pickup_item(&self.kind) };
        self.abstract_arrow_mut().expect("an arrow").pickup_item_stack = item_stack;
        if let ProjectileKind::Arrow(_) = self.kind {
            self.arrow_update_color();
        }
    }

    pub fn abstract_arrow_add_additional_save_data(&self, output: &mut Vec<(String, Nbt)>) {
        let arrow = self.abstract_arrow().expect("an arrow");
        output.push(("life".into(), Nbt::Short(arrow.life as i16)));
        if let Some(last_state) = arrow.last_state {
            output.push(("inBlockState".into(), state_to_nbt(last_state)));
        }
        output.push(("shake".into(), Nbt::Byte(arrow.shake_time as i8)));
        output.push(("inGround".into(), Nbt::Byte(self.is_in_ground() as i8)));
        output.push(("pickup".into(), Nbt::Byte(arrow.pickup as i8)));
        output.push(("damage".into(), Nbt::Double(arrow.base_damage)));
        output.push(("crit".into(), Nbt::Byte(self.is_crit_arrow() as i8)));
        output.push(("PierceLevel".into(), Nbt::Byte(self.get_pierce_level())));
        output.push(("SoundEvent".into(), Nbt::String(registry::SOUND_EVENT[arrow.sound_event as usize].into())));
        output.push(("item".into(), arrow.pickup_item_stack.to_nbt()));
        if let Some(weapon) = &arrow.fired_from_weapon {
            output.push(("weapon".into(), weapon.to_nbt()));
        }
    }

    pub fn abstract_arrow_read_additional_save_data(&mut self, input: &Nbt) {
        let number = |key: &str| input.get(key).and_then(int_value);
        let life = number("life").map_or(0, |v| v as i16 as i32);
        let last_state = input.get("inBlockState").and_then(state_from_nbt);
        let shake_time = number("shake").map_or(0, |v| v as i8 as i32 & 255);
        self.set_in_ground(number("inGround").is_some_and(|v| v as i8 != 0));
        let base_damage = input.get("damage").and_then(Nbt::as_f64).unwrap_or(ARROW_BASE_DAMAGE);
        let pickup = number("pickup").map_or(Pickup::Disallowed, |v| Pickup::by_ordinal(v as i8 as i32));
        self.set_crit_arrow(number("crit").is_some_and(|v| v as i8 != 0));
        self.set_pierce_level(number("PierceLevel").map_or(0, |v| v as i8));
        let sound_event = input
            .get("SoundEvent")
            .and_then(Nbt::as_str)
            .and_then(crate::commands::arguments::parse_identifier)
            .and_then(|name| registry::SOUND_EVENT.iter().position(|&s| s == name))
            .map_or_else(get_default_hit_ground_sound_event, |id| id as i32);
        let item = input.get("item").and_then(|item| ItemStack::from_nbt(item).ok()).unwrap_or_else(|| get_default_pickup_item(&self.kind));
        let weapon = input.get("weapon").and_then(|weapon| ItemStack::from_nbt(weapon).ok());
        let arrow = self.abstract_arrow_mut().expect("an arrow");
        arrow.life = life;
        arrow.last_state = last_state;
        arrow.shake_time = shake_time;
        arrow.base_damage = base_damage;
        arrow.pickup = pickup;
        arrow.sound_event = sound_event;
        self.set_pickup_item_stack(item);
        self.abstract_arrow_mut().expect("an arrow").fired_from_weapon = weapon;
    }
}

impl ServerLevel {
    fn abstract_arrow(&self, id: EntityId) -> &AbstractArrow {
        self.projectile(id).abstract_arrow().expect("an arrow")
    }

    fn abstract_arrow_mut(&mut self, id: EntityId) -> &mut AbstractArrow {
        self.projectile_mut(id).abstract_arrow_mut().expect("an arrow")
    }

    pub fn abstract_arrow_tick(&mut self, id: EntityId) {
        let physics_enabled = !self.projectile(id).is_no_physics();
        let movement = self.entity(id).delta_movement;
        let block_pos = self.entity(id).block_position();
        let block_state = self.block_state(block_pos);
        if !block_state.is_air() && physics_enabled {
            let shape = block_state.collision_voxel_shape();
            if !shape.is_empty() {
                let position = self.entity(id).position();
                for aabb in shape.to_aabbs() {
                    if aabb.move_to_block(block_pos).contains_vec(position) {
                        let projectile = self.projectile_mut(id);
                        projectile.entity.set_delta_movement(Vec3::ZERO);
                        projectile.set_in_ground(true);
                        break;
                    }
                }
            }
        }
        let arrow = self.abstract_arrow_mut(id);
        if arrow.shake_time > 0 {
            arrow.shake_time -= 1;
        }
        if self.is_in_water_or_rain(id) {
            self.clear_fire(id);
        }
        if self.projectile(id).is_in_ground() && physics_enabled {
            if self.abstract_arrow(id).last_state != Some(block_state) && self.should_fall(id) {
                self.start_falling(id);
            } else {
                self.tick_despawn(id);
            }
            self.abstract_arrow_mut(id).in_ground_time += 1;
            if self.is_alive(id) {
                self.apply_effects_from_blocks(id);
            }
            if self.entities.contains_key(&id) {
                let on_fire = self.entity(id).remaining_fire_ticks > 0;
                self.entity_mut(id).set_shared_flag(crate::world::entity::flags::ON_FIRE, on_fire);
            }
            return;
        }
        self.abstract_arrow_mut(id).in_ground_time = 0;
        let original_position = self.entity(id).position();
        if self.entity(id).is_in_water() {
            self.apply_inertia(id, self.get_water_inertia(id));
        }
        let y_rot = if !physics_enabled {
            (mth::atan2(-movement.x, -movement.z) * mth::RAD_TO_DEG as f64) as f32
        } else {
            (mth::atan2(movement.x, movement.z) * mth::RAD_TO_DEG as f64) as f32
        };
        let x_rot = (mth::atan2(movement.y, movement.horizontal_distance()) * mth::RAD_TO_DEG as f64) as f32;
        let e = self.entity_mut(id);
        e.set_x_rot(lerp_rotation(e.x_rot, x_rot));
        e.set_y_rot(lerp_rotation(e.y_rot, y_rot));
        self.check_left_owner(id);
        if physics_enabled {
            let ctx = ClipContext { from: original_position, to: original_position.add_vec(movement), block: ClipBlock::Collider, fluid: ClipFluid::None, is_player: false };
            let clip = self.clip_including_border(&ctx);
            self.step_move_and_hit(id, clip.hit, clip.miss);
        } else {
            self.entity_mut(id).set_pos(original_position.add_vec(movement));
            self.entity_moved(id);
            self.apply_effects_from_blocks(id);
        }
        if !self.entities.contains_key(&id) {
            return;
        }
        if !self.entity(id).is_in_water() {
            self.apply_inertia(id, self.air_drag(id));
        }
        if physics_enabled && !self.projectile(id).is_in_ground() {
            self.apply_gravity(id);
        }
        self.projectile_base_tick(id);
    }

    fn step_move_and_hit(&mut self, id: EntityId, block_hit_result: BlockHitResult, miss: bool) {
        while self.is_alive(id) {
            let initial_position = self.entity(id).position();
            let mut entities_hit = self.find_hit_entities(id, initial_position, block_hit_result.location);
            entities_hit.sort_by(|a, b| initial_position.distance_to_sqr(self.entity(a.entity).position()).total_cmp(&initial_position.distance_to_sqr(self.entity(b.entity).position())));
            let next_location = entities_hit.first().map_or(block_hit_result.location, |hit| hit.location);
            self.entity_mut(id).set_pos(next_location);
            self.entity_moved(id);
            self.apply_effects_from_blocks_between(id, initial_position, next_location);
            if self.entities.contains_key(&id) && self.entity(id).portal_process.is_some_and(|process| process.is_inside_portal_this_tick()) {
                self.handle_portal(id);
            }
            if entities_hit.is_empty() {
                if self.is_alive(id) && !miss {
                    self.hit_target_or_deflect_self(id, HitResult::Block { hit: block_hit_result, miss });
                    self.entity_mut(id).needs_sync = true;
                }
                break;
            } else if self.is_alive(id) && !self.entity(id).no_physics {
                let deflection = self.hit_targets_or_deflect_self(id, &entities_hit);
                self.entity_mut(id).needs_sync = true;
                if self.projectile(id).get_pierce_level() > 0 && deflection == ProjectileDeflection::None {
                    continue;
                }
                break;
            }
        }
    }

    fn hit_targets_or_deflect_self(&mut self, id: EntityId, entity_hit_results: &[EntityHitResult]) -> ProjectileDeflection {
        for &e in entity_hit_results {
            let deflection = self.hit_target_or_deflect_self(id, HitResult::Entity(e));
            if !self.is_alive(id) || deflection != ProjectileDeflection::None {
                return deflection;
            }
        }
        ProjectileDeflection::None
    }

    fn apply_inertia(&mut self, id: EntityId, inertia: f32) {
        let e = self.entity_mut(id);
        e.set_delta_movement(e.delta_movement.scale(inertia as f64));
    }

    fn get_water_inertia(&self, _id: EntityId) -> f32 {
        WATER_INERTIA
    }

    fn should_fall(&self, id: EntityId) -> bool {
        let position = self.entity(id).position();
        self.projectile(id).is_in_ground() && self.no_collision(&AABB::from_corners(position, position).inflate_all(0.06))
    }

    fn start_falling(&mut self, id: EntityId) {
        let projectile = self.projectile_mut(id);
        projectile.set_in_ground(false);
        let e = &mut projectile.entity;
        let delta_movement = e.delta_movement;
        let (x, y, z) = (e.random.next_float() * 0.2, e.random.next_float() * 0.2, e.random.next_float() * 0.2);
        e.set_delta_movement(delta_movement.multiply(x as f64, y as f64, z as f64));
        self.abstract_arrow_mut(id).life = 0;
    }

    pub fn abstract_arrow_after_move(&mut self, id: EntityId, mover: crate::world::entity::MoverType) {
        if mover != crate::world::entity::MoverType::SelfMove && self.should_fall(id) {
            self.start_falling(id);
        }
    }

    fn tick_despawn(&mut self, id: EntityId) {
        let arrow = self.abstract_arrow_mut(id);
        arrow.life += 1;
        if arrow.life >= 1200 {
            self.entity_mut(id).discard();
        }
    }

    pub fn abstract_arrow_on_item_break(&mut self, id: EntityId) {
        self.abstract_arrow_mut(id).fired_from_weapon = None;
    }

    pub fn abstract_arrow_on_hit_entity(&mut self, id: EntityId, hit_result: EntityHitResult) {
        let entity = hit_result.entity;
        let pow = self.entity(id).delta_movement.length() as f32;
        let mut arrow_damage = self.abstract_arrow(id).base_damage;
        let current_owner = self.projectile_get_owner(id);
        let damage_source = DamageSource::source(damage_types::ARROW, Some(id), Some(current_owner.unwrap_or(id)));
        if let Some(weapon) = self.abstract_arrow(id).fired_from_weapon.clone() {
            arrow_damage = self.modify_damage(&weapon, entity, &damage_source, arrow_damage as f32) as f64;
        }
        let mut damage = mth::ceil(mth::clamp(pow as f64 * arrow_damage, 0.0, 2.147483647E9));
        let pierce_level = self.projectile(id).get_pierce_level();
        if pierce_level > 0 {
            let arrow = self.abstract_arrow_mut(id);
            let ignore = arrow.piercing_ignore_entity_ids.get_or_insert_with(|| Vec::with_capacity(5));
            arrow.pierced_and_killed_entities.get_or_insert_with(|| Vec::with_capacity(5));
            if ignore.len() >= pierce_level as usize + 1 {
                self.entity_mut(id).discard();
                return;
            }
            if !ignore.contains(&entity) {
                ignore.push(entity);
            }
        }
        if self.projectile(id).is_crit_arrow() {
            let dmg_increase = self.entity_mut(id).random.next_int_bounded(damage / 2 + 2) as i64;
            damage = (dmg_increase + damage as i64).min(2147483647) as i32;
        }
        if let Some(owner) = current_owner.filter(|&owner| self.is_living(owner)) {
            self.set_last_hurt_mob(owner, entity);
        }
        let remaining_fire_ticks = self.entity(entity).remaining_fire_ticks;
        if self.is_on_fire(id) {
            self.ignite_for_seconds(entity, 5.0);
        }
        if self.hurt_server(entity, damage_source, damage as f32) {
            if self.is_living(entity) {
                if self.projectile(id).get_pierce_level() <= 0 {
                    let count = self.get_arrow_count(entity);
                    self.set_arrow_count(entity, count + 1);
                }
                self.do_knockback(id, entity, &damage_source);
                let weapon = self.abstract_arrow(id).fired_from_weapon.clone();
                self.do_post_attack_effects_with_item_source(entity, &damage_source, weapon.as_ref());
                self.do_post_hurt_effects(id, entity);
                if self.is_player(entity)
                    && let Some(owner_player) = current_owner.filter(|&owner| self.is_player(owner))
                    && !self.entity(id).is_silent()
                    && entity != owner_player
                {
                    self.player(owner_player).connection.send(clientbound::GAME_EVENT, |w| {
                        w.u8(GAME_EVENT_PLAY_ARROW_HIT_SOUND);
                        w.f32(0.0);
                    });
                }
                let alive = self.is_alive(entity);
                if !alive && let Some(killed) = &mut self.abstract_arrow_mut(id).pierced_and_killed_entities {
                    killed.push(entity);
                }
                if let Some(player) = current_owner.filter(|&owner| self.is_player(owner)) {
                    let weapon = self.abstract_arrow(id).fired_from_weapon.clone();
                    match self.abstract_arrow(id).pierced_and_killed_entities.clone() {
                        Some(killed) => criteria_triggers::KILLED_BY_ARROW.trigger(self, player, &killed, weapon.as_ref()),
                        None if !alive => criteria_triggers::KILLED_BY_ARROW.trigger(self, player, &[entity], weapon.as_ref()),
                        None => {}
                    }
                }
            }
            let sound = self.abstract_arrow(id).sound_event;
            let random = &mut self.entity_mut(id).random;
            let pitch = 1.2 / (random.next_float() * 0.2 + 0.9);
            self.entity_play_sound(id, sound, 1.0, pitch);
            if self.projectile(id).get_pierce_level() <= 0 {
                self.entity_mut(id).discard();
            }
        } else {
            self.set_remaining_fire_ticks(entity, remaining_fire_ticks);
            let owner = self.projectile(id).owner;
            self.deflect(id, ProjectileDeflection::Reverse, Some(entity), owner, false, Vec3::new(0.2, 0.2, 0.2));
            if self.entity(id).delta_movement.length_sqr() < 1.0e-7 {
                if self.abstract_arrow(id).pickup == Pickup::Allowed {
                    let item = self.projectile(id).get_pickup_item();
                    self.spawn_at_location(id, item, 0.1);
                }
                self.entity_mut(id).discard();
            }
        }
    }

    fn do_knockback(&mut self, id: EntityId, mob: EntityId, damage_source: &DamageSource) {
        let knockback = match self.abstract_arrow(id).fired_from_weapon.clone() {
            Some(weapon) => self.modify_knockback(&weapon, mob, damage_source, 0.0) as f64,
            None => 0.0,
        };
        if knockback > 0.0 {
            let knockback_resistance = (1.0 - self.attribute_value(mob, Attribute::KnockbackResistance)).max(0.0);
            let movement = self.entity(id).delta_movement.multiply(1.0, 0.0, 1.0).normalize().scale(knockback * 0.6 * knockback_resistance);
            if movement.length_sqr() > 0.0 {
                self.entity_mut(mob).push(movement.x, 0.1, movement.z);
            }
        }
    }

    fn do_post_hurt_effects(&mut self, id: EntityId, mob: EntityId) {
        match self.projectile(id).kind {
            ProjectileKind::Arrow(_) => self.arrow_do_post_hurt_effects(id, mob),
            ProjectileKind::SpectralArrow(_) => self.spectral_arrow_do_post_hurt_effects(id, mob),
            _ => {}
        }
    }

    pub fn abstract_arrow_on_hit_block(&mut self, id: EntityId, hit_result: BlockHitResult) {
        let state = self.block_state(hit_result.block_pos);
        self.abstract_arrow_mut(id).last_state = Some(state);
        self.projectile_base_on_hit_block(id, hit_result);
        if let Some(weapon_item) = self.abstract_arrow(id).fired_from_weapon.clone() {
            self.hit_block_enchantment_effects(id, hit_result, &weapon_item);
        }
        let movement = self.entity(id).delta_movement;
        let offset_direction = Vec3::new(java_signum(movement.x), java_signum(movement.y), java_signum(movement.z));
        let scaled_movement = offset_direction.scale(0.05f32 as f64);
        let e = self.entity_mut(id);
        let position = e.position().subtract(scaled_movement);
        e.set_pos(position);
        e.set_delta_movement(Vec3::ZERO);
        self.entity_moved(id);
        let sound = self.abstract_arrow(id).sound_event;
        let random = &mut self.entity_mut(id).random;
        let pitch = 1.2 / (random.next_float() * 0.2 + 0.9);
        self.entity_play_sound(id, sound, 1.0, pitch);
        let projectile = self.projectile_mut(id);
        projectile.set_in_ground(true);
        projectile.set_crit_arrow(false);
        projectile.set_pierce_level(0);
        let arrow = projectile.abstract_arrow_mut().expect("an arrow");
        arrow.shake_time = SHAKE_TIME;
        arrow.set_sound_event(get_default_hit_ground_sound_event());
        arrow.reset_pierced_entities();
    }

    fn hit_block_enchantment_effects(&mut self, id: EntityId, hit_result: BlockHitResult, weapon: &ItemStack) {
        let pos = hit_result.block_pos;
        let location = hit_result.location;
        let clamp = |v: f64, min: i32| mth::clamp(v, (min as f32 + 1.0e-5f32) as f64, min as f64 + 1.0 - 1.0e-5f32 as f64);
        let compensated_hit_position = Vec3::new(clamp(location.x, pos.x), clamp(location.y, pos.y), clamp(location.z, pos.z));
        let owner = self.projectile_get_owner(id).filter(|&owner| self.is_living(owner));
        let state = self.block_state(pos);
        if enchantment_helper::on_hit_block(self, weapon, owner, id, compensated_hit_position, state) {
            self.abstract_arrow_on_item_break(id);
        }
    }

    fn find_hit_entities(&mut self, id: EntityId, from: Vec3, to: Vec3) -> Vec<EntityHitResult> {
        let e = self.entity(id);
        let area = e.bounding_box().expand_towards_vec(e.delta_movement).inflate_all(1.0);
        let margin = compute_margin(e.tick_count);
        self.get_many_entity_hit_result(id, from, to, &area, |level, entity| level.can_hit_entity(id, entity), margin, ClipBlock::Collider, false, false)
    }

    pub fn abstract_arrow_can_hit_entity(&self, id: EntityId, entity: EntityId) -> bool {
        if self.is_player(entity)
            && let Some(owner) = self.projectile_get_owner(id).filter(|&owner| self.is_player(owner))
            && !self.can_harm_player(owner, entity)
        {
            return false;
        }
        self.projectile_base_can_hit_entity(id, entity)
            && self.abstract_arrow(id).piercing_ignore_entity_ids.as_ref().is_none_or(|ids| !ids.contains(&entity))
    }

    pub fn abstract_arrow_player_touch(&mut self, id: EntityId, player: EntityId) {
        let projectile = self.projectile(id);
        if (projectile.is_in_ground() || projectile.is_no_physics()) && self.abstract_arrow(id).shake_time <= 0 && self.try_pickup(id, player) {
            self.take(player, id, 1);
            self.entity_mut(id).discard();
        }
    }

    fn try_pickup(&mut self, id: EntityId, player: EntityId) -> bool {
        match self.abstract_arrow(id).pickup {
            Pickup::Disallowed => false,
            Pickup::Allowed => {
                let item = self.projectile(id).get_pickup_item();
                self.player_inventory_add(player, item)
            }
            Pickup::CreativeOnly => self.has_infinite_materials(player),
        }
    }

    pub fn abstract_arrow_push(&mut self, id: EntityId, xa: f64, ya: f64, za: f64) {
        if !self.projectile(id).is_in_ground() {
            self.entity_mut(id).push(xa, ya, za);
        }
    }

    pub fn abstract_arrow_is_pushed_by_fluid(&self, id: EntityId) -> bool {
        !self.projectile(id).is_in_ground()
    }
}
