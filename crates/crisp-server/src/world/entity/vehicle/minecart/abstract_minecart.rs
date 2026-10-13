use crisp_core::{AABB, BlockPos, Direction, Vec3, mth};
use crisp_data::properties::{POWERED, RailShape};
use crisp_data::{BlockState, Item, block_tags, blocks, items, registry, registry_id};
use crisp_protocol::nbt::Nbt;

use super::minecart::Minecart;
use super::old_minecart_behavior::OldMinecartBehavior;
use crate::server::level::ServerLevel;
use crate::world::entity::data::{DataValue, Pose};
use crate::world::entity::vehicle::dismount_helper;
use crate::world::entity::vehicle::vehicle_entity::{self, vehicle_entity_defaults};
use crate::world::entity::{Entity, EntityDimensions, EntityId, LevelEntity, MoverType};
use crate::world::level::BlockGetter;
use crate::world::level::block::base_rail_block;
use crate::world::level::chunk::serialization::{state_from_nbt, state_to_nbt};

const LOWERED_PASSENGER_ATTACHMENT: Vec3 = Vec3::new(0.0, 0.0, 0.0);
const DATA_ID_CUSTOM_DISPLAY_BLOCK: usize = 11;
const DATA_ID_DISPLAY_OFFSET: usize = 12;
pub const WATER_SLOWDOWN_FACTOR: f32 = 0.95;
const DEFAULT_FLIPPED_ROTATION: bool = false;
pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.98, 0.7).with_passenger_attachments(&[Vec3::new(0.0, 0.1875, 0.0)]);

pub enum MinecartKind {
    Minecart(Minecart),
}

pub struct AbstractMinecart {
    pub entity: Entity,
    on_rails: bool,
    flipped: bool,
    pub behavior: OldMinecartBehavior,
    pub kind: MinecartKind,
}

pub fn exits(shape: RailShape) -> ([i32; 3], [i32; 3]) {
    let x_neg = [-1, 0, 0];
    let x_pos = [1, 0, 0];
    let z_neg = [0, 0, -1];
    let z_pos = [0, 0, 1];
    let below = |v: [i32; 3]| [v[0], v[1] - 1, v[2]];
    match shape {
        RailShape::NorthSouth => (z_neg, z_pos),
        RailShape::EastWest => (x_neg, x_pos),
        RailShape::AscendingEast => (below(x_neg), x_pos),
        RailShape::AscendingWest => (x_neg, below(x_pos)),
        RailShape::AscendingNorth => (z_neg, below(z_pos)),
        RailShape::AscendingSouth => (below(z_neg), z_pos),
        RailShape::SouthEast => (z_pos, x_pos),
        RailShape::SouthWest => (z_pos, x_neg),
        RailShape::NorthWest => (z_neg, x_neg),
        RailShape::NorthEast => (z_neg, x_pos),
    }
}

pub fn rail_shape(state: BlockState) -> RailShape {
    state.get(base_rail_block::shape_property(state.block()))
}

impl AbstractMinecart {
    pub fn new(entity_type: &str, kind: MinecartKind) -> AbstractMinecart {
        let mut data = vehicle_entity_defaults();
        data.push(DataValue::OptionalBlockState(None));
        data.push(DataValue::Int(Self::default_display_offset()));
        let entity = Entity::new(registry_id(registry::ENTITY_TYPE, entity_type), DIMENSIONS, data);
        AbstractMinecart { entity, on_rails: false, flipped: DEFAULT_FLIPPED_ROTATION, behavior: OldMinecartBehavior::default(), kind }
    }

    pub fn set_initial_pos(&mut self, x: f64, y: f64, z: f64) {
        self.entity.set_pos(Vec3::new(x, y, z));
        self.entity.xo = Vec3::new(x, y, z);
    }

    pub fn is_on_rails(&self) -> bool {
        self.on_rails
    }

    pub fn set_on_rails(&mut self, on_rails: bool) {
        self.on_rails = on_rails;
    }

    pub fn is_flipped(&self) -> bool {
        self.flipped
    }

