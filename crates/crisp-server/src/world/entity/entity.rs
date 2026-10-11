use std::collections::VecDeque;
use std::sync::atomic::{AtomicI32, Ordering};

use crisp_core::hash_map::FastSet;
use crisp_core::random::{LegacyRandomSource, RandomSource};
use crisp_core::shapes::{BooleanOp, VoxelShape, join_is_not_empty};
use crisp_core::{AABB, Axis, AxisDirection, BlockPos, ChunkPos, Direction, Vec3, mth};
use crisp_data::{BlockState, block_tags, blocks, damage_type_tags, damage_types, entity_type_tags, fluid_tags, registry, registry_id};
use crisp_protocol::nbt::Nbt;

use super::attributes::Attribute;
use super::{EntityDimensions, ItemEntity, LevelEntity, data, relative};
use crate::world::damagesource::DamageSource;
use crate::world::item::ItemStack;
use crate::world::item::component as dc;
use crate::world::{InteractionHand, InteractionResult};
use super::data::{DataValue, Pose, SynchedEntityData, ids};
use super::fluid_interaction::{CurrentFluid, EntityFluidInteraction};
use super::inside_block::{InsideBlockEffectType, InsideBlockEffects, QueuedEffect};
use crate::server::level::game_mode::GameType;
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::attribute::environment_attributes;
use crate::world::level::BlockGetter;
use crate::world::level::block;
use crate::world::level::clip::{ClipBlock, ClipContext, ClipFluid};
use crate::world::level::clip::for_each_block_intersected_between;
use crate::world::level::collision::{Collider, collide_with_shapes};

pub type EntityId = i32;

