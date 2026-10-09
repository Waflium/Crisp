pub mod attributes;
pub mod data;
#[cfg(test)]
mod effect_tests;
pub mod effects;
pub mod entity_save;
pub mod fluid_interaction;
pub mod hurt;
pub mod inside_block;
pub mod item;
pub mod lightning_bolt;
pub mod primed_tnt;
pub mod living;
pub mod movement;
pub mod player;
pub mod relative;
pub mod schedule;
#[cfg(test)]
mod survival_tests;
pub mod using_item;

use crisp_protocol::nbt::Nbt;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicI32, Ordering};

use crisp_core::random::{LegacyRandomSource, RandomSource};
use crisp_core::{AABB, BlockPos, ChunkPos, Vec3, mth};
use crisp_data::BlockState;

use data::{DataValue, Pose, SynchedEntityData, ids};
use fluid_interaction::EntityFluidInteraction;
use inside_block::InsideBlockEffects;
pub use item::ItemEntity;

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

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct EntityDimensions {
    pub width: f32,
    pub height: f32,
    pub eye_height: f32,
}

impl EntityDimensions {
    pub fn make_bounding_box(self, pos: Vec3) -> AABB {
        let w = (self.width / 2.0) as f64;
        let h = self.height as f64;
        AABB::new(pos.x - w, pos.y, pos.z - w, pos.x + w, pos.y + h, pos.z + w)
    }
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

pub enum LevelEntity {
    Item(ItemEntity),
    LightningBolt(lightning_bolt::LightningBolt),
    PrimedTnt(primed_tnt::PrimedTnt),
}

impl LevelEntity {
    pub fn name(&self) -> crate::network::chat::Component {
        match self {
            LevelEntity::Item(item) => crate::network::chat::Component::translatable(item.item.item.info().description_id),
            LevelEntity::LightningBolt(_) => crate::network::chat::Component::translatable("entity.minecraft.lightning_bolt"),
            LevelEntity::PrimedTnt(_) => crate::network::chat::Component::translatable("entity.minecraft.tnt"),
        }
    }

    pub fn display_name(&self, team: Option<&crate::world::scores::player_team::PlayerTeam>) -> crate::network::chat::Component {
        use crate::network::chat::HoverEvent;
        let entity = self.entity();
        let type_id = crisp_data::registry::ENTITY_TYPE[entity.entity_type as usize];
        crate::world::scores::player_team::PlayerTeam::format_name_for_team(team, self.name())
            .with_hover(HoverEvent::ShowEntity { id: type_id, uuid: entity.uuid, name: Some(Box::new(self.name())) })
            .with_insertion(&crate::world::level::storage::format_uuid(entity.uuid))
    }

    pub fn tracking_config(&self) -> crate::server::level::chunk_map::TrackingConfig {
        use crate::server::level::chunk_map::TrackingConfig;
        use crate::server::level::server_entity::UpdateInterval;
        match self {
            LevelEntity::Item(_) => TrackingConfig { range: 6 * 16, update_interval: UpdateInterval::Periodic(20), track_deltas: true, stepped: false, is_item: true },
            LevelEntity::PrimedTnt(_) => TrackingConfig { range: 10 * 16, update_interval: UpdateInterval::Periodic(10), track_deltas: true, stepped: false, is_item: false },
            LevelEntity::LightningBolt(_) => TrackingConfig { range: 16 * 16, update_interval: UpdateInterval::Never, track_deltas: true, stepped: false, is_item: false },
        }
    }

    pub fn entity(&self) -> &Entity {
        match self {
            LevelEntity::Item(item) => &item.entity,
            LevelEntity::LightningBolt(bolt) => &bolt.entity,
            LevelEntity::PrimedTnt(tnt) => &tnt.entity,
        }
    }

    pub fn entity_mut(&mut self) -> &mut Entity {
        match self {
            LevelEntity::Item(item) => &mut item.entity,
            LevelEntity::LightningBolt(bolt) => &mut bolt.entity,
            LevelEntity::PrimedTnt(tnt) => &mut tnt.entity,
        }
    }

    pub fn as_item(&self) -> Option<&ItemEntity> {
        match self {
            LevelEntity::Item(item) => Some(item),
            _ => None,
        }
    }

    pub fn as_item_mut(&mut self) -> Option<&mut ItemEntity> {
        match self {
            LevelEntity::Item(item) => Some(item),
            _ => None,
        }
    }
}

pub fn can_summon(entity_type: &str) -> bool {
    !matches!(entity_type, "minecraft:player" | "minecraft:fishing_bobber")
}

pub fn description(entity_type: &str) -> crate::network::chat::Component {
    crate::network::chat::Component::translatable(&format!("entity.{}", entity_type.replacen(':', ".", 1)))
}