    pub fn set_flipped(&mut self, flipped: bool) {
        self.flipped = flipped;
    }

    pub fn get_drop_item(&self) -> Item {
        match &self.kind {
            MinecartKind::Minecart(_) => items::MINECART,
        }
    }

    pub fn get_pick_result(&self) -> Item {
        match &self.kind {
            MinecartKind::Minecart(_) => items::MINECART,
        }
    }

    pub fn is_rideable(&self) -> bool {
        match &self.kind {
            MinecartKind::Minecart(_) => true,
        }
    }

    pub fn is_furnace(&self) -> bool {
        false
    }

    fn get_custom_display_block_state(&self) -> Option<BlockState> {
        match self.entity.data.get(DATA_ID_CUSTOM_DISPLAY_BLOCK) {
            DataValue::OptionalBlockState(state) => *state,
            _ => None,
        }
    }

    pub fn get_display_offset(&self) -> i32 {
        match self.entity.data.get(DATA_ID_DISPLAY_OFFSET) {
            DataValue::Int(v) => *v,
            _ => Self::default_display_offset(),
        }
    }

    fn default_display_offset() -> i32 {
        6
    }

    pub fn set_custom_display_block_state(&mut self, state: Option<BlockState>) {
        self.entity.data.set(DATA_ID_CUSTOM_DISPLAY_BLOCK, DataValue::OptionalBlockState(state));
    }

    pub fn set_display_offset(&mut self, offset: i32) {
        self.entity.data.set(DATA_ID_DISPLAY_OFFSET, DataValue::Int(offset));
    }

    pub fn read_additional_save_data(&mut self, nbt: &Nbt) {
        self.set_custom_display_block_state(nbt.get("DisplayState").and_then(state_from_nbt));
        self.set_display_offset(nbt.get("DisplayOffset").and_then(Nbt::as_i64).map_or(Self::default_display_offset(), |v| v as i32));
        self.flipped = nbt.get("FlippedRotation").and_then(Nbt::as_i64).is_some_and(|v| v != 0);
        self.entity.first_tick = nbt.get("HasTicked").and_then(Nbt::as_i64).is_some_and(|v| v != 0);
    }

    pub fn add_additional_save_data(&self, out: &mut Vec<(String, Nbt)>) {
        if let Some(state) = self.get_custom_display_block_state() {
            out.push(("DisplayState".into(), state_to_nbt(state)));
        }
        let display_offset = self.get_display_offset();
        if display_offset != Self::default_display_offset() {
            out.push(("DisplayOffset".into(), Nbt::Int(display_offset)));
        }
        out.push(("FlippedRotation".into(), Nbt::Byte(self.flipped as i8)));
        out.push(("HasTicked".into(), Nbt::Byte(self.entity.first_tick as i8)));
    }
}

impl ServerLevel {
    pub fn minecart(&self, id: EntityId) -> &AbstractMinecart {
        match self.entities.get(&id) {
            Some(LevelEntity::Minecart(minecart)) => minecart,
            _ => unreachable!("not a minecart"),
        }
    }

    pub fn minecart_mut(&mut self, id: EntityId) -> &mut AbstractMinecart {
        match self.entities.get_mut(&id) {
            Some(LevelEntity::Minecart(minecart)) => minecart,
            _ => unreachable!("not a minecart"),
        }
    }

    pub fn is_minecart(&self, id: EntityId) -> bool {
        matches!(self.entities.get(&id), Some(LevelEntity::Minecart(_)))
    }

    pub fn abstract_minecart_passenger_attachment_point(&self, id: EntityId, passenger: EntityId) -> Vec3 {
        let lowered = self.entity_ref(passenger).is_some_and(|p| {
            p.entity_type == registry_id(registry::ENTITY_TYPE, "minecraft:villager") || p.entity_type == registry_id(registry::ENTITY_TYPE, "minecraft:wandering_trader")
        });
        if lowered { LOWERED_PASSENGER_ATTACHMENT } else { self.default_passenger_attachment_point(id, passenger) }
    }

