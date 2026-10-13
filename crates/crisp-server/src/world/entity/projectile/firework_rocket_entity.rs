use crisp_core::random::RandomSource;
use crisp_core::{Vec3, mth};
use crisp_data::{damage_types, items};
use crisp_protocol::nbt::Nbt;

use super::projectile::{Projectile, ProjectileKind};
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::damagesource::DamageSource;
use crate::world::entity::data::{DataValue, entity_defaults, ids};
use crate::world::entity::entity_reference::EntityReference;
use crate::world::entity::inside_block::InsideBlockEffects;
use crate::world::entity::{EntityDimensions, EntityId, MoverType, sound_event};
use crate::world::item::ItemStack;
use crate::world::item::component as dc;
use crate::world::level::BlockGetter;
use crate::world::level::block::{self, BlockHitResult};
use crate::world::level::block::entity::int_value;
use crate::world::level::clip::{ClipBlock, ClipContext, ClipFluid};
use crate::world::phys::hit_result::Type;

const ENTITY_TYPE: &str = "minecraft:firework_rocket";
const DATA_ID_FIREWORKS_ITEM: usize = 8;
const DATA_ATTACHED_TO_TARGET: usize = 9;
const DATA_SHOT_AT_ANGLE: usize = 10;
const EVENT_FIREWORKS_EXPLODE: u8 = 17;
pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.25, 0.25);

#[derive(Default)]
pub struct FireworkRocketEntity {
    life: i32,
    lifetime: i32,
    attached_to_entity: Option<EntityId>,
}

fn get_default_item() -> ItemStack {
    ItemStack::new(items::FIREWORK_ROCKET, 1)
}

fn firework(projectile: &mut Projectile) -> &mut FireworkRocketEntity {
    match &mut projectile.kind {
        ProjectileKind::FireworkRocket(firework) => firework,
        _ => unreachable!("not a firework rocket"),
    }
}

impl FireworkRocketEntity {
    pub fn of_type() -> Projectile {
        let mut data = entity_defaults();
        data.push(DataValue::ItemStack(Some(get_default_item())));
        data.push(DataValue::OptionalUnsignedInt(None));
        data.push(DataValue::Boolean(false));
        Projectile::new(ENTITY_TYPE, DIMENSIONS, data, ProjectileKind::FireworkRocket(FireworkRocketEntity::default()))
    }

    pub fn new(x: f64, y: f64, z: f64, source_item_stack: &ItemStack) -> Projectile {
        let mut projectile = FireworkRocketEntity::of_type();
        projectile.entity.set_pos(Vec3::new(x, y, z));
        projectile.entity.data.set(DATA_ID_FIREWORKS_ITEM, DataValue::ItemStack(Some(source_item_stack.copy())));
        let flight_count = 1 + source_item_stack.get(dc::FIREWORKS).and_then(|fireworks| fireworks.get("flight_duration")).and_then(int_value).unwrap_or(0);
        let random = &mut projectile.entity.random;
        let movement = Vec3::new(random.triangle(0.0, 0.002297), 0.05, random.triangle(0.0, 0.002297));
        projectile.entity.set_delta_movement(movement);
        let random = &mut projectile.entity.random;
        let lifetime = 10 * flight_count + random.next_int_bounded(6) + random.next_int_bounded(7);
        firework(&mut projectile).lifetime = lifetime;
        projectile
    }

    pub fn with_owner(owner: Option<u128>, x: f64, y: f64, z: f64, source_item_stack: &ItemStack) -> Projectile {
        let mut projectile = FireworkRocketEntity::new(x, y, z, source_item_stack);
        projectile.owner = owner.map(EntityReference::of);
        projectile
    }

    pub fn attached(level: &ServerLevel, source_item_stack: &ItemStack, stuck_to: EntityId) -> Projectile {
        let e = level.entity(stuck_to);
        let mut projectile = FireworkRocketEntity::with_owner(Some(e.uuid), e.x(), e.y(), e.z(), source_item_stack);
        projectile.entity.data.set(DATA_ATTACHED_TO_TARGET, DataValue::OptionalUnsignedInt(Some(stuck_to as u32)));
        firework(&mut projectile).attached_to_entity = Some(stuck_to);
        projectile
    }

    pub fn shot_at_angle(source_item_stack: &ItemStack, x: f64, y: f64, z: f64, shot_at_angle: bool) -> Projectile {
        let mut projectile = FireworkRocketEntity::new(x, y, z, source_item_stack);
        projectile.entity.data.set(DATA_SHOT_AT_ANGLE, DataValue::Boolean(shot_at_angle));
        projectile
    }

    pub fn shot_at_angle_with_owner(source_item_stack: &ItemStack, owner: u128, x: f64, y: f64, z: f64, shot_at_angle: bool) -> Projectile {
        let mut projectile = FireworkRocketEntity::shot_at_angle(source_item_stack, x, y, z, shot_at_angle);
        projectile.owner = Some(EntityReference::of(owner));
        projectile
    }
}

