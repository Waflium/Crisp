use crisp_core::{Vec3, mth};
use crisp_data::packets::play::clientbound;
use crisp_protocol::Writer;

use crate::network::connection::encode;
use crate::world::entity::Entity;
use crate::world::entity::attributes::AttributeMap;
use crate::world::entity::data::{DataValue, write_values};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum UpdateInterval {
    Never,
    Periodic(i32),
}

impl UpdateInterval {
    fn test(self, tick: i32) -> bool {
        match self {
            UpdateInterval::Never => false,
            UpdateInterval::Periodic(period) => tick % period == 0,
        }
    }

    fn next_interval(self, tick: i32) -> i32 {
        match self {
            UpdateInterval::Never => 0,
            UpdateInterval::Periodic(period) => tick / period * period + period,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum PositionPath {
    Linear(Vec3),
    Stepped(Vec<(Vec3, i32)>),
}

impl PositionPath {
    fn end_position(&self) -> Vec3 {
        match self {
            PositionPath::Linear(p) => *p,
            PositionPath::Stepped(steps) => steps.last().unwrap().0,
        }
    }

    fn write(&self, w: &mut Writer) {
        match self {
            PositionPath::Linear(p) => {
                w.varint(0);
                write_vec3(w, *p);
            }
            PositionPath::Stepped(steps) => {
                w.varint(1);
                w.varint(steps.len() as i32);
                for &(p, ticks) in steps {
                    write_vec3(w, p);
                    w.varint(ticks);
                }
            }
        }
    }
}

fn write_vec3(w: &mut Writer, v: Vec3) {
    w.f64(v.x);
    w.f64(v.y);
    w.f64(v.z);
}

#[derive(Default)]
pub struct SteppedInterpolationTracker {
    steps: Vec<(Vec3, i32)>,
    ticks_since_last_step: i32,
    predicted_delta: Vec3,
}

impl SteppedInterpolationTracker {
    fn new() -> SteppedInterpolationTracker {
        SteppedInterpolationTracker { steps: Vec::new(), ticks_since_last_step: -1, predicted_delta: Vec3::ZERO }
    }

    pub fn apply_predicted_movement(&mut self, delta: Vec3) {
        self.predicted_delta = self.predicted_delta.add_vec(delta);
    }

    fn add_step(&mut self, pos: Vec3) {
        if self.ticks_since_last_step > 0 {
            self.steps.push((pos, self.ticks_since_last_step));
            self.ticks_since_last_step = 0;
        }
    }

    fn update_tracking(&mut self, entity: &mut Entity, pos: Vec3) {
        if self.predicted_delta.length_sqr() > 1.0e-5f32 as f64 {
            let delta = self.predicted_delta;
            self.steps.iter_mut().for_each(|s| s.0 = s.0.add_vec(delta));
        }
        self.predicted_delta = Vec3::ZERO;
        self.ticks_since_last_step += 1;
        if entity.sync_position {
            entity.sync_position = false;
            self.add_step(pos);
        }
    }

    fn position_path(&mut self, pos: Vec3) -> PositionPath {
        self.add_step(pos);
        if self.steps.is_empty() { PositionPath::Linear(pos) } else { PositionPath::Stepped(self.steps.clone()) }
    }

    fn clear(&mut self) {
        self.steps.clear();
        self.ticks_since_last_step = 0;
    }
}

fn encode_coordinate(v: f64) -> i64 {
    mth::java_round(v * 4096.0)
}

fn encoding_precision_loss(v: f64) -> f64 {
    encode_coordinate(v) as f64 / 4096.0 - v
}

fn delta_too_big(d: [i64; 3]) -> bool {
    d.iter().any(|&v| !(-32768..=32767).contains(&v))
}

enum VecDelta {
    Linear([i16; 3]),
    Stepped(Vec<([i16; 3], i32)>),
}

impl VecDelta {
    fn step_count(&self) -> i32 {
        match self {
            VecDelta::Linear(_) => 0,
            VecDelta::Stepped(steps) => steps.len() as i32,
        }
    }

    fn has_delta(&self, axis: usize) -> bool {
        match self {
            VecDelta::Linear(d) => d[axis] != 0,
            VecDelta::Stepped(steps) => steps.iter().any(|(d, _)| d[axis] != 0),
        }
    }

    fn write(&self, w: &mut Writer) {
        match self {
            VecDelta::Linear(d) => d.iter().for_each(|&v| w.i16(v)),
            VecDelta::Stepped(steps) => {
                for (d, ticks) in steps {
                    w.varint(*ticks);
                    d.iter().for_each(|&v| w.i16(v));
                }
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Audience {
    Tracking,
    TrackingAndSelf,
}

pub struct ServerEntity {
    update_interval: UpdateInterval,
    track_delta: bool,
    is_item: bool,
    interpolation: Option<SteppedInterpolationTracker>,
    position_base: Vec3,
    last_sent_y_rot: i8,
    last_sent_x_rot: i8,
    last_sent_y_head_rot: i8,
    last_sent_movement: Vec3,
    tick_count: i32,
    teleport_delay: i32,
    was_on_ground: bool,
    tracked_data_values: Option<Vec<(usize, DataValue)>>,
}

impl ServerEntity {
    pub fn new(entity: &Entity, update_interval: UpdateInterval, track_delta: bool, stepped: bool, is_item: bool) -> ServerEntity {
        ServerEntity {
            update_interval,
            track_delta,
            is_item,
            interpolation: stepped.then(SteppedInterpolationTracker::new),
            position_base: entity.position(),
            last_sent_y_rot: mth::pack_degrees(entity.y_rot),
            last_sent_x_rot: mth::pack_degrees(entity.x_rot),
            last_sent_y_head_rot: mth::pack_degrees(entity.y_head_rot),
            last_sent_movement: entity.delta_movement,
            tick_count: 0,
            teleport_delay: 0,
            was_on_ground: entity.on_ground,
            tracked_data_values: entity.data.non_default_values(),
        }
    }

    pub fn interpolation(&mut self) -> Option<&mut SteppedInterpolationTracker> {
        self.interpolation.as_mut()
    }

    fn position_path(&mut self, pos: Vec3) -> PositionPath {
        match &mut self.interpolation {
            Some(tracker) => tracker.position_path(pos),
            None => PositionPath::Linear(pos),
        }
    }

    fn try_encode(&self, path: &PositionPath) -> Option<VecDelta> {
        let delta = |base: Vec3, p: Vec3| {
            [
                encode_coordinate(p.x) - encode_coordinate(base.x),
                encode_coordinate(p.y) - encode_coordinate(base.y),
                encode_coordinate(p.z) - encode_coordinate(base.z),
            ]
        };
        match path {
            PositionPath::Linear(p) => {
                let d = delta(self.position_base, *p);
                (!delta_too_big(d)).then(|| VecDelta::Linear(d.map(|v| v as i16)))
            }
            PositionPath::Stepped(steps) => {
                let mut base = self.position_base;
                let mut out = Vec::new();
                for &(p, ticks) in steps {
                    let d = delta(base, p);
                    if delta_too_big(d) {
                        return None;
                    }
                    out.push((d.map(|v| v as i16), ticks));
                    base = p;
                }
                Some(VecDelta::Stepped(out))
            }
        }
    }

    pub fn send_changes(&mut self, entity: &mut Entity, attributes: Option<&mut AttributeMap>) -> Vec<(Audience, Vec<u8>)> {
        let mut out = Vec::new();
        let current = entity.position();
        if let Some(tracker) = &mut self.interpolation {
            tracker.update_tracking(entity, current);
        }
        if entity.sync_position {
            self.tick_count = self.update_interval.next_interval(self.tick_count);
            entity.sync_position = false;
        }
        if entity.needs_sync || self.update_interval.test(self.tick_count) || entity.data.is_dirty() {
            let y_rot = mth::pack_degrees(entity.y_rot);
            let x_rot = mth::pack_degrees(entity.x_rot);
            let should_send_rotation =
                (y_rot as i32 - self.last_sent_y_rot as i32).abs() >= 1 || (x_rot as i32 - self.last_sent_x_rot as i32).abs() >= 1;
            self.teleport_delay += 1;
            let path = self.position_path(current);
            if let Some(tracker) = &mut self.interpolation {
                tracker.clear();
            }
            let position_changed = current.subtract(self.position_base).length_sqr() >= 7.6293945e-6f32 as f64;
            let should_send_position = position_changed || self.tick_count % 60 == 0;
            let packet = self.create_move_packet(entity, &path, y_rot, x_rot, should_send_position, should_send_rotation);
            if entity.needs_sync || self.track_delta {
                let movement = entity.delta_movement;
                let diff = movement.distance_to_sqr(self.last_sent_movement);
                if diff > 1.0e-7 || diff > 0.0 && movement.length_sqr() == 0.0 {
                    self.last_sent_movement = movement;
                    out.push((Audience::Tracking, set_entity_motion(entity.id, movement)));
                }
            }
            match packet {
                Some((bytes, has_position, has_rotation)) => {
                    out.push((Audience::Tracking, bytes));
                    self.send_dirty_entity_data(entity, attributes, &mut out);
                    if has_position {
                        self.position_base = current;
                    }
                    if has_rotation {
                        self.last_sent_y_rot = y_rot;
                        self.last_sent_x_rot = x_rot;
                    }
                }
                None => self.send_dirty_entity_data(entity, attributes, &mut out),
            }
            let y_head_rot = mth::pack_degrees(entity.y_head_rot);
            if (y_head_rot as i32 - self.last_sent_y_head_rot as i32).abs() >= 1 {
                out.push((
                    Audience::Tracking,
                    encode(clientbound::ROTATE_HEAD, |p| {
                        p.varint(entity.id);
                        p.u8(y_head_rot as u8);
                    }),
                ));
                self.last_sent_y_head_rot = y_head_rot;
            }
            entity.needs_sync = false;
        }
        self.tick_count += 1;
        if entity.sync_velocity {
            entity.sync_velocity = false;
            out.push((Audience::TrackingAndSelf, set_entity_motion(entity.id, entity.delta_movement)));
        }
        out
    }

    fn create_move_packet(
        &mut self,
        entity: &Entity,
        path: &PositionPath,
        y_rot: i8,
        x_rot: i8,
        send_position: bool,
        send_rotation: bool,
    ) -> Option<(Vec<u8>, bool, bool)> {
        let id = entity.id;
        if entity.requires_precise_position || self.teleport_delay > 400 || self.was_on_ground != entity.on_ground {
            self.was_on_ground = entity.on_ground;
            self.teleport_delay = 0;
            return Some((position_sync(entity, path), true, true));
        }
        if send_position {
            let delta = match self.try_encode(path) {
                Some(delta) if !self.is_full_precision_encoding_required(entity, path.end_position(), &delta) => delta,
                _ => return Some((position_sync(entity, path), true, true)),
            };
            let properties = entity.on_ground as i32 | delta.step_count() << 1;
            let packet = if send_rotation {
                encode(clientbound::MOVE_ENTITY_POS_ROT, |p| {
                    p.varint(id);
                    p.varint(properties);
                    delta.write(p);
                    p.u8(y_rot as u8);
                    p.u8(x_rot as u8);
                })
            } else {
                encode(clientbound::MOVE_ENTITY_POS, |p| {
                    p.varint(id);
                    p.varint(properties);
                    delta.write(p);
                })
            };
            Some((packet, true, send_rotation))
        } else if send_rotation {
            Some((rotation(entity.id, y_rot, x_rot, entity.on_ground), false, true))
        } else {
            None
        }
    }

    fn is_full_precision_encoding_required(&self, entity: &Entity, pos: Vec3, delta: &VecDelta) -> bool {
        if !self.is_item {
            return false;
        }
        let (dx, dz) = (delta.has_delta(0), delta.has_delta(2));
        let (loss_x, loss_z) = (encoding_precision_loss(pos.x) != 0.0, encoding_precision_loss(pos.z) != 0.0);
        entity.vertical_collision && (dx && loss_x || dz && loss_z) || entity.horizontal_collision && (dx && loss_z || dz && loss_x)
    }

    fn send_dirty_entity_data(&mut self, entity: &mut Entity, attributes: Option<&mut AttributeMap>, out: &mut Vec<(Audience, Vec<u8>)>) {
        if let Some(values) = entity.data.pack_dirty() {
            self.tracked_data_values = entity.data.non_default_values();
            out.push((Audience::TrackingAndSelf, encode(clientbound::SET_ENTITY_DATA, |p| write_values(p, entity.id, &values))));
        }
        if let Some(attributes) = attributes
            && attributes.has_to_sync()
        {
            out.push((Audience::TrackingAndSelf, encode(clientbound::UPDATE_ATTRIBUTES, |p| attributes.write_to_sync(p, entity.id))));
        }
    }

    pub fn pairing_packets(&self, entity: &Entity) -> Vec<Vec<u8>> {
        let mut packets = vec![encode(clientbound::ADD_ENTITY, |p| {
            p.varint(entity.id);
            p.uuid(entity.uuid);
            p.varint(entity.entity_type);
            write_vec3(p, self.position_base);
            p.lp_vec3(self.last_sent_movement.x, self.last_sent_movement.y, self.last_sent_movement.z);
            p.u8(mth::pack_degrees(mth::unpack_degrees(self.last_sent_x_rot)) as u8);
            p.u8(mth::pack_degrees(mth::unpack_degrees(self.last_sent_y_rot)) as u8);
            p.u8(mth::pack_degrees(mth::unpack_degrees(self.last_sent_y_head_rot)) as u8);
            p.varint(0);
        })];
        if let Some(values) = &self.tracked_data_values {
            packets.push(encode(clientbound::SET_ENTITY_DATA, |p| write_values(p, entity.id, values)));
        }
        packets
    }
}

fn position_sync(entity: &Entity, path: &PositionPath) -> Vec<u8> {
    encode(clientbound::ENTITY_POSITION_SYNC, |p| {
        p.varint(entity.id);
        path.write(p);
        p.f32(entity.y_rot);
        p.f32(entity.x_rot);
        p.bool(entity.on_ground);
    })
}

fn rotation(id: i32, y_rot: i8, x_rot: i8, on_ground: bool) -> Vec<u8> {
    encode(clientbound::MOVE_ENTITY_ROT, |p| {
        p.varint(id);
        p.bool(on_ground);
        p.u8(y_rot as u8);
        p.u8(x_rot as u8);
    })
}

pub fn set_entity_motion(id: i32, movement: Vec3) -> Vec<u8> {
    encode(clientbound::SET_ENTITY_MOTION, |p| {
        p.varint(id);
        p.lp_vec3(movement.x, movement.y, movement.z);
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deltas_match_vanilla() {
        let mut x: f64 = -0.3;
        let mut sent = x;
        let mut deltas = Vec::new();
        for _ in 0..6 {
            x += 0.2;
            x += 0.2;
            deltas.push(encode_coordinate(x) - encode_coordinate(sent));
            sent = x;
        }
        assert_eq!(deltas, [0x667, 0x666, 0x666, 0x667, 0x666, 0x667]);
    }
}