    pub fn abstract_minecart_dismount_location_for_passenger(&mut self, id: EntityId, passenger: EntityId) -> Vec3 {
        let forward = self.minecart_motion_direction(id);
        if forward.axis() == crisp_core::Axis::Y {
            return self.entity_dismount_location_for_passenger(id, passenger);
        }
        let offsets = dismount_helper::offsets_for_direction(forward);
        let vehicle_block_pos = self.entity(id).block_position();
        let dismount_poses = self.dismount_poses(passenger);
        for &pose in dismount_poses {
            let passenger_dimensions = self.dimensions_for_pose(passenger, pose);
            let dismount_area_reach = (passenger_dimensions.width.min(1.0) / 2.0) as f64;
            for &offset_y in pose_dismount_heights(pose) {
                for offset_xz in offsets {
                    let target_block_pos = BlockPos::new(vehicle_block_pos.x + offset_xz[0], vehicle_block_pos.y + offset_y, vehicle_block_pos.z + offset_xz[1]);
                    let block_floor_height = dismount_helper::get_block_floor_height_non_climbable(self, target_block_pos);
                    if dismount_helper::is_block_floor_valid(block_floor_height) {
                        let dismount_collision_box =
                            AABB::new(-dismount_area_reach, 0.0, -dismount_area_reach, dismount_area_reach, passenger_dimensions.height as f64, dismount_area_reach);
                        let location = Vec3::up_from_bottom_center_of(target_block_pos, block_floor_height);
                        if dismount_helper::can_dismount_to(self, passenger, &dismount_collision_box.move_by(location.x, location.y, location.z)) {
                            self.set_entity_pose(passenger, pose);
                            return location;
                        }
                    }
                }
            }
        }
        let vehicle_top = self.entity(id).bounding_box().max_y;
        let target_block_pos = BlockPos::new(vehicle_block_pos.x, mth::floor(vehicle_top), vehicle_block_pos.z);
        for &pose in dismount_poses {
            let pose_height = self.dimensions_for_pose(passenger, pose).height as f64;
            let block_coverage_y = mth::ceil(vehicle_top - target_block_pos.y as f64 + pose_height);
            let ceiling_above_vehicle = dismount_helper::find_ceiling_from(self, target_block_pos, block_coverage_y);
            if vehicle_top + pose_height <= ceiling_above_vehicle {
                self.set_entity_pose(passenger, pose);
                break;
            }
        }
        self.entity_dismount_location_for_passenger(id, passenger)
    }

    pub fn abstract_minecart_block_speed_factor(&mut self, id: EntityId) -> Option<f32> {
        let state = self.block_state(self.entity(id).block_position());
        state.is(&block_tags::RAILS).then_some(1.0)
    }

    pub fn minecart_motion_direction(&self, id: EntityId) -> Direction {
        let minecart = self.minecart(id);
        let direction = Direction::from_y_rot(minecart.entity.y_rot as f64);
        if minecart.is_flipped() { direction.opposite().clockwise() } else { direction.clockwise() }
    }

    pub fn abstract_minecart_default_gravity(&self, id: EntityId) -> f64 {
        if self.entity(id).is_in_water() { 0.005 } else { 0.04 }
    }

    pub fn tick_minecart(&mut self, id: EntityId) {
        match &self.minecart(id).kind {
            MinecartKind::Minecart(_) => self.minecart_tick(id),
        }
    }

    pub fn abstract_minecart_tick(&mut self, id: EntityId) {
        let e = self.entity_mut(id);
        let hurt_time = vehicle_entity::get_hurt_time(e);
        if hurt_time > 0 {
            vehicle_entity::set_hurt_time(e, hurt_time - 1);
        }
        let damage = vehicle_entity::get_damage(e);
        if damage > 0.0 {
            vehicle_entity::set_damage(e, damage - 1.0);
        }
        self.check_below_world(id);
        if self.entity(id).is_removed() {
            return;
        }
        self.entity_mut(id).compute_speed();
        self.handle_portal(id);
        if self.entity(id).is_removed() {
            return;
        }
        self.old_minecart_behavior_tick(id);
        if self.entity_ref(id).is_none_or(Entity::is_removed) {
            return;
        }
        self.update_fluid_interaction(id);
        if self.entity(id).is_in_lava() {
            self.lava_ignite(id);
            self.lava_hurt(id);
            if self.entity_ref(id).is_none() {
                return;
            }
            self.entity_mut(id).fall_distance *= 0.5;
        }
        self.entity_mut(id).first_tick = false;
    }