impl Projectile {
    fn firework_attached_to_target(&self) -> Option<u32> {
        match self.entity.data.get(DATA_ATTACHED_TO_TARGET) {
            DataValue::OptionalUnsignedInt(target) => *target,
            _ => None,
        }
    }

    pub fn is_shot_at_angle(&self) -> bool {
        matches!(self.entity.data.get(DATA_SHOT_AT_ANGLE), DataValue::Boolean(true))
    }

    pub fn firework_get_item(&self) -> ItemStack {
        match self.entity.data.get(DATA_ID_FIREWORKS_ITEM) {
            DataValue::ItemStack(Some(item)) => item.clone(),
            _ => ItemStack::empty(),
        }
    }

    fn get_explosions(&self) -> usize {
        self.firework_get_item().get(dc::FIREWORKS).and_then(|fireworks| fireworks.get("explosions")).map_or(0, |explosions| explosions.as_list().len())
    }

    pub fn firework_add_additional_save_data(&self, output: &mut Vec<(String, Nbt)>) {
        let ProjectileKind::FireworkRocket(rocket) = &self.kind else { unreachable!("not a firework rocket") };
        output.push(("Life".into(), Nbt::Int(rocket.life)));
        output.push(("LifeTime".into(), Nbt::Int(rocket.lifetime)));
        output.push(("FireworksItem".into(), self.firework_get_item().to_nbt()));
        output.push(("ShotAtAngle".into(), Nbt::Byte(self.is_shot_at_angle() as i8)));
    }

    pub fn firework_read_additional_save_data(&mut self, input: &Nbt) {
        let rocket = firework(self);
        rocket.life = input.get("Life").and_then(int_value).unwrap_or(0);
        rocket.lifetime = input.get("LifeTime").and_then(int_value).unwrap_or(0);
        let item = input.get("FireworksItem").and_then(|item| ItemStack::from_nbt(item).ok()).unwrap_or_else(get_default_item);
        self.entity.data.set(DATA_ID_FIREWORKS_ITEM, DataValue::ItemStack(Some(item)));
        let shot_at_angle = input.get("ShotAtAngle").and_then(int_value).is_some_and(|v| v != 0);
        self.entity.data.set(DATA_SHOT_AT_ANGLE, DataValue::Boolean(shot_at_angle));
    }
}

fn calculate_view_vector(x_rot: f32, y_rot: f32) -> Vec3 {
    let x_rot_rad = x_rot * mth::DEG_TO_RAD;
    let y_rot_rad = -y_rot * mth::DEG_TO_RAD;
    let (y_cos, y_sin) = (mth::cos(y_rot_rad as f64), mth::sin(y_rot_rad as f64));
    let (x_cos, x_sin) = (mth::cos(x_rot_rad as f64), mth::sin(x_rot_rad as f64));
    Vec3::new((y_sin * x_cos) as f64, -x_sin as f64, (y_cos * x_cos) as f64)
}

impl ServerLevel {
    fn get_hand_holding_item_angle(&self, id: EntityId, item: crisp_data::Item) -> Vec3 {
        let Some(player) = self.players.get(&id) else { return Vec3::ZERO };
        let main_hand_is = player.item_in_hand(crate::world::InteractionHand::MainHand).is_some_and(|stack| stack.is(item));
        let off_hand_is = player.item_in_hand(crate::world::InteractionHand::OffHand).is_some_and(|stack| stack.is(item));
        let item_only_in_offhand = off_hand_is && !main_hand_is;
        let main_arm_right = matches!(player.entity.data.get(ids::PLAYER_MAIN_HAND), DataValue::HumanoidArm(1));
        let item_arm_right = if item_only_in_offhand { !main_arm_right } else { main_arm_right };
        calculate_view_vector(0.0, player.entity.y_rot + if item_arm_right { 80.0 } else { -80.0 }).scale(0.5)
    }