pub fn next_entity_id() -> EntityId {
    static COUNTER: AtomicI32 = AtomicI32::new(0);
    COUNTER.fetch_add(1, Ordering::Relaxed) + 1
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RemovalReason {
    Killed,
    Discarded,
    UnloadedToChunk,
    UnloadedWithPlayer,
    ChangedDimension,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MoverType {
    SelfMove,
    Player,
    Piston,
    ShulkerBox,
    Shulker,
}

impl MoverType {
    pub fn is_server_and_client_simulated(self) -> bool {
        self != MoverType::SelfMove
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MovementEmission {
    None,
    All,
}

impl MovementEmission {
    pub fn emits_anything(self) -> bool {
        self != MovementEmission::None
    }

    pub fn emits_sounds(self) -> bool {
        self == MovementEmission::All
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Movement {
    pub from: Vec3,
    pub to: Vec3,
    pub axis_dependent_original_movement: Option<Vec3>,
}

pub mod flags {
    pub const ON_FIRE: u8 = 0;
    pub const SHIFT_KEY_DOWN: u8 = 1;
    pub const SPRINTING: u8 = 3;
    pub const SWIMMING: u8 = 4;
    pub const INVISIBLE: u8 = 5;
    pub const GLOWING: u8 = 6;
    pub const FALL_FLYING: u8 = 7;
}

pub const MAX_ENTITY_TAG_COUNT: usize = 1024;
pub const TOTAL_AIR_SUPPLY: i32 = 300;

pub struct Entity {
    pub id: EntityId,
    pub uuid: u128,
    pub entity_type: i32,
    pub random: LegacyRandomSource,
    pos: Vec3,
    bb: AABB,
    pub delta_movement: Vec3,
    pub y_rot: f32,
    pub x_rot: f32,
    pub y_head_rot: f32,
    pub dimensions: EntityDimensions,
    pub data: SynchedEntityData,
    pub on_ground: bool,
    pub horizontal_collision: bool,
    pub requires_precise_position: bool,
    pub vertical_collision: bool,
    pub vertical_collision_below: bool,
    pub minor_horizontal_collision: bool,
    pub tick_count: i32,
    pub removal: Option<RemovalReason>,
    pub needs_sync: bool,
    pub sync_position: bool,
    pub sync_velocity: bool,
    pub xo: Vec3,
    pub old_pos: Vec3,
    pub stuck_speed_multiplier: Vec3,
    pub move_dist: f32,
    pub fly_dist: f32,
    pub fall_distance: f64,
    pub next_step: f32,
    pub no_physics: bool,
    pub first_tick: bool,
    pub remaining_fire_ticks: i32,
    pub invulnerable: bool,
    pub invulnerable_time: i32,
    pub portal_cooldown: i32,
    pub has_glowing_tag: bool,
    pub has_visual_fire: bool,
    pub tags: Vec<String>,
    pub custom_data: Option<Nbt>,
    pub was_touching_water: bool,
    pub was_eye_in_water: bool,
    pub is_in_powder_snow: bool,
    pub was_in_powder_snow: bool,
    pub fluid_interaction: EntityFluidInteraction,
    pub main_supporting_block_pos: Option<BlockPos>,
    pub on_ground_no_blocks: bool,
    pub crystal_sound_intensity: f32,
    pub last_crystal_sound_play_tick: i32,
    pub last_known_speed: Vec3,
    pub last_known_position: Option<Vec3>,
    pub in_block_state: Option<BlockState>,
    pub movement_this_tick: VecDeque<Movement>,
    pub final_movements_this_tick: Vec<Movement>,
    pub inside_effects: InsideBlockEffects,
}

impl Entity {
    pub fn new(entity_type: i32, dimensions: EntityDimensions, data: Vec<DataValue>) -> Entity {
        let mut random = LegacyRandomSource::create();
        let uuid = create_insecure_uuid(&mut random);
        Entity {
            id: next_entity_id(),
            uuid,
            entity_type,
            random,
            pos: Vec3::ZERO,
            bb: dimensions.make_bounding_box(Vec3::ZERO),
            delta_movement: Vec3::ZERO,
            y_rot: 0.0,
            x_rot: 0.0,
            y_head_rot: 0.0,
            dimensions,
            data: SynchedEntityData::new(data),
            on_ground: false,
            horizontal_collision: false,
            vertical_collision: false,
            vertical_collision_below: false,
            minor_horizontal_collision: false,
            tick_count: 0,
            removal: None,
            needs_sync: false,
            requires_precise_position: false,
            sync_position: false,
            sync_velocity: false,
            xo: Vec3::ZERO,
            old_pos: Vec3::ZERO,
            stuck_speed_multiplier: Vec3::ZERO,
            move_dist: 0.0,
            fly_dist: 0.0,
            fall_distance: 0.0,
            next_step: 1.0,
            no_physics: false,
            first_tick: true,
            remaining_fire_ticks: 0,
            invulnerable: false,
            invulnerable_time: 0,
            portal_cooldown: 0,
            has_glowing_tag: false,
            has_visual_fire: false,
            tags: Vec::new(),
            custom_data: None,
            was_touching_water: false,
            was_eye_in_water: false,
            is_in_powder_snow: false,
            was_in_powder_snow: false,
            fluid_interaction: EntityFluidInteraction::default(),
            main_supporting_block_pos: None,
            on_ground_no_blocks: false,
            crystal_sound_intensity: 0.0,
            last_crystal_sound_play_tick: 0,
            last_known_speed: Vec3::ZERO,
            last_known_position: None,
            in_block_state: None,
            movement_this_tick: VecDeque::new(),
            final_movements_this_tick: Vec::new(),
            inside_effects: InsideBlockEffects::default(),
        }
    }

    pub fn position(&self) -> Vec3 {
        self.pos
    }

    pub fn x(&self) -> f64 {
        self.pos.x
    }

    pub fn y(&self) -> f64 {
        self.pos.y
    }

    pub fn z(&self) -> f64 {
        self.pos.z
    }

    pub fn set_pos(&mut self, pos: Vec3) {
        self.set_pos_raw(pos);
        self.bb = self.dimensions.make_bounding_box(pos);
    }

    pub fn set_pos_raw(&mut self, pos: Vec3) {
        if self.pos != pos {
            if BlockPos::containing_vec(pos) != BlockPos::containing_vec(self.pos) {
                self.in_block_state = None;
            }
            self.pos = pos;
        }
    }

    pub fn reapply_position(&mut self) {
        self.last_known_position = None;
        self.set_pos(self.pos);
    }

    pub fn bounding_box(&self) -> AABB {
        self.bb
    }

    pub fn set_bounding_box(&mut self, bb: AABB) {
        self.bb = bb;
    }

    pub fn block_position(&self) -> BlockPos {
        BlockPos::containing_vec(self.pos)
    }

    pub fn chunk_position(&self) -> ChunkPos {
        ChunkPos::new(mth::floor(self.pos.x) >> 4, mth::floor(self.pos.z) >> 4)
    }

    pub fn eye_y(&self) -> f64 {
        self.pos.y + self.dimensions.eye_height as f64
    }

    pub fn eye_position(&self) -> Vec3 {
        Vec3::new(self.pos.x, self.eye_y(), self.pos.z)
    }

    pub fn add_tag(&mut self, tag: &str) -> bool {
        if self.tags.len() >= MAX_ENTITY_TAG_COUNT || self.tags.iter().any(|t| t == tag) {
            return false;
        }
        self.tags.push(tag.to_string());
        true
    }

    pub fn remove_tag(&mut self, tag: &str) -> bool {
        let Some(i) = self.tags.iter().position(|t| t == tag) else { return false };
        self.tags.remove(i);
        true
    }

    pub fn is_removed(&self) -> bool {
        self.removal.is_some()
    }

    pub fn random_x(&mut self, spread: f64) -> f64 {
        self.pos.x + self.dimensions.width as f64 * ((2.0 * self.random.next_double() - 1.0) * spread)
    }

    pub fn random_y(&mut self) -> f64 {
        self.pos.y + self.dimensions.height as f64 * self.random.next_double()
    }

    pub fn random_y_scaled(&mut self, spread: f64) -> f64 {
        self.pos.y + self.dimensions.height as f64 * ((2.0 * self.random.next_double() - 1.0) * spread)
    }

    pub fn random_z(&mut self, spread: f64) -> f64 {
        self.pos.z + self.dimensions.width as f64 * ((2.0 * self.random.next_double() - 1.0) * spread)
    }

    pub fn custom_name(&self) -> Option<crate::network::chat::Component> {
        match self.data.get(data::ids::CUSTOM_NAME) {
            DataValue::OptionalComponent(Some(name)) => Some(crate::network::chat::Component::from_nbt(name)),
            _ => None,
        }
    }

    pub fn set_custom_name(&mut self, name: Option<Nbt>) {
        self.data.set(data::ids::CUSTOM_NAME, DataValue::OptionalComponent(name));
    }

    pub fn apply_components_from_item_stack(&mut self, stack: &ItemStack) {
        for ty in [dc::CUSTOM_NAME, dc::CUSTOM_DATA] {
            if let Some(value) = stack.get(ty) {
                self.apply_implicit_component(ty, value);
            }
        }
    }

    pub fn apply_implicit_component(&mut self, ty: dc::DataComponentType, value: &Nbt) -> bool {
        if ty == dc::CUSTOM_NAME {
            self.set_custom_name(Some(value.clone()));
            true
        } else if ty == dc::CUSTOM_DATA {
            self.custom_data = Some(value.clone());
            true
        } else {
            false
        }
    }

    pub fn discard(&mut self) {
        self.removal = Some(RemovalReason::Discarded);
    }

    pub fn view_vector(&self) -> Vec3 {
        Vec3::direction_from_rotation(self.x_rot, self.y_rot)
    }

    pub fn look_angle(&self) -> Vec3 {
        let x_rot = self.x_rot * mth::DEG_TO_RAD;
        let y_rot = -self.y_rot * mth::DEG_TO_RAD;
        let (y_cos, y_sin) = (mth::cos(y_rot as f64), mth::sin(y_rot as f64));
        let (x_cos, x_sin) = (mth::cos(x_rot as f64), mth::sin(x_rot as f64));
        Vec3::new((y_sin * x_cos) as f64, -x_sin as f64, (y_cos * x_cos) as f64)
    }

    pub fn set_delta_movement(&mut self, movement: Vec3) {
        if movement.is_finite() {
            self.delta_movement = movement;
        }
    }

    pub fn add_delta_movement(&mut self, momentum: Vec3) {
        if momentum.is_finite() {
            self.set_delta_movement(self.delta_movement.add_vec(momentum));
        }
    }

    pub fn set_old_pos(&mut self) {
        self.xo = self.pos;
        self.old_pos = self.pos;
    }

    pub fn old_position(&self) -> Vec3 {
        self.old_pos
    }

    pub fn push(&mut self, xa: f64, ya: f64, za: f64) {
        if xa.is_finite() && ya.is_finite() && za.is_finite() {
            self.set_delta_movement(self.delta_movement.add(xa, ya, za));
            self.needs_sync = true;
        }
    }

    pub fn snap_to(&mut self, x: f64, y: f64, z: f64, y_rot: f32, x_rot: f32) {
        self.set_pos_raw(Vec3::new(x, y, z));
        self.set_y_rot(y_rot);
        self.set_x_rot(x_rot);
        self.set_old_pos();
        self.reapply_position();
    }

    pub fn abs_snap_to(&mut self, x: f64, y: f64, z: f64) {
        let cx = mth::clamp(x, -3.0e7, 3.0e7);
        let cz = mth::clamp(z, -3.0e7, 3.0e7);
        self.xo = Vec3::new(cx, y, cz);
        self.set_pos(Vec3::new(cx, y, cz));
    }

    pub fn teleport_set_position(&mut self, current: relative::PositionMoveRotation, destination: relative::PositionMoveRotation, relatives: u16) {
        let absolute = relative::PositionMoveRotation::calculate_absolute(current, destination, relatives);
        self.set_pos_raw(absolute.position);
        self.set_y_rot(absolute.y_rot);
        self.set_x_rot(absolute.x_rot);
        self.reapply_position();
        self.set_old_pos();
        self.set_delta_movement(absolute.delta_movement);
        self.clear_movement_this_tick();
    }

    pub fn abs_snap_rotation_to(&mut self, y_rot: f32, x_rot: f32) {
        self.set_y_rot(y_rot % 360.0);
        self.set_x_rot(mth::clamp_f(x_rot, -90.0, 90.0) % 360.0);
    }

    pub fn set_y_rot(&mut self, y_rot: f32) {
        if y_rot.is_finite() {
            self.y_rot = y_rot;
        }
    }

    pub fn set_x_rot(&mut self, x_rot: f32) {
        if x_rot.is_finite() {
            self.x_rot = (x_rot % 360.0).clamp(-90.0, 90.0);
        }
    }

    fn shared_flags(&self) -> i8 {
        match self.data.get(ids::SHARED_FLAGS) {
            DataValue::Byte(b) => *b,
            _ => 0,
        }
    }

    pub fn shared_flag(&self, flag: u8) -> bool {
        self.shared_flags() & (1 << flag) != 0
    }

    pub fn set_shared_flag(&mut self, flag: u8, value: bool) {
        let current = self.shared_flags();
        let new = if value { current | 1 << flag } else { current & !(1 << flag) };
        self.data.set(ids::SHARED_FLAGS, DataValue::Byte(new));
    }

    pub fn is_shift_key_down(&self) -> bool {
        self.shared_flag(flags::SHIFT_KEY_DOWN)
    }

    pub fn set_shift_key_down(&mut self, value: bool) {
        self.set_shared_flag(flags::SHIFT_KEY_DOWN, value);
    }

    pub fn is_suppressing_bounce(&self) -> bool {
        self.is_shift_key_down()
    }

    pub fn is_discrete(&self) -> bool {
        self.is_shift_key_down()
    }

    pub fn distance_to_sqr_xyz(&self, x: f64, y: f64, z: f64) -> f64 {
        let xd = self.x() - x;
        let yd = self.y() - y;
        let zd = self.z() - z;
        xd * xd + yd * yd + zd * zd
    }

    pub fn distance_to_sqr(&self, entity: &Entity) -> f64 {
        self.position().distance_to_sqr(entity.position())
    }

    pub fn is_sprinting(&self) -> bool {
        self.shared_flag(flags::SPRINTING)
    }

    pub fn is_swimming(&self) -> bool {
        self.shared_flag(flags::SWIMMING)
    }

    pub fn set_swimming(&mut self, value: bool) {
        self.set_shared_flag(flags::SWIMMING, value);
    }

    pub fn is_fall_flying(&self) -> bool {
        self.shared_flag(flags::FALL_FLYING)
    }

    pub fn pose(&self) -> Pose {
        match self.data.get(ids::POSE) {
            DataValue::Pose(p) => *p,
            _ => Pose::Standing,
        }
    }

    pub fn set_pose(&mut self, pose: Pose) {
        self.data.set(ids::POSE, DataValue::Pose(pose));
    }

    pub fn is_crouching(&self) -> bool {
        self.pose() == Pose::Crouching
    }

    pub fn is_silent(&self) -> bool {
        matches!(self.data.get(ids::SILENT), DataValue::Boolean(true))
    }

    pub fn is_no_gravity(&self) -> bool {
        matches!(self.data.get(ids::NO_GRAVITY), DataValue::Boolean(true))
    }

    pub fn ticks_frozen(&self) -> i32 {
        match self.data.get(ids::TICKS_FROZEN) {
            DataValue::Int(v) => *v,
            _ => 0,
        }
    }

    pub fn set_ticks_frozen(&mut self, ticks: i32) {
        self.data.set(ids::TICKS_FROZEN, DataValue::Int(ticks));
    }

    pub fn air_supply(&self) -> i32 {
        match self.data.get(ids::AIR_SUPPLY) {
            DataValue::Int(v) => *v,
            _ => 300,
        }
    }

    pub fn set_air_supply(&mut self, supply: i32) {
        self.data.set(ids::AIR_SUPPLY, DataValue::Int(supply));
    }

    pub fn is_freezing(&self) -> bool {
        self.ticks_frozen() > 0
    }

    pub fn is_in_water(&self) -> bool {
        self.was_touching_water
    }

    pub fn is_in_lava(&self) -> bool {
        !self.first_tick && self.fluid_interaction.is_in_fluid(&crisp_data::fluid_tags::LAVA)
    }

    pub fn is_under_water(&self) -> bool {
        self.was_eye_in_water && self.is_in_water()
    }

    pub fn is_in_liquid(&self) -> bool {
        self.is_in_water() || self.is_in_lava()
    }

    pub fn fluid_height(&self, fluid: &crisp_data::Tag) -> f64 {
        self.fluid_interaction.fluid_height(fluid)
    }

    pub fn is_eye_in_fluid(&self, fluid: &crisp_data::Tag) -> bool {
        self.fluid_interaction.is_eye_in_fluid(fluid)
    }

    pub fn fluid_jump_threshold(&self) -> f64 {
        if (self.dimensions.eye_height as f64) < 0.4 { 0.0 } else { 0.4 }
    }

    pub fn reset_fall_distance(&mut self) {
        self.fall_distance = 0.0;
    }

    pub fn check_fall_distance_accumulation(&mut self) {
        if self.delta_movement.y > -0.5 && self.fall_distance > 1.0 {
            self.fall_distance = 1.0;
        }
    }

    pub fn compute_speed(&mut self) {
        let last = *self.last_known_position.get_or_insert(self.pos);
        self.last_known_speed = self.pos.subtract(last);
        self.last_known_position = Some(self.pos);
    }

    pub fn has_moved_horizontally_recently(&self) -> bool {
        self.last_known_speed.horizontal_distance().abs() > 1.0e-5f32 as f64
    }

    pub fn add_movement_this_tick(&mut self, movement: Movement) {
        if self.movement_this_tick.len() >= 100 {
            let first = self.movement_this_tick.pop_front().expect("movement");
            let second = self.movement_this_tick.pop_front().expect("movement");
            self.movement_this_tick.push_front(Movement { from: first.from, to: second.to, axis_dependent_original_movement: None });
        }
        self.movement_this_tick.push_back(movement);
    }

    pub fn remove_latest_movement_recording(&mut self) {
        self.movement_this_tick.pop_back();
    }

    pub fn clear_movement_this_tick(&mut self) {
        self.movement_this_tick.clear();
    }

    fn on_pos_below(&self, offset: f32) -> BlockPos {
        BlockPos::new(mth::floor(self.pos.x), mth::floor(self.pos.y - offset as f64), mth::floor(self.pos.z))
    }

    pub fn move_relative(&mut self, speed: f32, input: Vec3) {
        let delta = input_vector(input, speed, self.y_rot);
        self.set_delta_movement(self.delta_movement.add_vec(delta));
    }

    pub fn clear_fire(&mut self) {
        self.remaining_fire_ticks = self.remaining_fire_ticks.min(0);
    }

    pub fn ignite_for_ticks(&mut self, ticks: i32) {
        if self.remaining_fire_ticks < ticks {
            self.remaining_fire_ticks = ticks;
        }
        self.set_ticks_frozen(0);
    }
}

pub fn input_vector(input: Vec3, speed: f32, y_rot: f32) -> Vec3 {
    let length = input.length_sqr();
    if length < 1.0e-7 {
        return Vec3::ZERO;
    }
    let movement = if length > 1.0 { input.normalize() } else { input }.scale(speed as f64);
    let sin = mth::sin((y_rot * mth::DEG_TO_RAD) as f64) as f64;
    let cos = mth::cos((y_rot * mth::DEG_TO_RAD) as f64) as f64;
    Vec3::new(movement.x * cos - movement.z * sin, movement.y, movement.z * cos + movement.x * sin)
}

pub fn create_insecure_uuid(random: &mut impl RandomSource) -> u128 {
    let most = random.next_long() & -61441 | 16384;
    let least = random.next_long() & 4611686018427387903 | i64::MIN;
    (most as u64 as u128) << 64 | least as u64 as u128
}

const ON_POS_LEGACY: f32 = 0.2;
const BELOW_AFFECTS_MOVEMENT: f32 = 0.500001;
const ITEM_BELOW_AFFECTS_MOVEMENT: f32 = 0.999999;
const ON_POS: f32 = 1.0e-5;

pub fn sound_event(name: &str) -> i32 {
    registry_id(registry::SOUND_EVENT, name)
}

pub fn compute_modified_friction(friction: f32, modifier: f32) -> f32 {
    mth::clamp_f(1.0 - (1.0 - friction) * modifier, 0.0, 1.0)
}

impl ServerLevel {
    pub fn entity(&self, id: EntityId) -> &Entity {
        self.entity_ref(id).expect("entity in this level")
    }

    pub fn entity_mut(&mut self, id: EntityId) -> &mut Entity {
        match self.players.get_mut(&id) {
            Some(p) => &mut p.entity,
            None => self.entities.get_mut(&id).expect("entity in this level").entity_mut(),
        }
    }

    pub fn is_player(&self, id: EntityId) -> bool {
        self.players.contains_key(&id)
    }

    pub fn force_set_rotation(&mut self, id: EntityId, y_rot: f32, relative_y: bool, x_rot: f32, relative_x: bool) {
        let living = self.is_living(id);
        let e = self.entity_mut(id);
        let absolute_y = if relative_y { e.y_rot } else { 0.0 } + y_rot;
        let absolute_x = mth::clamp_f(if relative_x { e.x_rot } else { 0.0 } + x_rot, -90.0, 90.0);
        e.set_y_rot(absolute_y);
        if living {
            e.y_head_rot = absolute_y;
        }
        e.set_x_rot(absolute_x);
        if let Some(player) = self.players.get(&id) {
            player.connection.send(crisp_data::packets::play::clientbound::PLAYER_ROTATION, |w| {
                w.f32(y_rot);
                w.bool(relative_y);
                w.f32(x_rot);
                w.bool(relative_x);
            });
        }
    }

    pub fn attribute_value(&self, id: EntityId, attribute: Attribute) -> f64 {
        self.living(id).attributes.value(attribute)
    }

    pub fn is_spectator(&self, id: EntityId) -> bool {
        self.players.get(&id).is_some_and(|p| p.game_mode.game_type == GameType::Spectator)
    }

    pub fn vehicle(&self, _id: EntityId) -> Option<EntityId> {
        None
    }

    pub fn passengers(&self, _id: EntityId) -> Vec<EntityId> {
        Vec::new()
    }

    pub fn known_movement(&self, id: EntityId) -> Vec3 {
        match self.players.get(&id) {
            Some(player) => player.known_movement,
            None => self.entity(id).delta_movement,
        }
    }

    pub fn fishing_hook_open_water(&self, _id: EntityId) -> Option<bool> {
        None
    }

    pub fn is_alive(&self, id: EntityId) -> bool {
        match self.players.get(&id) {
            Some(p) => !p.entity.is_removed() && p.health > 0.0,
            None => match self.mob(id) {
                Some(mob) => !mob.entity.is_removed() && mob.health > 0.0,
                None => self.entity_ref(id).is_some_and(|e| !e.is_removed()),
            },
        }
    }

    pub fn fire_immune(&self, id: EntityId) -> bool {
        match self.entities.get(&id) {
            Some(super::LevelEntity::Item(item)) => !item.item.can_be_hurt_by(&crate::world::damagesource::DamageSource::new(crisp_data::damage_types::IN_FIRE)),
            Some(super::LevelEntity::PrimedTnt(_)) => true,
            _ => false,
        }
    }

    pub fn is_on_fire(&self, id: EntityId) -> bool {
        !self.fire_immune(id) && self.entity(id).remaining_fire_ticks > 0
    }

    fn fire_immune_ticks(&self, id: EntityId) -> i32 {
        if self.is_player(id) { 20 } else { 0 }
    }

    pub fn ignite_for_seconds(&mut self, id: EntityId, number_of_seconds: f32) {
        self.ignite_for_ticks(id, mth::floor_f(number_of_seconds * 20.0));
    }

    pub fn ignite_for_ticks(&mut self, id: EntityId, ticks: i32) {
        let ticks = if self.is_living(id) { mth::ceil(ticks as f64 * self.attribute_value(id, Attribute::BurningTime)) } else { ticks };
        if self.entity(id).remaining_fire_ticks < ticks {
            self.set_remaining_fire_ticks(id, ticks);
        }
        self.entity_mut(id).set_ticks_frozen(0);
    }

    pub fn max_up_step(&self, id: EntityId) -> f32 {
        if self.is_living(id) { self.attribute_value(id, Attribute::StepHeight) as f32 } else { 0.0 }
    }

    fn default_gravity(&self, id: EntityId) -> f64 {
        match self.entities.get(&id) {
            None | Some(super::LevelEntity::Mob(_)) => self.attribute_value(id, Attribute::Gravity),
            Some(super::LevelEntity::ExperienceOrb(_)) => 0.03,
            Some(_) => 0.04,
        }
    }

    pub fn gravity(&self, id: EntityId) -> f64 {
        if self.entity(id).is_no_gravity() { 0.0 } else { self.default_gravity(id) }
    }

    pub fn effective_gravity(&self, id: EntityId) -> f64 {
        let is_falling = self.entity(id).delta_movement.y <= 0.0;
        if is_falling && self.is_living(id) && self.has_effect(id, crisp_data::mob_effects::SLOW_FALLING) { self.gravity(id).min(0.01) } else { self.gravity(id) }
    }

    pub fn apply_gravity(&mut self, id: EntityId) {
        let gravity = self.gravity(id);
        if gravity != 0.0 {
            let e = self.entity_mut(id);
            e.set_delta_movement(e.delta_movement.add(0.0, -gravity, 0.0));
        }
    }

    pub fn air_drag(&self, id: EntityId) -> f32 {
        if self.is_living(id) { compute_modified_friction(0.98, self.attribute_value(id, Attribute::AirDragModifier) as f32) } else { 0.98 }
    }

    fn entity_bounciness(&self, id: EntityId) -> f64 {
        if self.is_living(id) { self.attribute_value(id, Attribute::Bounciness) } else { 0.0 }
    }

    fn movement_emission(&self, id: EntityId) -> MovementEmission {
        if self.is_living(id) { MovementEmission::All } else { MovementEmission::None }
    }

    pub fn is_pushed_by_fluid(&self, id: EntityId) -> bool {
        match self.players.get(&id) {
            Some(p) => !p.abilities_flying(),
            None => true,
        }
    }

    fn is_local_instance_authoritative(&self, id: EntityId) -> bool {
        !self.is_player(id)
    }

    pub fn sound_source(&self, id: EntityId) -> SoundSource {
        match self.entities.get(&id) {
            None => SoundSource::Players,
            Some(super::LevelEntity::ExperienceOrb(_)) => SoundSource::Ambient,
            Some(_) => SoundSource::Neutral,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn play_sound_at(&mut self, except: Option<EntityId>, x: f64, y: f64, z: f64, sound: i32, source: SoundSource, volume: f32, pitch: f32) {
        use crate::network::connection::encode;
        use crisp_data::packets::play::clientbound;
        let seed = self.thread_safe_random.next_long();
        let packet = encode(clientbound::SOUND, |p| {
            p.varint(sound + 1);
            p.varint(source as i32);
            p.i32((x * 8.0) as i32);
            p.i32((y * 8.0) as i32);
            p.i32((z * 8.0) as i32);
            p.f32(volume);
            p.f32(pitch);
            p.i64(seed);
        });
        let range = if volume > 1.0 { 16.0 * volume } else { 16.0 } as f64;
        self.broadcast_near(except, x, y, z, range, &packet);
    }

    pub fn entity_play_sound(&mut self, id: EntityId, sound: i32, volume: f32, pitch: f32) {
        let source = self.sound_source(id);
        let e = self.entity(id);
        let (x, y, z) = (e.x(), e.y(), e.z());
        if self.is_player(id) {
            self.play_sound_at(Some(id), x, y, z, sound, source, volume, pitch);
        } else if !e.is_silent() {
            self.play_sound_at(None, x, y, z, sound, source, volume, pitch);
        }
    }

    pub fn on_pos(&mut self, id: EntityId, offset: f32) -> BlockPos {
        let e = self.entity(id);
        let Some(on_pos) = e.main_supporting_block_pos else { return e.on_pos_below(offset) };
        if !(offset > 1.0e-5) {
            return on_pos;
        }
        let y = mth::floor(e.y() - offset as f64);
        let below = self.block_state(on_pos);
        if (!(offset as f64 <= 0.5) || !below.is(&block_tags::FENCES)) && !below.is(&block_tags::WALLS) && !below.block().is_instance_of("FenceGateBlock") {
            BlockPos::new(on_pos.x, y, on_pos.z)
        } else {
            on_pos
        }
    }

    pub fn on_pos_legacy(&mut self, id: EntityId) -> BlockPos {
        self.on_pos(id, ON_POS_LEGACY)
    }

    pub fn block_pos_below_that_affects_my_movement(&mut self, id: EntityId) -> BlockPos {
        let offset = if self.is_living(id) { BELOW_AFFECTS_MOVEMENT } else { ITEM_BELOW_AFFECTS_MOVEMENT };
        self.on_pos(id, offset)
    }

    pub fn set_on_ground(&mut self, id: EntityId, on_ground: bool) {
        self.entity_mut(id).on_ground = on_ground;
        self.check_supporting_block(id, on_ground, None);
    }

    pub fn set_on_ground_with_movement(&mut self, id: EntityId, on_ground: bool, horizontal_collision: bool, movement: Vec3) {
        let e = self.entity_mut(id);
        e.on_ground = on_ground;
        e.horizontal_collision = horizontal_collision;
        self.check_supporting_block(id, on_ground, Some(movement));
    }

    fn check_supporting_block(&mut self, id: EntityId, on_ground: bool, movement: Option<Vec3>) {
        let e = self.entity(id);
        if on_ground {
            let bb = e.bounding_box();
            let test_area = AABB::new(bb.min_x, bb.min_y - 1.0e-6, bb.min_z, bb.max_x, bb.min_y, bb.max_z);
            let position = e.position();
            let no_blocks = e.on_ground_no_blocks;
            let mut supporting = self.find_supporting_block(position, &test_area);
            if supporting.is_some() || no_blocks {
                self.entity_mut(id).main_supporting_block_pos = supporting;
            } else if let Some(movement) = movement {
                supporting = self.find_supporting_block(position, &test_area.move_by(-movement.x, 0.0, -movement.z));
                self.entity_mut(id).main_supporting_block_pos = supporting;
            }
            self.entity_mut(id).on_ground_no_blocks = supporting.is_none();
        } else {
            let e = self.entity_mut(id);
            e.on_ground_no_blocks = false;
            e.main_supporting_block_pos = None;
        }
    }

    pub fn block_speed_factor(&mut self, id: EntityId) -> f32 {
        let state = self.block_state(self.entity(id).block_position());
        let here = state.block().info().speed_factor;
        let factor = if !state.is_block(blocks::WATER) && !state.is_block(blocks::BUBBLE_COLUMN) {
            if here == 1.0 {
                let below = self.block_pos_below_that_affects_my_movement(id);
                self.block_state(below).block().info().speed_factor
            } else {
                here
            }
        } else {
            here
        };
        if self.is_living(id) { mth::lerp_f(self.attribute_value(id, Attribute::MovementEfficiency) as f32, factor, 1.0) } else { factor }
    }

    pub fn block_jump_factor(&mut self, id: EntityId) -> f32 {
        let here = self.block_state(self.entity(id).block_position()).block().info().jump_factor;
        let below_pos = self.block_pos_below_that_affects_my_movement(id);
        let below = self.block_state(below_pos).block().info().jump_factor;
        if here == 1.0 { below } else { here }
    }

    pub fn in_block_state(&mut self, id: EntityId) -> BlockState {
        if let Some(state) = self.entity(id).in_block_state {
            return state;
        }
        let state = self.block_state(self.entity(id).block_position());
        self.entity_mut(id).in_block_state = Some(state);
        state
    }

    pub fn move_entity(&mut self, id: EntityId, mover: MoverType, delta: Vec3) {
        let e = self.entity_mut(id);
        if e.no_physics {
            let to = e.position().add_vec(delta);
            e.set_pos(to);
            self.entity_moved(id);
            self.record_movement(id, mover, delta);
            let e = self.entity_mut(id);
            e.horizontal_collision = false;
            e.vertical_collision = false;
            e.vertical_collision_below = false;
            e.minor_horizontal_collision = false;
            return;
        }
        let mut delta = delta;
        if e.stuck_speed_multiplier.length_sqr() > 1.0e-7 {
            if mover != MoverType::Piston {
                let m = e.stuck_speed_multiplier;
                delta = delta.multiply(m.x, m.y, m.z);
            }
            e.stuck_speed_multiplier = Vec3::ZERO;
            e.set_delta_movement(Vec3::ZERO);
        }
        let delta = self.maybe_back_off_from_edge(id, delta, mover);
        let movement = self.collide(id, delta);
        let movement_length = movement.length_sqr();
        if movement_length > 1.0e-7 || delta.length_sqr() - movement_length < 1.0e-7 {
            let e = self.entity(id);
            if e.fall_distance != 0.0 && movement_length >= 1.0 {
                let check_distance = movement.length().min(8.0);
                let check_to = e.position().add_vec(movement.normalize().scale(check_distance));
                let ctx = ClipContext {
                    from: e.position(),
                    to: check_to,
                    block: ClipBlock::FallDamageResetting,
                    fluid: ClipFluid::Water,
                    is_player: self.is_player(id),
                };
                if !self.clip(&ctx).miss {
                    self.reset_fall_distance(id);
                }
            }
            let e = self.entity_mut(id);
            let pos = e.position();
            let new_position = pos.add_vec(movement);
            e.add_movement_this_tick(Movement { from: pos, to: new_position, axis_dependent_original_movement: Some(delta) });
            e.set_pos(new_position);
            self.entity_moved(id);
            self.record_movement(id, mover, movement);
        }
        let x_collision = !mth::equal(delta.x, movement.x);
        let z_collision = !mth::equal(delta.z, movement.z);
        let local = self.is_local_instance_authoritative(id);
        let e = self.entity_mut(id);
        e.horizontal_collision = x_collision || z_collision;
        let moved_vertically = delta.y.abs() > 0.0;
        if moved_vertically || local {
            e.vertical_collision = delta.y != movement.y;
            e.vertical_collision_below = e.vertical_collision && delta.y < 0.0;
            let (below, horizontal) = (e.vertical_collision_below, e.horizontal_collision);
            self.set_on_ground_with_movement(id, below, horizontal, movement);
        }
        let e = self.entity_mut(id);
        e.minor_horizontal_collision = false;
        let effect_pos = self.on_pos_legacy(id);
        let effect_state = self.block_state(effect_pos);
        if local {
            let on_ground = self.entity(id).on_ground;
            self.check_fall_damage(id, movement.y, on_ground, effect_state, effect_pos);
        }
        if self.entity_ref(id).is_none_or(Entity::is_removed) {
            return;
        }
        let e = self.entity(id);
        if moved_vertically && e.vertical_collision || e.horizontal_collision {
            self.restitute_movement_after_collisions(id, effect_state, x_collision, z_collision, movement);
        }
        let emission = self.movement_emission(id);
        if emission.emits_anything() {
            self.apply_movement_emission_and_play_sound(id, emission, movement, effect_pos, effect_state);
        }
        let factor = self.block_speed_factor(id) as f64;
        let e = self.entity_mut(id);
        e.set_delta_movement(e.delta_movement.multiply(factor, 1.0, factor));
    }

    fn record_movement(&mut self, id: EntityId, mover: MoverType, movement: Vec3) {
        if mover.is_server_and_client_simulated()
            && let Some(tracker) = self.chunk_map.tracked.get_mut(id).and_then(|t| t.server_entity.interpolation())
        {
            tracker.apply_predicted_movement(movement);
        }
    }

    fn maybe_back_off_from_edge(&mut self, id: EntityId, delta: Vec3, mover: MoverType) -> Vec3 {
        let Some(p) = self.players.get(&id) else { return delta };
        let max_down_step = self.max_up_step(id);
        let flying = p.abilities_flying();
        let shift = p.entity.is_shift_key_down();
        if flying || delta.y > 0.0 || !matches!(mover, MoverType::SelfMove | MoverType::Player) || !shift || !self.is_above_ground(id, max_down_step) {
            return delta;
        }
        let (mut dx, mut dz) = (delta.x, delta.z);
        let step = 0.05;
        let step_x = dx.signum_java() * step;
        let step_z = dz.signum_java() * step;
        while dx != 0.0 && self.can_fall_at_least(id, dx, 0.0, max_down_step as f64) {
            if dx.abs() <= step {
                dx = 0.0;
                break;
            }
            dx -= step_x;
        }
        while dz != 0.0 && self.can_fall_at_least(id, 0.0, dz, max_down_step as f64) {
            if dz.abs() <= step {
                dz = 0.0;
                break;
            }
            dz -= step_z;
        }
        while dx != 0.0 && dz != 0.0 && self.can_fall_at_least(id, dx, dz, max_down_step as f64) {
            if dx.abs() <= step {
                dx = 0.0;
            } else {
                dx -= step_x;
            }
            if dz.abs() <= step {
                dz = 0.0;
            } else {
                dz -= step_z;
            }
        }
        Vec3::new(dx, delta.y, dz)
    }

    fn is_above_ground(&self, id: EntityId, max_down_step: f32) -> bool {
        let e = self.entity(id);
        e.on_ground || e.fall_distance < max_down_step as f64 && !self.can_fall_at_least(id, 0.0, 0.0, max_down_step as f64 - e.fall_distance)
    }

    fn can_fall_at_least(&self, id: EntityId, dx: f64, dz: f64, min_height: f64) -> bool {
        let bb = self.entity(id).bounding_box();
        self.no_collision_for(id, &AABB::new(
            bb.min_x + 1.0e-7 + dx,
            bb.min_y - min_height - 1.0e-7,
            bb.min_z + 1.0e-7 + dz,
            bb.max_x - 1.0e-7 + dx,
            bb.min_y,
            bb.max_z - 1.0e-7 + dz,
        ))
    }

    pub(crate) fn colliders(&self, id: EntityId, box_: &AABB) -> Vec<Collider<'_>> {
        let border = self.border_collision(id, box_).map(|shape| Collider { shape, offset: [0.0; 3] });
        border.into_iter().chain(self.block_collisions(box_, false).into_iter().map(|(_, c)| c)).collect()
    }

    fn collide(&mut self, id: EntityId, movement: Vec3) -> Vec3 {
        let e = self.entity(id);
        let aabb = e.bounding_box();
        let on_ground = e.on_ground;
        let movement_step = if movement.length_sqr() == 0.0 { movement } else { collide_with_shapes(movement, &aabb, &self.colliders(id, &aabb.expand_towards_vec(movement))) };
        let x_collision = movement.x != movement_step.x;
        let y_collision = movement.y != movement_step.y;
        let z_collision = movement.z != movement_step.z;
        let on_ground_after_collision = y_collision && movement.y < 0.0;
        let max_up_step = self.max_up_step(id);
        if max_up_step > 0.0 && (on_ground_after_collision || on_ground) && (x_collision || z_collision) {
            let grounded = if on_ground_after_collision { aabb.move_by(0.0, movement_step.y, 0.0) } else { aabb };
            let mut step_up_aabb = grounded.expand_towards(movement.x, max_up_step as f64, movement.z);
            if !on_ground_after_collision {
                step_up_aabb = step_up_aabb.expand_towards(0.0, -1.0e-5f32 as f64, 0.0);
            }
            let colliders = self.colliders(id, &step_up_aabb);
            let step_height_to_skip = movement_step.y as f32;
            for candidate in collect_candidate_step_up_heights(&grounded, &colliders, max_up_step, step_height_to_skip) {
                let step_from_ground = collide_with_shapes(Vec3::new(movement.x, candidate as f64, movement.z), &grounded, &colliders);
                if step_from_ground.horizontal_distance_sqr() > movement_step.horizontal_distance_sqr() {
                    let distance_to_ground = aabb.min_y - grounded.min_y;
                    return step_from_ground.subtract(Vec3::new(0.0, distance_to_ground, 0.0));
                }
            }
        }
        movement_step
    }

    pub fn reset_fall_distance(&mut self, id: EntityId) {
        if let Some(p) = self.players.get_mut(&id) {
            p.starting_to_fall_position = None;
        }
        self.entity_mut(id).reset_fall_distance();
    }

    pub fn check_fall_damage(&mut self, id: EntityId, ya: f64, on_ground: bool, on_state: BlockState, pos: BlockPos) {
        let living = self.is_living(id);
        if living && !self.entity(id).is_in_water() {
            self.update_fluid_interaction(id);
        }
        if living && on_ground && self.entity(id).fall_distance > 0.0 {
            let fall_power = self.entity(id).fall_distance + 1.0e-6 - self.attribute_value(id, Attribute::SafeFallDistance);
            let power = mth::floor(fall_power).max(0) as f64;
            if power > 0.0 && !on_state.is_air() {
                let e = self.entity(id);
                let (mut x, y, mut z) = (e.x(), e.y(), e.z());
                let entity_pos = e.block_position();
                if pos.x != entity_pos.x || pos.z != entity_pos.z {
                    let x_diff = x - pos.x as f64 - 0.5;
                    let z_diff = z - pos.z as f64 - 0.5;
                    let max_diff = x_diff.abs().max(z_diff.abs());
                    x = pos.x as f64 + 0.5 + x_diff / max_diff * 0.5;
                    z = pos.z as f64 + 0.5 + z_diff / max_diff * 0.5;
                }
                let scale = (0.2f32 as f64 + power / 15.0).min(2.5);
                let particles = (150.0 * scale) as i32;
                self.send_block_particles(on_state, x, y, z, particles, (0.0, 0.0, 0.0), 0.15f32 as f64);
            }
        }
        let e = self.entity_mut(id);
        if !e.is_in_water() && ya < 0.0 {
            e.fall_distance -= (ya as f32) as f64;
        }
        if on_ground {
            let fall_distance = e.fall_distance;
            if fall_distance > 0.0 {
                block::of(on_state).fall_on(self, on_state, pos, id, fall_distance);
            }
            if self.entity_ref(id).is_some() {
                self.reset_fall_distance(id);
            }
        }
        if living && on_ground && let Some(p) = self.players.get_mut(&id) {
            p.living.last_climbable_pos = None;
        }
    }

    pub fn do_check_fall_damage(&mut self, id: EntityId, xa: f64, ya: f64, za: f64, on_ground: bool) {
        if self.touching_unloaded_chunk(id) {
            return;
        }
        self.check_supporting_block(id, on_ground, Some(Vec3::new(xa, ya, za)));
        let pos = self.on_pos_legacy(id);
        let state = self.block_state(pos);
        self.check_fall_damage(id, ya, on_ground, state, pos);
    }

    pub fn touching_unloaded_chunk(&self, id: EntityId) -> bool {
        let b = self.entity(id).bounding_box().inflate_all(1.0);
        !self.has_chunks_at(mth::floor(b.min_x), mth::floor(b.min_z), mth::ceil(b.max_x), mth::ceil(b.max_z))
    }

    pub fn cause_fall_damage(&mut self, id: EntityId, fall_distance: f64, damage_modifier: f32, damage_source: crate::world::damagesource::DamageSource) -> bool {
        if entity_type_tags::FALL_DAMAGE_IMMUNE.contains(self.entity(id).entity_type as usize) {
            return false;
        }
        let Some(p) = self.players.get_mut(&id) else { return false };
        if p.abilities_may_fly() {
            return false;
        }
        let effective = match p.living.current_impulse_impact_pos {
            Some(impact) => {
                let effective = fall_distance.min(impact.y - p.entity.y());
                if effective <= 0.0 {
                    p.living.reset_current_impulse_context();
                } else {
                    p.living.try_reset_current_impulse_context();
                }
                effective
            }
            None => fall_distance,
        };
        let damage = self.calculate_fall_damage(id, effective, damage_modifier);
        if damage > 0 {
            self.living_mut(id).reset_current_impulse_context();
            let sound = match (self.is_player(id), damage > 4) {
                (true, true) => "minecraft:entity.player.big_fall",
                (true, false) => "minecraft:entity.player.small_fall",
                (false, true) => "minecraft:entity.generic.big_fall",
                (false, false) => "minecraft:entity.generic.small_fall",
            };
            self.entity_play_sound(id, sound_event(sound), 1.0, 1.0);
            self.play_block_fall_sound(id);
            self.hurt(id, damage_source, damage as f32);
            return true;
        }
        false
    }

    pub fn calculate_fall_damage(&self, id: EntityId, fall_distance: f64, damage_modifier: f32) -> i32 {
        if entity_type_tags::FALL_DAMAGE_IMMUNE.contains(self.entity(id).entity_type as usize) {
            return 0;
        }
        let base_damage = fall_distance + 1.0e-6 - self.attribute_value(id, Attribute::SafeFallDistance);
        mth::floor(base_damage * damage_modifier as f64 * self.attribute_value(id, Attribute::FallDamageMultiplier))
    }

    fn play_block_fall_sound(&mut self, id: EntityId) {
        let e = self.entity(id);
        if e.is_silent() {
            return;
        }
        let pos = BlockPos::new(mth::floor(e.x()), mth::floor(e.y() - 0.2f32 as f64), mth::floor(e.z()));
        let state = self.block_state(pos);
        if !state.is_air() {
            let sound = state.sound_type();
            self.entity_play_sound(id, sound.fall_sound, sound.volume * 0.5, sound.pitch * 0.75);
        }
    }

    fn restitute_movement_after_collisions(&mut self, id: EntityId, effect_state: BlockState, x_collision: bool, z_collision: bool, movement: Vec3) {
        let suppressing = self.entity(id).is_suppressing_bounce();
        let mut restitution = if suppressing { 0.0 } else { self.entity_bounciness(id) };
        let effective_gravity = self.effective_gravity(id);
        let air_drag = self.air_drag(id);
        let living = self.is_living(id);
        let e = self.entity(id);
        let current = e.delta_movement;
        let mut after = current;
        if x_collision {
            after = after.with(Axis::X, -current.x * restitution);
        }
        if z_collision {
            after = after.with(Axis::Z, -current.z * restitution);
        }
        let mut bounced = restitution > 0.0 && (x_collision || z_collision);
        if e.vertical_collision {
            if e.vertical_collision_below {
                restitution = if !(-current.y <= effective_gravity) && !suppressing && !effect_state.is(&block_tags::SUPPRESSES_BOUNCE) {
                    let mut block_bounciness = effect_state.block().info().bounce_restitution;
                    if !living {
                        block_bounciness *= 0.8;
                    }
                    mth::jmax(restitution, block_bounciness as f64)
                } else {
                    0.0
                };
            }
            let (gravity_compensation, effective_drag) = if restitution > 0.0 {
                let portion = movement.y / current.y;
                bounced = true;
                (portion * effective_gravity, mth::lerp(portion, 1.0, air_drag as f64))
            } else {
                (0.0, 1.0)
            };
            after = after.with(Axis::Y, (gravity_compensation - current.y) * effective_drag * restitution);
        }
        if bounced {
            let pos = self.block_pos_below_that_affects_my_movement(id);
            let fall_distance = self.entity(id).fall_distance;
            block::of(effect_state).bounce_on(self, effect_state, pos, id, fall_distance);
            if self.entity_ref(id).is_some() {
                self.entity_mut(id).sync_position = true;
            }
        }
        if self.entity_ref(id).is_some() {
            self.entity_mut(id).set_delta_movement(after);
        }
    }

    fn is_state_climbable(state: BlockState) -> bool {
        state.is(&block_tags::CLIMBABLE) || state.is_block(blocks::POWDER_SNOW)
    }

    fn apply_movement_emission_and_play_sound(&mut self, id: EntityId, emission: MovementEmission, movement: Vec3, effect_pos: BlockPos, effect_state: BlockState) {
        let moved_distance = (movement.length() * 0.6f32 as f64) as f32;
        let horizontal_moved_distance = (movement.horizontal_distance() * 0.6f32 as f64) as f32;
        let supporting_pos = self.on_pos(id, ON_POS);
        let supporting_state = self.block_state(supporting_pos);
        let climbing = Self::is_state_climbable(supporting_state);
        let e = self.entity_mut(id);
        e.move_dist += if climbing { moved_distance } else { horizontal_moved_distance };
        e.fly_dist += moved_distance;
        if e.move_dist > e.next_step && !supporting_state.is_air() {
            let only_effect_state = supporting_pos == effect_pos;
            let mut produced = self.vibration_and_sound_effects_from_block(id, effect_pos, effect_state, emission.emits_sounds(), movement);
            if !only_effect_state {
                produced |= self.vibration_and_sound_effects_from_block(id, supporting_pos, supporting_state, false, movement);
            }
            if produced {
                let e = self.entity_mut(id);
                e.next_step = (e.move_dist as i32 + 1) as f32;
            } else if self.entity(id).is_in_water() {
                let e = self.entity_mut(id);
                e.next_step = (e.move_dist as i32 + 1) as f32;
                if emission.emits_sounds() {
                    self.water_swim_sound(id);
                }
            }
        }
    }

    fn vibration_and_sound_effects_from_block(&mut self, id: EntityId, pos: BlockPos, state: BlockState, should_sound: bool, movement: Vec3) -> bool {
        if state.is_air() {
            return false;
        }
        let climbable = Self::is_state_climbable(state);
        let e = self.entity(id);
        if (e.on_ground || climbable || e.is_crouching() && movement.y == 0.0) && !e.is_swimming() {
            if should_sound {
                self.walking_step_sound(id, pos, state);
            }
            return true;
        }
        false
    }

    fn walking_step_sound(&mut self, id: EntityId, pos: BlockPos, state: BlockState) {
        self.play_step_sound(id, pos, state);
        let e = self.entity(id);
        if state.is(&block_tags::CRYSTAL_SOUND_BLOCKS) && e.tick_count >= e.last_crystal_sound_play_tick + 20 {
            let e = self.entity_mut(id);
            e.crystal_sound_intensity *= 0.997f64.powf((e.tick_count - e.last_crystal_sound_play_tick) as f64) as f32;
            e.crystal_sound_intensity = (e.crystal_sound_intensity + 0.07).min(1.0);
            let pitch = 0.5 + e.crystal_sound_intensity * e.random.next_float() * 1.2;
            let volume = 0.1 + e.crystal_sound_intensity * 1.2;
            self.entity_play_sound(id, sound_event("minecraft:block.amethyst_block.chime"), volume, pitch);
            let e = self.entity_mut(id);
            e.last_crystal_sound_play_tick = e.tick_count;
        }
    }

    fn play_step_sound(&mut self, id: EntityId, pos: BlockPos, state: BlockState) {
        if let Some(mob) = self.mob(id) {
            let sound = sound_event(&mob.sound_set().step_sound);
            self.entity_play_sound(id, sound, 0.15, 1.0);
            return;
        }
        if self.is_player(id) {
            if self.entity(id).is_in_water() {
                self.water_swim_sound(id);
                let sound = state.sound_type();
                self.entity_play_sound(id, sound.step_sound, sound.volume * 0.05, sound.pitch * 0.8);
                return;
            }
            let above = pos.above();
            let above_state = self.block_state(above);
            let primary = if !above_state.is(&block_tags::INSIDE_STEP_SOUND_BLOCKS) && !above_state.is(&block_tags::COMBINATION_STEP_SOUND_BLOCKS) { pos } else { above };
            if primary != pos {
                let primary_state = self.block_state(primary);
                if primary_state.is(&block_tags::COMBINATION_STEP_SOUND_BLOCKS) {
                    let primary_sound = primary_state.sound_type();
                    self.entity_play_sound(id, primary_sound.step_sound, primary_sound.volume * 0.15, primary_sound.pitch);
                    let secondary = state.sound_type();
                    self.entity_play_sound(id, secondary.step_sound, secondary.volume * 0.05, secondary.pitch * 0.8);
                    return;
                }
                let sound = primary_state.sound_type();
                self.entity_play_sound(id, sound.step_sound, sound.volume * 0.15, sound.pitch);
                return;
            }
        }
        let sound = state.sound_type();
        self.entity_play_sound(id, sound.step_sound, sound.volume * 0.15, sound.pitch);
    }

    fn water_swim_sound(&mut self, id: EntityId) {
        let e = self.entity_mut(id);
        let m = e.delta_movement;
        let speed = (((m.x * m.x * 0.2f32 as f64 + m.y * m.y + m.z * m.z * 0.2f32 as f64).sqrt() as f32) * 0.35).min(1.0);
        let pitch = 1.0 + (e.random.next_float() - e.random.next_float()) * 0.4;
        let sound = if self.is_player(id) { "minecraft:entity.player.swim" } else { "minecraft:entity.generic.swim" };
        self.entity_play_sound(id, sound_event(sound), speed, pitch);
    }

    pub fn apply_effects_from_blocks(&mut self, id: EntityId) {
        let e = self.entity_mut(id);
        let mut movements = std::mem::take(&mut e.final_movements_this_tick);
        movements.clear();
        movements.extend(e.movement_this_tick.drain(..));
        match movements.last() {
            None => movements.push(Movement { from: e.old_position(), to: e.position(), axis_dependent_original_movement: None }),
            Some(last) if last.to.distance_to_sqr(e.position()) > 9.9999994e-11f32 as f64 => {
                movements.push(Movement { from: last.to, to: e.position(), axis_dependent_original_movement: None });
            }
            Some(_) => {}
        }
        self.apply_effects_from_movements(id, &movements);
        self.restore_final_movements(id, movements);
    }

    pub fn apply_effects_from_blocks_for_last_movements(&mut self, id: EntityId) {
        let movements = std::mem::take(&mut self.entity_mut(id).final_movements_this_tick);
        self.apply_effects_from_movements(id, &movements);
        self.restore_final_movements(id, movements);
    }

    fn restore_final_movements(&mut self, id: EntityId, movements: Vec<Movement>) {
        if self.entity_ref(id).is_some() {
            self.entity_mut(id).final_movements_this_tick = movements;
        }
    }

    fn is_affected_by_blocks(&self, id: EntityId) -> bool {
        self.entity_ref(id).is_some_and(|e| !e.is_removed() && !e.no_physics)
    }

    fn apply_effects_from_movements(&mut self, id: EntityId, movements: &[Movement]) {
        if !self.is_affected_by_blocks(id) {
            return;
        }
        if self.entity(id).on_ground {
            let effect_pos = self.on_pos_legacy(id);
            let state = self.block_state(effect_pos);
            block::of(state).step_on(self, effect_pos, state, id);
            if self.entity_ref(id).is_none() {
                return;
            }
        }
        let was_on_fire = self.is_on_fire(id);
        let was_freezing = self.entity(id).is_freezing();
        let previous_fire_ticks = self.entity(id).remaining_fire_ticks;
        self.check_inside_blocks(id, movements);
        self.apply_inside_effects(id);
        if self.entity_ref(id).is_none() {
            return;
        }
        if was_on_fire && !self.is_on_fire(id) || was_freezing && !self.entity(id).is_freezing() {
            self.play_entity_on_fire_extinguished_sound(id);
        }
        let was_ignited_this_tick = self.entity(id).remaining_fire_ticks > previous_fire_ticks;
        if !self.is_on_fire(id) && !was_ignited_this_tick {
            let ticks = -self.fire_immune_ticks(id);
            self.set_remaining_fire_ticks(id, ticks);
        }
    }

    fn play_entity_on_fire_extinguished_sound(&mut self, id: EntityId) {
        let source = self.sound_source(id);
        let e = self.entity_mut(id);
        let pitch = 1.6 + (e.random.next_float() - e.random.next_float()) * 0.4;
        let (x, y, z) = (e.x(), e.y(), e.z());
        self.play_sound_at(None, x, y, z, sound_event("minecraft:entity.generic.extinguish_fire"), source, 0.7, pitch);
    }

    fn apply_inside_effects(&mut self, id: EntityId) {
        if self.entity_ref(id).is_none() {
            return;
        }
        let effects = self.entity_mut(id).inside_effects.take_final();
        for effect in effects {
            if !self.is_alive(id) {
                break;
            }
            match effect {
                QueuedEffect::Type(ty) => self.apply_inside_block_effect_type(id, ty),
                QueuedEffect::Callback(f) => f(self, id),
            }
        }
    }

    fn apply_inside_block_effect_type(&mut self, id: EntityId, ty: InsideBlockEffectType) {
        match ty {
            InsideBlockEffectType::Freeze => {
                let can_freeze = !entity_type_tags::FREEZE_IMMUNE_ENTITY_TYPES.contains(self.entity(id).entity_type as usize);
                let e = self.entity_mut(id);
                e.is_in_powder_snow = true;
                if can_freeze {
                    let ticks = (e.ticks_frozen() + 1).min(140);
                    e.set_ticks_frozen(ticks);
                }
            }
            InsideBlockEffectType::ClearFreeze => self.entity_mut(id).set_ticks_frozen(0),
            InsideBlockEffectType::FireIgnite => self.fire_ignite(id),
            InsideBlockEffectType::LavaIgnite => {
                if !self.fire_immune(id) {
                    self.ignite_for_ticks(id, mth::floor_f(15.0 * 20.0));
                }
            }
            InsideBlockEffectType::Extinguish => self.clear_fire(id),
        }
    }

    fn fire_ignite(&mut self, id: EntityId) {
        if self.fire_immune(id) {
            return;
        }
        let ticks = self.entity(id).remaining_fire_ticks;
        if ticks < 0 {
            self.set_remaining_fire_ticks(id, ticks + 1);
        } else if self.is_player(id) {
            let added = 1 + self.random.next_int_bounded(2);
            self.set_remaining_fire_ticks(id, ticks + added);
        }
        if self.entity(id).remaining_fire_ticks >= 0 {
            self.ignite_for_ticks(id, mth::floor_f(8.0 * 20.0));
        }
    }

    fn check_inside_blocks(&mut self, id: EntityId, movements: &[Movement]) {
        if !self.is_affected_by_blocks(id) {
            return;
        }
        let mut visited: FastSet<BlockPos> = FastSet::default();
        for movement in movements {
            let mut pos = movement.from;
            let delta = movement.to.subtract(movement.from);
            let mut max_iterations = 16;
            match movement.axis_dependent_original_movement {
                Some(original) if delta.length_sqr() > 0.0 => {
                    for axis in Axis::step_order(original.x, original.z) {
                        let axis_move = delta.get(axis);
                        if axis_move != 0.0 {
                            let to = pos.relative(axis.positive(), axis_move);
                            max_iterations -= self.check_inside_blocks_between(id, pos, to, &mut visited, max_iterations);
                            pos = to;
                        }
                    }
                }
                _ => max_iterations -= self.check_inside_blocks_between(id, movement.from, movement.to, &mut visited, 16),
            }
            if max_iterations <= 0 {
                self.check_inside_blocks_between(id, movement.to, movement.to, &mut visited, 1);
            }
        }
    }

    fn check_inside_blocks_between(&mut self, id: EntityId, from: Vec3, to: Vec3, visited: &mut FastSet<BlockPos>, max_iterations: i32) -> i32 {
        let e = self.entity_mut(id);
        let dimensions = e.dimensions;
        let deflated = dimensions.make_bounding_box(to).deflate_all(1.0e-5f32 as f64);
        let moved_far = from.distance_to_sqr(to) > mth::square(0.9999900000002526);
        let mut effects = std::mem::take(&mut e.inside_effects);
        let mut iterations = 0;
        let box_from = dimensions.make_bounding_box(from);
        let travel = to.subtract(from);
        for_each_block_intersected_between(from, to, &deflated, |pos, iteration| {
            if !self.is_alive(id) {
                return false;
            }
            if iteration >= max_iterations {
                return false;
            }
            iterations = iteration;
            let state = self.block_state(pos);
            if state.is_air() {
                return true;
            }
            let behaviour = block::of(state);
            let inside_block = match behaviour.entity_inside_collision_shape(state, self, pos, id) {
                None => true,
                Some(boxes) => {
                    let moved: Vec<AABB> = boxes.iter().map(|b| b.move_to_block(pos)).collect();
                    box_from.collided_along_vector(travel, &moved)
                }
            };
            let fluid = state.fluid_state();
            let inside_fluid = match self.fluid_aabb(fluid, pos) {
                Some(fluid_box) => box_from.collided_along_vector(travel, &[fluid_box]),
                None => false,
            };
            if (inside_block || inside_fluid) && visited.insert(pos) {
                if inside_block {
                    let is_precise = moved_far || deflated.intersects_block(pos);
                    effects.advance_step(iteration);
                    behaviour.entity_inside(state, self, pos, id, &mut effects, is_precise);
                }
                if inside_fluid {
                    effects.advance_step(iteration);
                    if fluid.is(&fluid_tags::WATER) {
                        effects.apply(InsideBlockEffectType::Extinguish);
                    } else if fluid.is(&fluid_tags::LAVA) {
                        effects.apply(InsideBlockEffectType::ClearFreeze);
                        effects.apply(InsideBlockEffectType::LavaIgnite);
                        effects.run_after(InsideBlockEffectType::LavaIgnite, Box::new(|level, id| level.lava_hurt(id)));
                    }
                }
            }
            true
        });
        if self.entity_ref(id).is_some() {
            self.entity_mut(id).inside_effects = effects;
        }
        iterations + 1
    }

    pub fn update_fluid_interaction(&mut self, id: EntityId) -> bool {
        let pushed = self.is_pushed_by_fluid(id);
        let in_any_fluid = self.update_fluid_trackers(id, !pushed);
        let e = self.entity(id);
        let in_water = e.fluid_interaction.is_in_fluid(&fluid_tags::WATER);
        let in_lava = e.fluid_interaction.is_in_fluid(&fluid_tags::LAVA);
        if in_water {
            self.reset_fall_distance(id);
            let e = self.entity(id);
            if !e.was_touching_water && !e.first_tick {
                self.do_water_splash_effect(id);
            }
        }
        self.entity_mut(id).was_touching_water = in_water;
        if pushed {
            if in_water {
                self.apply_fluid_current(id, CurrentFluid::Water, 0.014);
            }
            if in_lava {
                let fast_lava = self.environment_attributes().get_dimension_value(environment_attributes::FAST_LAVA);
                self.apply_fluid_current(id, CurrentFluid::Lava, if fast_lava { 0.007 } else { 0.0023333333333333335 });
            }
        }
        in_any_fluid
    }

    fn do_water_splash_effect(&mut self, id: EntityId) {
        if self.is_spectator(id) || matches!(self.entities.get(&id), Some(super::LevelEntity::ExperienceOrb(_))) {
            return;
        }
        let player = self.is_player(id);
        let e = self.entity_mut(id);
        let m = e.delta_movement;
        let speed = (((m.x * m.x * 0.2f32 as f64 + m.y * m.y + m.z * m.z * 0.2f32 as f64).sqrt() as f32) * 0.2).min(1.0);
        let pitch = 1.0 + (e.random.next_float() - e.random.next_float()) * 0.4;
        let sound = match (player, speed < 0.25) {
            (true, true) => "minecraft:entity.player.splash",
            (true, false) => "minecraft:entity.player.splash.high_speed",
            (false, _) => "minecraft:entity.generic.splash",
        };
        self.entity_play_sound(id, sound_event(sound), speed, pitch);
        let e = self.entity_mut(id);
        let count = 1.0 + e.dimensions.width * 20.0;
        let mut i = 0;
        while (i as f32) < count {
            e.random.next_double();
            e.random.next_double();
            e.random.next_double();
            i += 1;
        }
        let mut i = 0;
        while (i as f32) < count {
            e.random.next_double();
            e.random.next_double();
            i += 1;
        }
    }

    pub fn update_swimming(&mut self, id: EntityId) {
        if self.players.get(&id).is_some_and(|p| p.abilities_flying()) {
            self.entity_mut(id).set_swimming(false);
            return;
        }
        let e = self.entity(id);
        let swimming = if e.is_swimming() {
            e.is_sprinting() && e.is_in_water()
        } else {
            e.is_sprinting() && e.is_under_water() && {
                let pos = e.block_position();
                self.fluid_state(pos).is(&fluid_tags::WATER)
            }
        };
        self.entity_mut(id).set_swimming(swimming);
    }

    pub fn entity_base_tick(&mut self, id: EntityId) {
        let e = self.entity_mut(id);
        e.compute_speed();
        e.in_block_state = None;
        e.was_in_powder_snow = e.is_in_powder_snow;
        e.is_in_powder_snow = false;
        e.was_eye_in_water = e.is_eye_in_fluid(&fluid_tags::WATER);
        self.update_fluid_interaction(id);
        self.update_swimming(id);
        let remaining_fire_ticks = self.entity(id).remaining_fire_ticks;
        if remaining_fire_ticks > 0 {
            if self.fire_immune(id) {
                self.clear_fire(id);
            } else {
                if remaining_fire_ticks % 20 == 0 && !self.entity(id).is_in_lava() {
                    self.hurt_server(id, crate::world::damagesource::DamageSource::new(crisp_data::damage_types::ON_FIRE), 1.0);
                }
                let ticks = self.entity(id).remaining_fire_ticks - 1;
                self.set_remaining_fire_ticks(id, ticks);
            }
        }
        let e = self.entity_mut(id);
        if e.is_in_lava() {
            e.fall_distance *= 0.5;
        }
        if self.entity(id).y() < (self.chunks.min_y() - 64) as f64 {
            if self.is_living(id) {
                self.hurt(id, crate::world::damagesource::DamageSource::new(crisp_data::damage_types::OUT_OF_WORLD), 4.0);
            } else {
                self.entity_mut(id).discard();
            }
        }
        let e = self.entity_mut(id);
        let on_fire = e.remaining_fire_ticks > 0;
        e.set_shared_flag(flags::ON_FIRE, on_fire);
        e.first_tick = false;
    }

    pub fn move_towards_closest_space(&mut self, id: EntityId, x: f64, y: f64, z: f64) {
        let pos = BlockPos::containing(x, y, z);
        let delta = Vec3::new(x - pos.x as f64, y - pos.y as f64, z - pos.z as f64);
        let mut closest_direction = Direction::Up;
        let mut closest = f64::MAX;
        for direction in [Direction::North, Direction::South, Direction::West, Direction::East, Direction::Up] {
            let neighbour = pos.relative(direction);
            if !self.block_state(neighbour).is_collision_shape_full_block() {
                let d = delta.get(direction.axis());
                let oriented = if direction.axis_direction() == AxisDirection::Positive { 1.0 - d } else { d };
                if oriented < closest {
                    closest = oriented;
                    closest_direction = direction;
                }
            }
        }
        let e = self.entity_mut(id);
        let speed = e.random.next_float() * 0.2 + 0.1;
        let step = closest_direction.axis_direction().step() as f32;
        let scaled = e.delta_movement.scale(0.75);
        let push = (step * speed) as f64;
        match closest_direction.axis() {
            Axis::X => e.set_delta_movement(Vec3::new(push, scaled.y, scaled.z)),
            Axis::Y => e.set_delta_movement(Vec3::new(scaled.x, push, scaled.z)),
            Axis::Z => e.set_delta_movement(Vec3::new(scaled.x, scaled.y, push)),
        }
    }

    pub fn is_free(&mut self, id: EntityId, xa: f64, ya: f64, za: f64) -> bool {
        let box_ = self.entity(id).bounding_box().move_by(xa, ya, za);
        self.no_collision_for(id, &box_) && !self.contains_any_liquid(&box_)
    }

    pub fn make_stuck_in_block(&mut self, id: EntityId, speed_multiplier: Vec3) {
        if !self.players.get(&id).is_some_and(|p| p.abilities_flying()) {
            self.reset_fall_distance(id);
            self.entity_mut(id).stuck_speed_multiplier = speed_multiplier;
        }
        if let Some(p) = self.players.get_mut(&id) {
            p.living.try_reset_current_impulse_context();
        }
    }

    pub fn friction_below(&mut self, id: EntityId) -> f32 {
        let pos = self.block_pos_below_that_affects_my_movement(id);
        self.block_state(pos).block().info().friction
    }

    pub fn entity_tick(&mut self, id: EntityId) {
        self.base_tick(id);
    }

    pub fn base_tick(&mut self, id: EntityId) {
        if self.is_mob(id) {
            self.mob_base_tick(id);
        } else if self.is_living(id) {
            self.living_entity_base_tick(id);
        } else {
            self.entity_base_tick(id);
        }
    }

    pub fn hurt(&mut self, id: EntityId, source: DamageSource, damage: f32) {
        self.hurt_server(id, source, damage);
    }

    pub fn hurt_server(&mut self, id: EntityId, source: DamageSource, damage: f32) -> bool {
        if self.is_player(id) {
            return self.server_player_hurt_server(id, source, damage);
        }
        if self.is_mob(id) {
            return self.living_entity_hurt_server(id, source, damage);
        }
        match self.entities.get(&id) {
            Some(LevelEntity::ExperienceOrb(_)) => self.experience_orb_hurt_server(id, &source, damage),
            Some(LevelEntity::Item(_)) => self.item_entity_hurt_server(id, source, damage),
            _ => false,
        }
    }

    pub fn is_invulnerable_to(&mut self, id: EntityId, source: &DamageSource) -> bool {
        if self.is_player(id) {
            self.server_player_is_invulnerable_to(id, source)
        } else if self.is_living(id) {
            self.living_entity_is_invulnerable_to(id, source)
        } else {
            self.is_invulnerable_to_base(id, source)
        }
    }

    pub fn is_invulnerable_to_base(&self, id: EntityId, source: &DamageSource) -> bool {
        let e = self.entity(id);
        let creative_player = source.causing_entity.and_then(|c| self.players.get(&c)).is_some_and(|p| p.abilities.instabuild);
        e.is_removed()
            || e.invulnerable && !source.is(&damage_type_tags::BYPASSES_INVULNERABILITY) && !creative_player
            || source.is(&damage_type_tags::IS_FIRE) && self.fire_immune(id)
            || source.is(&damage_type_tags::IS_FALL) && entity_type_tags::FALL_DAMAGE_IMMUNE.contains(e.entity_type as usize)
    }

    fn should_play_lava_hurt_sound(&self, id: EntityId) -> bool {
        match self.entities.get(&id) {
            Some(LevelEntity::Item(_)) => self.item_entity_should_play_lava_hurt_sound(id),
            _ => true,
        }
    }

    pub fn interact(&mut self, id: EntityId, player: EntityId, hand: InteractionHand, location: Vec3) -> InteractionResult {
        if !self.is_mob(id) {
            return self.entity_interact(id, player, hand, location);
        }
        if !self.is_alive(id) {
            return InteractionResult::Pass;
        }
        let interaction_result = self.check_and_handle_important_interactions(id, player, hand);
        if interaction_result.consumes_action() {
            return interaction_result;
        }
        let super_reaction = self.entity_interact(id, player, hand, location);
        if super_reaction != InteractionResult::Pass {
            return super_reaction;
        }
        let interaction_result = self.mob_interact(id, player, hand);
        if interaction_result.consumes_action() { interaction_result } else { InteractionResult::Pass }
    }

    pub fn entity_interact(&mut self, _id: EntityId, _player: EntityId, _hand: InteractionHand, _location: Vec3) -> InteractionResult {
        InteractionResult::Pass
    }

    pub fn is_attackable(&self, id: EntityId) -> bool {
        !matches!(self.entities.get(&id), Some(LevelEntity::Item(_) | LevelEntity::ExperienceOrb(_)))
    }

    pub fn is_allied_to(&self, id: EntityId, other: EntityId) -> bool {
        match (self.team_of(id), self.team_of(other)) {
            (Some(team), Some(other_team)) => team.name() == other_team.name(),
            _ => false,
        }
    }

    pub fn kill(&mut self, id: EntityId) {
        self.hurt_server(id, DamageSource::new(damage_types::GENERIC_KILL), f32::MAX);
    }

    pub fn set_remaining_fire_ticks(&mut self, id: EntityId, ticks: i32) {
        if self.is_player(id) {
            self.player_set_remaining_fire_ticks(id, ticks);
        } else {
            self.entity_mut(id).remaining_fire_ticks = ticks;
        }
    }

    pub fn clear_fire(&mut self, id: EntityId) {
        let ticks = self.entity(id).remaining_fire_ticks.min(0);
        self.set_remaining_fire_ticks(id, ticks);
    }

    pub fn lava_hurt(&mut self, id: EntityId) {
        if self.fire_immune(id) {
            return;
        }
        if self.hurt_server(id, DamageSource::new(damage_types::LAVA), 4.0) && self.should_play_lava_hurt_sound(id) && !self.entity(id).is_silent() {
            let source = self.sound_source(id);
            let e = self.entity_mut(id);
            let pitch = 2.0 + e.random.next_float() * 0.4;
            let (x, y, z) = (e.x(), e.y(), e.z());
            self.play_sound_at(None, x, y, z, sound_event("minecraft:entity.generic.burn"), source, 0.4, pitch);
        }
    }

    pub fn is_in_wall(&mut self, id: EntityId) -> bool {
        let e = self.entity(id);
        if e.no_physics {
            return false;
        }
        let check_width = (e.dimensions.width * 0.8) as f64;
        let eye = e.eye_position();
        let eye_bb = AABB::new(
            eye.x - check_width / 2.0,
            eye.y - 1.0e-6 / 2.0,
            eye.z - check_width / 2.0,
            eye.x + check_width / 2.0,
            eye.y + 1.0e-6 / 2.0,
            eye.z + check_width / 2.0,
        );
        let eye_shape = VoxelShape::from_boxes(&[eye_bb]);
        let (min, max) = (BlockPos::containing(eye_bb.min_x, eye_bb.min_y, eye_bb.min_z), BlockPos::containing(eye_bb.max_x, eye_bb.max_y, eye_bb.max_z));
        for x in min.x..=max.x {
            for y in min.y..=max.y {
                for z in min.z..=max.z {
                    let pos = BlockPos::new(x, y, z);
                    let state = self.block_state(pos);
                    if !state.is_air() && state.is_suffocating() {
                        let shape = state.collision_voxel_shape();
                        if join_is_not_empty(&shape.moved(pos.x as f64, pos.y as f64, pos.z as f64), &eye_shape, BooleanOp::And) {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }

    pub fn entity_to_string(&self, id: EntityId) -> String {
        let e = self.entity(id);
        let name = self.entities.get(&id).map(|e| e.name().get_string()).unwrap_or_default();
        let class = match self.mob(id).map(|m| &m.kind) {
            Some(super::mob::MobKind::Chicken(_)) => "Chicken",
            None => "Entity",
        };
        format!("{class}['{name}'/{id}, l='ServerLevel[{}]', x={:.2}, y={:.2}, z={:.2}]", self.level_name, e.x(), e.y(), e.z())
    }

    pub fn spawn_at_location(&mut self, id: EntityId, stack: ItemStack, offset: f32) {
        if stack.is_empty() {
            return;
        }
        let e = self.entity(id);
        let pos = Vec3::new(e.x(), e.y() + offset as f64, e.z());
        let mut item = ItemEntity::new(pos, stack);
        item.set_default_pickup_delay();
        self.add_fresh_entity(LevelEntity::Item(item));
    }
}

fn collect_candidate_step_up_heights(bounding_box: &AABB, colliders: &[Collider], max_step_height: f32, step_height_to_skip: f32) -> Vec<f32> {
    let mut candidates: Vec<f32> = Vec::with_capacity(4);
    for collider in colliders {
        for coord in collider.coords(Axis::Y) {
            let relative = (coord - bounding_box.min_y) as f32;
            if !(relative < 0.0) && relative != step_height_to_skip {
                if relative > max_step_height {
                    break;
                }
                if !candidates.iter().any(|c| c.to_bits() == relative.to_bits()) {
                    candidates.push(relative);
                }
            }
        }
    }
    candidates.sort_by(f32::total_cmp);
    candidates
}

trait SignumJava {
    fn signum_java(self) -> f64;
}

impl SignumJava for f64 {
    fn signum_java(self) -> f64 {
        if self == 0.0 || self.is_nan() { self } else { self.signum() }
    }
}

#[cfg(test)]
mod tests {
    use crisp_core::{BlockPos, ChunkPos, Vec3};
    use crisp_data::{blocks, items};

    use crate::server::level::{ServerLevel, testing};
    use crate::world::entity::{ItemEntity, LevelEntity, MoverType};
    use crate::world::item::ItemStack;
    use crate::world::level::update_flags::UPDATE_ALL;

    fn loaded_level() -> ServerLevel {
        let mut level = testing::level();
        for x in -2..=2 {
            for z in -2..=2 {
                level.chunks.chunk(ChunkPos::new(x, z));
            }
        }
        level
    }

    #[test]
    fn dropped_item_lands_on_the_ground_and_rests() {
        let mut level = loaded_level();
        let item = ItemEntity::with_movement(Vec3::new(0.5, -59.0, 0.5), ItemStack::new(items::STONE, 1), Vec3::ZERO);
        let id = item.entity.id;
        level.add_fresh_entity(LevelEntity::Item(item));
        let (mut y, mut v, mut landing_tick) = (-59.0f64, 0.0f64, 0);
        let landed_y = loop {
            landing_tick += 1;
            v -= 0.04;
            if y + v <= -60.0 {
                break y + (-60.0 - y);
            }
            y += v;
            v *= 0.98f32 as f64;
        };
        assert_eq!(landing_tick, 7);
        for tick in 1..=60 {
            level.common_tick(id);
            level.tick_item(id);
            let e = level.entity(id);
            if tick < landing_tick {
                assert!(!e.on_ground, "tick {tick}");
                assert!(e.y() > -60.0, "tick {tick}");
            } else {
                assert!(e.on_ground, "tick {tick}");
                assert_eq!(e.y(), landed_y, "tick {tick}");
                assert_eq!(e.x(), 0.5);
                assert_eq!(e.z(), 0.5);
            }
        }
    }

    #[test]
    fn player_walking_into_a_block_is_stopped() {
        let mut level = loaded_level();
        level.set_block(BlockPos::new(2, -60, 0), blocks::STONE.default_state(), UPDATE_ALL);
        let id = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
        level.move_entity(id, MoverType::Player, Vec3::new(2.0, 0.0, 0.0));
        let e = level.entity(id);
        let half_width = (0.6f32 / 2.0) as f64;
        assert_eq!(e.x(), 0.5 + (2.0 - (0.5 + half_width)));
        assert_eq!(e.y(), -60.0);
        assert!(e.horizontal_collision);
    }

    #[test]
    fn moving_through_a_block_is_refused() {
        let mut level = loaded_level();
        level.set_block(BlockPos::new(2, -60, 0), blocks::STONE.default_state(), UPDATE_ALL);
        let id = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
        level.handle_player_loaded(id);
        level.handle_move_player(id, Some(Vec3::new(2.5, -60.0, 0.5)), None, true, false);
        let p = level.player(id);
        assert_eq!(p.entity.position(), Vec3::new(0.5, -60.0, 0.5));
        assert_eq!(p.movement.awaiting_position_from_client, Some(Vec3::new(0.5, -60.0, 0.5)));
    }

    #[test]
    fn player_steps_up_onto_a_slab() {
        let mut level = loaded_level();
        level.set_block(BlockPos::new(1, -60, 0), blocks::OAK_SLAB.default_state(), UPDATE_ALL);
        let id = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
        level.entity_mut(id).on_ground = true;
        level.move_entity(id, MoverType::Player, Vec3::new(1.0, 0.0, 0.0));
        assert_eq!(level.entity(id).position(), Vec3::new(1.5, -59.5, 0.5));
    }

    #[test]
    fn a_full_block_is_too_high_to_step_onto() {
        let mut level = loaded_level();
        level.set_block(BlockPos::new(1, -60, 0), blocks::STONE.default_state(), UPDATE_ALL);
        let id = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
        level.entity_mut(id).on_ground = true;
        level.move_entity(id, MoverType::Player, Vec3::new(1.0, 0.0, 0.0));
        assert_eq!(level.entity(id).y(), -60.0);
        assert!(level.entity(id).x() < 0.71);
    }

    #[test]
    fn falling_player_accumulates_fall_distance_and_lands() {
        let mut level = loaded_level();
        let id = testing::player(&mut level, Vec3::new(0.5, -50.0, 0.5), 0.0, 0.0);
        level.handle_player_loaded(id);
        let (mut y, mut v) = (-50.0f64, 0.0f64);
        let mut expected = 0.0f64;
        loop {
            level.tick_player(id);
            level.handle_client_tick_end(id);
            v = (v - 0.08) * 0.98f32 as f64;
            let next = (y + v).max(-60.0);
            let landed = next == -60.0;
            level.handle_move_player(id, Some(Vec3::new(0.5, next, 0.5)), None, landed, false);
            assert_eq!(level.entity(id).y(), next);
            expected -= ((next - y) as f32) as f64;
            y = next;
            if landed {
                break;
            }
            assert_eq!(level.entity(id).fall_distance, expected);
            assert!(!level.entity(id).on_ground);
        }
        assert!(level.entity(id).on_ground);
        assert_eq!(level.entity(id).fall_distance, 0.0);
        assert!((expected - 10.0).abs() < 1.0e-4);
        assert_eq!(level.calculate_fall_damage(id, expected, 1.0), 7);
        assert_eq!(level.player(id).health, 13.0);
    }
}