    pub fn get_current_block_pos_or_rail_below(&mut self, id: EntityId) -> BlockPos {
        let e = self.entity(id);
        let (xt, mut yt, zt) = (mth::floor(e.x()), mth::floor(e.y()), mth::floor(e.z()));
        if self.block_state(BlockPos::new(xt, yt - 1, zt)).is(&block_tags::RAILS) {
            yt -= 1;
        }
        BlockPos::new(xt, yt, zt)
    }

    pub fn minecart_max_speed(&self, id: EntityId) -> f64 {
        self.old_minecart_behavior_max_speed(id)
    }

    pub fn activate_minecart(&mut self, id: EntityId, xt: i32, yt: i32, zt: i32, state: bool) {
        match &self.minecart(id).kind {
            MinecartKind::Minecart(_) => self.minecart_activate_minecart(id, xt, yt, zt, state),
        }
    }

    pub fn come_off_track(&mut self, id: EntityId) {
        let max_speed = self.minecart_max_speed(id);
        let e = self.entity_mut(id);
        let movement = e.delta_movement;
        e.set_delta_movement(Vec3::new(mth::clamp(movement.x, -max_speed, max_speed), movement.y, mth::clamp(movement.z, -max_speed, max_speed)));
        if e.on_ground {
            e.set_delta_movement(e.delta_movement.scale(0.5));
        }
        let delta = e.delta_movement;
        self.minecart_move(id, MoverType::SelfMove, delta);
        if self.entity_ref(id).is_none() {
            return;
        }
        let air_drag = self.air_drag(id) as f64;
        let e = self.entity_mut(id);
        if !e.on_ground {
            e.set_delta_movement(e.delta_movement.scale(air_drag));
        }
    }

    pub fn minecart_move(&mut self, id: EntityId, mover: MoverType, delta: Vec3) {
        self.entity_move(id, mover, delta);
        if self.entity_ref(id).is_some() {
            self.minecart_apply_effects_from_blocks(id);
        }
    }

    pub fn minecart_apply_effects_from_blocks(&mut self, id: EntityId) {
        let position = self.entity(id).position();
        self.apply_effects_from_blocks_between(id, position, position);
        if self.entity_ref(id).is_some() {
            self.entity_mut(id).clear_movement_this_tick();
        }
    }

    pub fn get_redstone_direction(&mut self, pos: BlockPos) -> Vec3 {
        let state = self.block_state(pos);
        if state.is_block(blocks::POWERED_RAIL) && state.get(POWERED) {
            let shape = rail_shape(state);
            if shape == RailShape::EastWest {
                if self.is_redstone_conductor(pos.west()) {
                    return Vec3::new(1.0, 0.0, 0.0);
                }
                if self.is_redstone_conductor(pos.east()) {
                    return Vec3::new(-1.0, 0.0, 0.0);
                }
            } else if shape == RailShape::NorthSouth {
                if self.is_redstone_conductor(pos.north()) {
                    return Vec3::new(0.0, 0.0, 1.0);
                }
                if self.is_redstone_conductor(pos.south()) {
                    return Vec3::new(0.0, 0.0, -1.0);
                }
            }
        }
        Vec3::ZERO
    }

    pub fn is_redstone_conductor(&mut self, pos: BlockPos) -> bool {
        self.block_state(pos).is_redstone_conductor()
    }