    pub fn firework_rocket_tick(&mut self, id: EntityId) {
        self.projectile_base_tick(id);
        let hit_result = if let Some(target) = self.projectile(id).firework_attached_to_target() {
            if firework(self.projectile_mut(id)).attached_to_entity.is_none() && self.entity_ref(target as EntityId).is_some() && self.is_living(target as EntityId) {
                firework(self.projectile_mut(id)).attached_to_entity = Some(target as EntityId);
            }
            if let Some(attached) = firework(self.projectile_mut(id)).attached_to_entity.filter(|&attached| self.entity_ref(attached).is_some()) {
                let hand_angle = if self.entity(attached).is_fall_flying() {
                    let look_angle = self.entity(attached).look_angle();
                    let a = self.entity_mut(attached);
                    let movement = a.delta_movement;
                    a.set_delta_movement(movement.add(
                        look_angle.x * 0.1 + (look_angle.x * 1.5 - movement.x) * 0.5,
                        look_angle.y * 0.1 + (look_angle.y * 1.5 - movement.y) * 0.5,
                        look_angle.z * 0.1 + (look_angle.z * 1.5 - movement.z) * 0.5,
                    ));
                    self.get_hand_holding_item_angle(attached, items::FIREWORK_ROCKET)
                } else {
                    Vec3::ZERO
                };
                let a = self.entity(attached);
                let (position, movement) = (a.position().add_vec(hand_angle), a.delta_movement);
                let e = self.entity_mut(id);
                e.set_pos(position);
                e.set_delta_movement(movement);
                self.entity_moved(id);
            }
            self.get_hit_result_on_move_vector(id, |level, entity| level.can_hit_entity(id, entity))
        } else {
            if !self.projectile(id).is_shot_at_angle() {
                let e = self.entity_mut(id);
                let horizontal_acceleration = if e.horizontal_collision { 1.0 } else { 1.15 };
                e.set_delta_movement(e.delta_movement.multiply(horizontal_acceleration, 1.0, horizontal_acceleration).add(0.0, 0.04, 0.0));
            }
            let movement = self.entity(id).delta_movement;
            let hit_result = self.get_hit_result_on_move_vector(id, |level, entity| level.can_hit_entity(id, entity));
            self.move_entity(id, MoverType::SelfMove, movement);
            self.apply_effects_from_blocks(id);
            self.entity_mut(id).set_delta_movement(movement);
            hit_result
        };
        if !self.entity(id).no_physics && self.is_alive(id) && hit_result.get_type() != Type::Miss {
            self.hit_target_or_deflect_self(id, hit_result);
            self.entity_mut(id).needs_sync = true;
        }
        self.update_rotation(id);
        if firework(self.projectile_mut(id)).life == 0 && !self.entity(id).is_silent() {
            let e = self.entity(id);
            let (x, y, z) = (e.x(), e.y(), e.z());
            self.play_sound_at(None, x, y, z, sound_event("minecraft:entity.firework_rocket.launch"), SoundSource::Ambient, 3.0, 1.0);
        }
        let rocket = firework(self.projectile_mut(id));
        rocket.life += 1;
        if rocket.life > rocket.lifetime {
            self.firework_explode(id);
        }
    }

    fn firework_explode(&mut self, id: EntityId) {
        self.broadcast_entity_event(id, EVENT_FIREWORKS_EXPLODE);
        self.deal_explosion_damage(id);
        self.entity_mut(id).discard();
    }

    pub fn firework_rocket_on_hit_entity(&mut self, id: EntityId) {
        self.firework_explode(id);
    }

    pub fn firework_rocket_on_hit_block(&mut self, id: EntityId, hit_result: BlockHitResult) {
        let pos = hit_result.block_pos;
        let state = self.block_state(pos);
        let mut noop = InsideBlockEffects::default();
        block::of(state).entity_inside(state, self, pos, id, &mut noop, true);
        if self.projectile(id).get_explosions() > 0 {
            self.firework_explode(id);
        }
        self.projectile_base_on_hit_block(id, hit_result);
    }

    fn deal_explosion_damage(&mut self, id: EntityId) {
        let explosions = self.projectile(id).get_explosions();
        let damage_amount = if explosions > 0 { 5.0 + (explosions * 2) as f32 } else { 0.0 };
        if damage_amount <= 0.0 {
            return;
        }
        let attached_to_entity = firework(self.projectile_mut(id)).attached_to_entity;
        let source = DamageSource::source(damage_types::FIREWORKS, Some(id), self.projectile_get_owner(id));
        if let Some(attached) = attached_to_entity.filter(|&attached| self.entity_ref(attached).is_some()) {
            self.hurt_server(attached, source, 5.0 + (explosions * 2) as f32);
        }
        let rocket_pos = self.entity(id).position();
        let area = self.entity(id).bounding_box().inflate_all(5.0);
        let targets: Vec<EntityId> = self.get_entities(None, &area).into_iter().filter(|&entity| self.is_living(entity)).collect();
        for target in targets {
            if Some(target) == attached_to_entity || self.entity(id).distance_to_sqr(self.entity(target)) > 25.0 {
                continue;
            }
            let mut can_see = false;
            for test_step in 0..2 {
                let t = self.entity(target);
                let to = Vec3::new(t.x(), t.y() + t.dimensions.height as f64 * (0.5 * test_step as f64), t.z());
                let clip = self.clip(&ClipContext { from: rocket_pos, to, block: ClipBlock::Collider, fluid: ClipFluid::None, is_player: false });
                if clip.miss {
                    can_see = true;
                    break;
                }
            }
            if can_see {
                let distance = self.entity(id).distance_to(self.entity(target));
                let damage = damage_amount * ((5.0 - distance as f64) / 5.0).sqrt() as f32;
                let source = DamageSource::source(damage_types::FIREWORKS, Some(id), self.projectile_get_owner(id));
                self.hurt_server(target, source, damage);
            }
        }
    }
}
