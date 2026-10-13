use crisp_core::random::RandomSource;
use crisp_core::shapes::{BooleanOp, VoxelShape, join_is_not_empty};
use crisp_core::{AABB, BlockPos, Vec3, mth};
use crisp_data::{Item, entity_type_tags, fluid_tags, registry, registry_id};

use crate::server::level::ServerLevel;
use super::abstract_chest_boat;
use crate::world::entity::data::DataValue;
use crate::world::entity::vehicle::container_entity::ContainerEntity;
use crate::world::entity::vehicle::dismount_helper;
use crate::world::entity::vehicle::vehicle_entity::{self, vehicle_entity_defaults};
use crate::world::entity::{Entity, EntityDimensions, EntityId, LevelEntity, MoverType, get_collision_horizontal_escape_vector, sound_event};
use crate::world::level::BlockGetter;
use crate::world::{InteractionHand, InteractionResult};

const DATA_ID_PADDLE_LEFT: usize = 11;
const DATA_ID_PADDLE_RIGHT: usize = 12;
const DATA_ID_BUBBLE_TIME: usize = 13;
pub const PADDLE_LEFT: usize = 0;
pub const PADDLE_RIGHT: usize = 1;
const TIME_TO_EJECT: f32 = 60.0;
const PADDLE_SPEED: f32 = std::f64::consts::PI as f32 / 8.0;
const PADDLE_SOUND_TIME: f32 = std::f64::consts::PI as f32 / 4.0;
const BUBBLE_TIME: i32 = 60;
pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(1.375, 0.5625).with_eye_height(0.5625);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Status {
    InWater,
    UnderWater,
    UnderFlowingWater,
    OnLand,
    InAir,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BoatKind {
    Boat,
    Raft,
    ChestBoat,
    ChestRaft,
}

pub struct AbstractBoat {
    pub entity: Entity,
    pub kind: BoatKind,
    pub container: Option<ContainerEntity>,
    paddle_positions: [f32; 2],
    out_of_control_ticks: f32,
    delta_rotation: f32,
    water_level: f64,
    land_friction: f32,
    status: Option<Status>,
    old_status: Option<Status>,
    last_yd: f64,
    is_above_bubble_column: bool,
    bubble_column_direction_is_down: bool,
    drop_item: Item,
}

impl AbstractBoat {
    pub fn new(entity_type: &str, kind: BoatKind, drop_item: Item) -> AbstractBoat {
        let mut data = vehicle_entity_defaults();
        data.extend([DataValue::Boolean(false), DataValue::Boolean(false), DataValue::Int(0)]);
        let entity = Entity::new(registry_id(registry::ENTITY_TYPE, entity_type), DIMENSIONS, data);
        let container = matches!(kind, BoatKind::ChestBoat | BoatKind::ChestRaft).then(|| ContainerEntity::new(abstract_chest_boat::CONTAINER_SIZE));
        AbstractBoat {
            entity,
            kind,
            container,
            paddle_positions: [0.0; 2],
            out_of_control_ticks: 0.0,
            delta_rotation: 0.0,
            water_level: 0.0,
            land_friction: 0.0,
            status: None,
            old_status: None,
            last_yd: 0.0,
            is_above_bubble_column: false,
            bubble_column_direction_is_down: false,
            drop_item,
        }
    }

    pub fn set_initial_pos(&mut self, x: f64, y: f64, z: f64) {
        self.entity.set_pos(Vec3::new(x, y, z));
        self.entity.xo = Vec3::new(x, y, z);
    }

    pub fn get_drop_item(&self) -> Item {
        self.drop_item
    }

    pub fn get_pick_result(&self) -> Item {
        self.drop_item
    }

    fn ride_height(&self, dimensions: EntityDimensions) -> f64 {
        match self.kind {
            BoatKind::Boat | BoatKind::ChestBoat => (dimensions.height / 3.0) as f64,
            BoatKind::Raft | BoatKind::ChestRaft => (dimensions.height * 0.8888889) as f64,
        }
    }

    fn get_single_passenger_x_offset(&self) -> f32 {
        if self.container.is_some() { abstract_chest_boat::SINGLE_PASSENGER_X_OFFSET } else { 0.0 }
    }

    pub fn get_max_passengers(&self) -> usize {
        if self.container.is_some() { abstract_chest_boat::MAX_PASSENGERS } else { 2 }
    }

    pub fn set_paddle_state(&mut self, left: bool, right: bool) {
        self.entity.data.set(DATA_ID_PADDLE_LEFT, DataValue::Boolean(left));
        self.entity.data.set(DATA_ID_PADDLE_RIGHT, DataValue::Boolean(right));
    }

    fn paddle_data(&self, side: usize) -> bool {
        let id = if side == PADDLE_LEFT { DATA_ID_PADDLE_LEFT } else { DATA_ID_PADDLE_RIGHT };
        matches!(self.entity.data.get(id), DataValue::Boolean(true))
    }

    fn set_bubble_time(&mut self, value: i32) {
        self.entity.data.set(DATA_ID_BUBBLE_TIME, DataValue::Int(value));
    }

    fn get_bubble_time(&self) -> i32 {
        match self.entity.data.get(DATA_ID_BUBBLE_TIME) {
            DataValue::Int(v) => *v,
            _ => 0,
        }
    }

    pub fn is_under_water(&self) -> bool {
        matches!(self.status, Some(Status::UnderWater | Status::UnderFlowingWater))
    }

    pub fn has_enough_space_for(&self, entity: &Entity) -> bool {
        entity.dimensions.width < self.entity.dimensions.width
    }
}

pub fn can_vehicle_collide(level: &ServerLevel, vehicle: EntityId, entity: EntityId) -> bool {
    (level.can_be_collided_with(entity, Some(vehicle)) || level.is_pushable_immutable(entity)) && !level.is_passenger_of_same_vehicle(vehicle, entity)
}

impl ServerLevel {
    pub fn boat(&self, id: EntityId) -> &AbstractBoat {
        match self.entities.get(&id) {
            Some(LevelEntity::Boat(boat)) => boat,
            _ => unreachable!("not a boat"),
        }
    }

    pub fn boat_mut(&mut self, id: EntityId) -> &mut AbstractBoat {
        match self.entities.get_mut(&id) {
            Some(LevelEntity::Boat(boat)) => boat,
            _ => unreachable!("not a boat"),
        }
    }

    pub fn is_boat(&self, id: EntityId) -> bool {
        matches!(self.entities.get(&id), Some(LevelEntity::Boat(_)))
    }

    pub fn abstract_boat_passenger_attachment_point(&self, id: EntityId, passenger: EntityId) -> Vec3 {
        let boat = self.boat(id);
        let mut offset = boat.get_single_passenger_x_offset();
        let passengers = &boat.entity.passengers;
        if passengers.len() > 1 {
            let index = passengers.iter().position(|&p| p == passenger);
            offset = if index == Some(0) { 0.2 } else { -0.6 };
            if self.is_animal(passenger) {
                offset += 0.2;
            }
        }
        Vec3::new(0.0, boat.ride_height(boat.entity.dimensions), offset as f64).y_rot(-boat.entity.y_rot * mth::DEG_TO_RAD)
    }

    pub fn abstract_boat_on_above_bubble_column(&mut self, id: EntityId, drag_down: bool, _pos: BlockPos) {
        let boat = self.boat_mut(id);
        boat.is_above_bubble_column = true;
        boat.bubble_column_direction_is_down = drag_down;
        if boat.get_bubble_time() == 0 {
            boat.set_bubble_time(BUBBLE_TIME);
        }
        let under_water = self.boat(id).is_under_water();
        if !under_water && self.entity_mut(id).random.next_int_bounded(100) == 0 {
            let e = self.entity_mut(id);
            e.random.next_float();
            e.random.next_float();
            e.random.next_float();
        }
    }

    pub fn abstract_boat_push(&mut self, id: EntityId, entity: EntityId) {
        let (bb, other_bb) = (self.entity(id).bounding_box(), self.entity(entity).bounding_box());
        if self.is_boat(entity) {
            if other_bb.min_y < bb.max_y {
                self.entity_push_entity(id, entity);
            }
        } else if other_bb.min_y <= bb.min_y {
            self.entity_push_entity(id, entity);
        }
    }

    pub fn abstract_boat_animate_hurt(&mut self, id: EntityId) {
        let e = self.entity_mut(id);
        let hurt_dir = vehicle_entity::get_hurt_dir(e);
        vehicle_entity::set_hurt_dir(e, -hurt_dir);
        vehicle_entity::set_hurt_time(e, 10);
        let damage = vehicle_entity::get_damage(e);
        vehicle_entity::set_damage(e, damage * 11.0);
    }

    pub fn tick_boat(&mut self, id: EntityId) {
        let status = self.get_status(id);
        let boat = self.boat_mut(id);
        boat.old_status = boat.status;
        boat.status = Some(status);
        if status != Status::UnderWater && status != Status::UnderFlowingWater {
            boat.out_of_control_ticks = 0.0;
        } else {
            boat.out_of_control_ticks += 1.0;
        }
        if boat.out_of_control_ticks >= TIME_TO_EJECT {
            self.eject_passengers(id);
        }
        let e = self.entity_mut(id);
        let hurt_time = vehicle_entity::get_hurt_time(e);
        if hurt_time > 0 {
            vehicle_entity::set_hurt_time(e, hurt_time - 1);
        }
        let damage = vehicle_entity::get_damage(e);
        if damage > 0.0 {
            vehicle_entity::set_damage(e, damage - 1.0);
        }
        self.entity_base_tick(id);
        if self.entity(id).is_removed() {
            return;
        }
        if self.is_local_instance_authoritative(id) {
            if !self.first_passenger(id).is_some_and(|p| self.is_player(p)) {
                self.boat_mut(id).set_paddle_state(false, false);
            }
            self.float_boat(id);
            let delta = self.entity(id).delta_movement;
            self.move_entity(id, MoverType::SelfMove, delta);
            if self.entity_ref(id).is_none_or(Entity::is_removed) {
                return;
            }
        } else {
            let boat = self.boat_mut(id);
            boat.entity.set_delta_movement(Vec3::ZERO);
            boat.delta_rotation = 0.0;
        }
        self.apply_effects_from_blocks(id);
        if self.entity_ref(id).is_none_or(Entity::is_removed) {
            return;
        }
        self.apply_effects_from_blocks(id);
        if self.entity_ref(id).is_none_or(Entity::is_removed) {
            return;
        }
        self.tick_bubble_column(id);
        for i in PADDLE_LEFT..=PADDLE_RIGHT {
            if self.get_paddle_state(id, i) {
                let boat = self.boat(id);
                let position = boat.paddle_positions[i];
                if !boat.entity.is_silent()
                    && position % (std::f64::consts::PI as f32 * 2.0) <= PADDLE_SOUND_TIME
                    && (position + PADDLE_SPEED) % (std::f64::consts::PI as f32 * 2.0) >= PADDLE_SOUND_TIME
                    && let Some(sound) = self.get_paddle_sound(id)
                {
                    let e = self.entity_mut(id);
                    let view_vector = e.view_vector();
                    let dx = if i == 1 { -view_vector.z } else { view_vector.z };
                    let dz = if i == 1 { view_vector.x } else { -view_vector.x };
                    let pitch = 0.8 + 0.4 * e.random.next_float();
                    let (x, y, z) = (e.x() + dx, e.y(), e.z() + dz);
                    let source = self.sound_source(id);
                    self.play_sound_at(None, x, y, z, sound_event(sound), source, 1.0, pitch);
                }
                self.boat_mut(id).paddle_positions[i] += PADDLE_SPEED;
            } else {
                self.boat_mut(id).paddle_positions[i] = 0.0;
            }
        }
        let area = self.entity(id).bounding_box().inflate(0.2f32 as f64, -0.01f32 as f64, 0.2f32 as f64);
        let entities: Vec<EntityId> = self.get_entities(Some(id), &area).into_iter().filter(|&e| self.pushable_by(id, e)).collect();
        if entities.is_empty() {
            return;
        }
        let add_new_passengers = !self.controlling_passenger(id).is_some_and(|p| self.is_player(p));
        for entity in entities {
            if self.has_passenger(entity, id) {
                continue;
            }
            let boat = self.boat(id);
            let start_riding = add_new_passengers
                && boat.entity.passengers.len() < boat.get_max_passengers()
                && !self.is_passenger(entity)
                && boat.has_enough_space_for(self.entity(entity))
                && self.is_living(entity)
                && !entity_type_tags::CANNOT_BE_PUSHED_ONTO_BOATS.contains(self.entity(entity).entity_type as usize);
            if start_riding {
                self.start_riding(entity, id, false, true);
            } else {
                self.push_entity(id, entity);
            }
        }
    }

    fn tick_bubble_column(&mut self, id: EntityId) {
        let boat = self.boat_mut(id);
        if !boat.is_above_bubble_column {
            boat.set_bubble_time(0);
        }
        let mut bubble_time = boat.get_bubble_time();
        if bubble_time > 0 {
            bubble_time -= 1;
            boat.set_bubble_time(bubble_time);
            let diff = BUBBLE_TIME - bubble_time - 1;
            if diff > 0 && bubble_time == 0 {
                boat.set_bubble_time(0);
                let down = boat.bubble_column_direction_is_down;
                if down {
                    self.eject_passengers(id);
                    self.broadcast_entity_event(id, 71);
                } else {
                    self.broadcast_entity_event(id, 72);
                }
                self.handle_bubble_column_effect(id, down);
            }
            self.boat_mut(id).is_above_bubble_column = false;
        }
    }

    fn handle_bubble_column_effect(&mut self, id: EntityId, drag_down: bool) {
        if self.can_simulate_movement(id) {
            let e = self.entity_mut(id);
            let movement = e.delta_movement;
            if drag_down {
                e.set_delta_movement(movement.add(0.0, -0.7, 0.0));
            } else {
                e.set_delta_movement(Vec3::new(movement.x, 0.6, movement.z));
            }
        }
    }

    fn get_paddle_sound(&mut self, id: EntityId) -> Option<&'static str> {
        match self.get_status(id) {
            Status::InWater | Status::UnderWater | Status::UnderFlowingWater => Some("minecraft:entity.boat.paddle_water"),
            Status::OnLand => Some("minecraft:entity.boat.paddle_land"),
            Status::InAir => None,
        }
    }

    pub fn get_paddle_state(&self, id: EntityId, side: usize) -> bool {
        self.boat(id).paddle_data(side) && self.controlling_passenger(id).is_some()
    }

    fn get_status(&mut self, id: EntityId) -> Status {
        if let Some(water_status) = self.is_underwater(id) {
            let max_y = self.entity(id).bounding_box().max_y;
            self.boat_mut(id).water_level = max_y;
            return water_status;
        }
        if self.check_in_water(id) {
            return Status::InWater;
        }
        let friction = self.get_ground_friction(id);
        if friction > 0.0 {
            self.boat_mut(id).land_friction = friction;
            Status::OnLand
        } else {
            Status::InAir
        }
    }

    fn get_water_level_above(&mut self, id: EntityId) -> f32 {
        let aabb = self.entity(id).bounding_box();
        let min_x = mth::floor(aabb.min_x);
        let max_x = mth::ceil(aabb.max_x);
        let min_y = mth::floor(aabb.max_y);
        let max_y = mth::ceil(aabb.max_y - self.boat(id).last_yd);
        let min_z = mth::floor(aabb.min_z);
        let max_z = mth::ceil(aabb.max_z);
        'rows: for y in min_y..max_y {
            let mut block_height = 0.0f32;
            for x in min_x..max_x {
                for z in min_z..max_z {
                    let pos = BlockPos::new(x, y, z);
                    let fluid_state = self.fluid_state(pos);
                    if fluid_state.is(&fluid_tags::WATER) {
                        block_height = block_height.max(self.fluid_height(fluid_state, pos));
                    }
                    if block_height >= 1.0 {
                        continue 'rows;
                    }
                }
            }
            if block_height < 1.0 {
                return y as f32 + block_height;
            }
        }
        (max_y + 1) as f32
    }

    fn get_ground_friction(&mut self, id: EntityId) -> f32 {
        let bb = self.entity(id).bounding_box();
        let box_ = AABB::new(bb.min_x, bb.min_y - 0.001, bb.min_z, bb.max_x, bb.min_y, bb.max_z);
        let x0 = mth::floor(box_.min_x) - 1;
        let x1 = mth::ceil(box_.max_x) + 1;
        let y0 = mth::floor(box_.min_y) - 1;
        let y1 = mth::ceil(box_.max_y) + 1;
        let z0 = mth::floor(box_.min_z) - 1;
        let z1 = mth::ceil(box_.max_z) + 1;
        let boat_shape = VoxelShape::from_boxes(&[box_]);
        let mut friction = 0.0f32;
        let mut count = 0;
        for x in x0..x1 {
            for z in z0..z1 {
                let edges = (if x != x0 && x != x1 - 1 { 0 } else { 1 }) + (if z != z0 && z != z1 - 1 { 0 } else { 1 });
                if edges == 2 {
                    continue;
                }
                for y in y0..y1 {
                    if edges <= 0 || y != y0 && y != y1 - 1 {
                        let block_pos = BlockPos::new(x, y, z);
                        let block_state = self.block_state(block_pos);
                        if !block_state.block().is_instance_of("LilyPadBlock")
                            && join_is_not_empty(&block_state.collision_voxel_shape().moved(x as f64, y as f64, z as f64), &boat_shape, BooleanOp::And)
                        {
                            friction += block_state.block().info().friction;
                            count += 1;
                        }
                    }
                }
            }
        }
        friction / count as f32
    }

    fn check_in_water(&mut self, id: EntityId) -> bool {
        let bb = self.entity(id).bounding_box();
        let min_x = mth::floor(bb.min_x);
        let max_x = mth::ceil(bb.max_x);
        let min_y = mth::floor(bb.min_y);
        let max_y = mth::ceil(bb.min_y + 0.001);
        let min_z = mth::floor(bb.min_z);
        let max_z = mth::ceil(bb.max_z);
        let mut in_water = false;
        let mut water_level = -f64::MAX;
        for x in min_x..max_x {
            for y in min_y..max_y {
                for z in min_z..max_z {
                    let pos = BlockPos::new(x, y, z);
                    let fluid_state = self.fluid_state(pos);
                    if fluid_state.is(&fluid_tags::WATER) {
                        let height = y as f32 + self.fluid_height(fluid_state, pos);
                        water_level = (height as f64).max(water_level);
                        in_water |= bb.min_y < height as f64;
                    }
                }
            }
        }
        self.boat_mut(id).water_level = water_level;
        in_water
    }

    fn is_underwater(&mut self, id: EntityId) -> Option<Status> {
        let aabb = self.entity(id).bounding_box();
        let max_y = aabb.max_y + 0.001;
        let x0 = mth::floor(aabb.min_x);
        let x1 = mth::ceil(aabb.max_x);
        let y0 = mth::floor(aabb.max_y);
        let y1 = mth::ceil(max_y);
        let z0 = mth::floor(aabb.min_z);
        let z1 = mth::ceil(aabb.max_z);
        let mut under_water = false;
        for x in x0..x1 {
            for y in y0..y1 {
                for z in z0..z1 {
                    let pos = BlockPos::new(x, y, z);
                    let fluid_state = self.fluid_state(pos);
                    if fluid_state.is(&fluid_tags::WATER) && max_y < (pos.y as f32 + self.fluid_height(fluid_state, pos)) as f64 {
                        if !fluid_state.is_source() {
                            return Some(Status::UnderFlowingWater);
                        }
                        under_water = true;
                    }
                }
            }
        }
        under_water.then_some(Status::UnderWater)
    }

    fn float_boat(&mut self, id: EntityId) {
        let mut vspeed = -self.gravity(id);
        let mut buoyancy = 0.0;
        let mut inv_friction = 0.05f32;
        let (old_status, status) = (self.boat(id).old_status, self.boat(id).status);
        if old_status == Some(Status::InAir) && status != Some(Status::InAir) && status != Some(Status::OnLand) {
            let e = self.entity(id);
            let water_level = e.y() + e.dimensions.height as f64 * 1.0;
            self.boat_mut(id).water_level = water_level;
            let target_y = self.get_water_level_above(id) as f64 - self.entity(id).dimensions.height as f64 + 0.101;
            let e = self.entity(id);
            let moved = e.bounding_box().move_by(0.0, target_y - e.y(), 0.0);
            if self.no_collision_for(id, &moved) {
                let e = self.entity_mut(id);
                e.set_pos(Vec3::new(e.x(), target_y, e.z()));
                e.set_delta_movement(e.delta_movement.multiply(1.0, 0.0, 1.0));
                self.entity_moved(id);
                self.boat_mut(id).last_yd = 0.0;
            }
            self.boat_mut(id).status = Some(Status::InWater);
            return;
        }
        match status {
            Some(Status::InWater) => {
                let boat = self.boat(id);
                buoyancy = (boat.water_level - boat.entity.y()) / boat.entity.dimensions.height as f64;
                inv_friction = 0.9;
            }
            Some(Status::UnderFlowingWater) => {
                vspeed = -7.0e-4;
                inv_friction = 0.9;
            }
            Some(Status::UnderWater) => {
                buoyancy = 0.01f32 as f64;
                inv_friction = 0.45;
            }
            Some(Status::InAir) => inv_friction = 0.9,
            Some(Status::OnLand) => {
                inv_friction = self.boat(id).land_friction;
                if self.controlling_passenger(id).is_some_and(|p| self.is_player(p)) {
                    self.boat_mut(id).land_friction /= 2.0;
                }
            }
            None => {}
        }
        let boat = self.boat_mut(id);
        let movement = boat.entity.delta_movement;
        boat.entity.set_delta_movement(Vec3::new(movement.x * inv_friction as f64, movement.y + vspeed, movement.z * inv_friction as f64));
        boat.delta_rotation *= inv_friction;
        if buoyancy > 0.0 {
            let delta_movement = boat.entity.delta_movement;
            boat.entity.set_delta_movement(Vec3::new(delta_movement.x, (delta_movement.y + buoyancy * (0.04 / 0.65)) * 0.75, delta_movement.z));
        }
    }

    pub fn abstract_boat_position_rider(&mut self, id: EntityId, passenger: EntityId) {
        if entity_type_tags::CAN_TURN_IN_BOATS.contains(self.entity(passenger).entity_type as usize) {
            return;
        }
        if self.is_local_instance_authoritative(passenger) {
            let delta_rotation = self.boat(id).delta_rotation;
            let p = self.entity_mut(passenger);
            p.set_y_rot(p.y_rot + delta_rotation);
            p.y_head_rot += delta_rotation;
        }
        self.clamp_rotation(id, passenger);
    }

    pub fn abstract_boat_dismount_location_for_passenger(&mut self, id: EntityId, passenger: EntityId) -> Vec3 {
        let e = self.entity(id);
        let passenger_entity = self.entity(passenger);
        let direction = get_collision_horizontal_escape_vector((e.dimensions.width * mth::SQRT_OF_TWO) as f64, passenger_entity.dimensions.width as f64, passenger_entity.y_rot);
        let target_x = e.x() + direction.x;
        let target_z = e.z() + direction.z;
        let target_block_pos = BlockPos::containing(target_x, e.bounding_box().max_y, target_z);
        let below_block_pos = target_block_pos.below();
        if !self.is_water_at(below_block_pos) {
            let mut targets = Vec::new();
            let target_floor = dismount_helper::get_block_floor_height(self, target_block_pos);
            if dismount_helper::is_block_floor_valid(target_floor) {
                targets.push(Vec3::new(target_x, target_block_pos.y as f64 + target_floor, target_z));
            }
            let below_floor = dismount_helper::get_block_floor_height(self, below_block_pos);
            if dismount_helper::is_block_floor_valid(below_floor) {
                targets.push(Vec3::new(target_x, below_block_pos.y as f64 + below_floor, target_z));
            }
            for &dismount_pose in self.dismount_poses(passenger) {
                for &target in &targets {
                    if dismount_helper::can_dismount_to_pose(self, target, passenger, dismount_pose) {
                        self.set_entity_pose(passenger, dismount_pose);
                        return target;
                    }
                }
            }
        }
        self.entity_dismount_location_for_passenger(id, passenger)
    }

    fn clamp_rotation(&mut self, id: EntityId, passenger: EntityId) -> f32 {
        let passenger_body_y_rot = self.calculate_passenger_body_y_rot(id, passenger);
        self.set_y_body_rot(passenger, passenger_body_y_rot);
        if self.is_mob(passenger) {
            self.entity_mut(passenger).set_y_rot(passenger_body_y_rot);
            self.clamp_head_rotation_to_body(passenger);
            mth::wrap_degrees(passenger_body_y_rot - self.entity(passenger).y_rot)
        } else {
            let p = self.entity_mut(passenger);
            let delta = mth::wrap_degrees(p.y_rot - passenger_body_y_rot);
            let target_delta = mth::clamp_f(delta, -105.0, 105.0);
            p.set_y_rot(p.y_rot + target_delta - delta);
            target_delta - delta
        }
    }

    fn calculate_passenger_body_y_rot(&self, id: EntityId, passenger: EntityId) -> f32 {
        let boat = self.boat(id);
        if self.is_animal(passenger) && boat.entity.passengers.len() == boat.get_max_passengers() {
            boat.entity.y_rot + if passenger % 2 == 0 { 90.0 } else { 270.0 }
        } else {
            boat.entity.y_rot
        }
    }

    pub fn abstract_boat_interact(&mut self, id: EntityId, player: EntityId, hand: InteractionHand, location: Vec3) -> InteractionResult {
        let super_interaction = self.entity_interact(id, player, hand, location);
        if super_interaction != InteractionResult::Pass {
            return super_interaction;
        }
        if self.player(player).is_secondary_use_active() || !(self.boat(id).out_of_control_ticks < TIME_TO_EJECT) || !self.start_riding(player, id, false, true) {
            InteractionResult::Pass
        } else {
            InteractionResult::SUCCESS
        }
    }

    pub fn abstract_boat_check_fall_damage(&mut self, id: EntityId, ya: f64, on_ground: bool) {
        let last_yd = self.entity(id).delta_movement.y;
        self.boat_mut(id).last_yd = last_yd;
        if self.is_passenger(id) {
            return;
        }
        if on_ground {
            self.reset_fall_distance(id);
        } else {
            let below = self.entity(id).block_position().below();
            if !self.fluid_state(below).is(&fluid_tags::WATER) && ya < 0.0 {
                self.entity_mut(id).fall_distance -= (ya as f32) as f64;
            }
        }
    }

    pub fn abstract_boat_can_add_passenger(&self, id: EntityId) -> bool {
        let boat = self.boat(id);
        boat.entity.passengers.len() < boat.get_max_passengers() && !boat.entity.is_eye_in_fluid(&fluid_tags::WATER)
    }

    pub fn abstract_boat_controlling_passenger(&self, id: EntityId) -> Option<EntityId> {
        self.first_passenger(id).filter(|&p| self.is_player(p))
    }

    pub fn abstract_boat_modify_passenger_fluid_interaction_box(&self, id: EntityId, passenger_box: AABB) -> Option<AABB> {
        let boat = self.boat(id);
        if boat.is_under_water() {
            return Some(passenger_box);
        }
        let boat_box = boat.entity.bounding_box();
        if boat_box.max_y >= passenger_box.max_y {
            return None;
        }
        let min_y = passenger_box.min_y.max(boat_box.max_y);
        Some(AABB::new(passenger_box.min_x, min_y, passenger_box.min_z, passenger_box.max_x, passenger_box.max_y, passenger_box.max_z))
    }

    pub fn abstract_boat_is_under_water(&self, id: EntityId) -> bool {
        self.boat(id).is_under_water()
    }

    pub fn boat_set_paddle_state(&mut self, id: EntityId, left: bool, right: bool) {
        self.boat_mut(id).set_paddle_state(left, right);
    }
}