    pub fn apply_natural_slowdown(&self, id: EntityId, movement: Vec3) -> Vec3 {
        let slowdown_factor = self.old_minecart_behavior_slowdown_factor(id);
        let new_movement = movement.multiply(slowdown_factor, 0.0, slowdown_factor);
        if self.entity(id).is_in_water() { new_movement.scale(WATER_SLOWDOWN_FACTOR as f64) } else { new_movement }
    }

    pub fn abstract_minecart_push(&mut self, id: EntityId, entity: EntityId) {
        let (e, o) = (self.entity(id), self.entity(entity));
        if o.no_physics || e.no_physics || self.has_passenger(id, entity) {
            return;
        }
        let mut xa = o.x() - e.x();
        let mut za = o.z() - e.z();
        let mut dd = xa * xa + za * za;
        if dd >= 1.0e-4f32 as f64 {
            dd = dd.sqrt();
            xa /= dd;
            za /= dd;
            let pow = (1.0 / dd).min(1.0);
            xa *= pow;
            za *= pow;
            xa *= 0.1f32 as f64;
            za *= 0.1f32 as f64;
            xa *= 0.5;
            za *= 0.5;
            if self.is_minecart(entity) {
                self.push_other_minecart(id, entity, xa, za);
            } else {
                self.entity_mut(id).push(-xa, 0.0, -za);
                self.entity_push(entity, xa / 4.0, 0.0, za / 4.0);
            }
        }
    }

    fn push_other_minecart(&mut self, id: EntityId, other: EntityId, xa: f64, za: f64) {
        let (e, o) = (self.entity(id), self.entity(other));
        let xo = o.x() - e.x();
        let zo = o.z() - e.z();
        let dir = Vec3::new(xo, 0.0, zo).normalize();
        let y_rot = e.y_rot * mth::DEG_TO_RAD;
        let facing = Vec3::new(mth::cos(y_rot as f64) as f64, 0.0, mth::sin(y_rot as f64) as f64).normalize();
        let dot = dir.dot(facing).abs();
        if dot < 0.8f32 as f64 {
            return;
        }
        let movement = e.delta_movement;
        let entity_movement = o.delta_movement;
        let (self_furnace, other_furnace) = (self.minecart(id).is_furnace(), self.minecart(other).is_furnace());
        if other_furnace && !self_furnace {
            let e = self.entity_mut(id);
            e.set_delta_movement(movement.multiply(0.2, 1.0, 0.2));
            e.push(entity_movement.x - xa, 0.0, entity_movement.z - za);
            self.entity_mut(other).set_delta_movement(entity_movement.multiply(0.95, 1.0, 0.95));
        } else if !other_furnace && self_furnace {
            let o = self.entity_mut(other);
            o.set_delta_movement(entity_movement.multiply(0.2, 1.0, 0.2));
            o.push(movement.x + xa, 0.0, movement.z + za);
            self.entity_mut(id).set_delta_movement(movement.multiply(0.95, 1.0, 0.95));
        } else {
            let xdd = (entity_movement.x + movement.x) / 2.0;
            let zdd = (entity_movement.z + movement.z) / 2.0;
            let e = self.entity_mut(id);
            e.set_delta_movement(movement.multiply(0.2, 1.0, 0.2));
            e.push(xdd - xa, 0.0, zdd - za);
            let o = self.entity_mut(other);
            o.set_delta_movement(entity_movement.multiply(0.2, 1.0, 0.2));
            o.push(xdd + xa, 0.0, zdd + za);
        }
    }

    pub fn abstract_minecart_animate_hurt(&mut self, id: EntityId) {
        let e = self.entity_mut(id);
        let hurt_dir = vehicle_entity::get_hurt_dir(e);
        vehicle_entity::set_hurt_dir(e, -hurt_dir);
        vehicle_entity::set_hurt_time(e, 10);
        let damage = vehicle_entity::get_damage(e);
        vehicle_entity::set_damage(e, damage + damage * 10.0);
    }
}

fn pose_dismount_heights(pose: Pose) -> &'static [i32] {
    match pose {
        Pose::Swimming => &[0, 1],
        _ => &[0, 1, -1],
    }
}
