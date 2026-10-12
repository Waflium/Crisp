use crisp_core::{BlockPos, ChunkPos, mth};

use super::waypoint::Icon;
use crate::network::protocol::game::clientbound_tracked_waypoint_packet::ClientboundTrackedWaypointPacket;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::attributes::Attribute;

pub const REALLY_FAR_DISTANCE: i32 = 332;

pub fn does_source_ignore_receiver(level: &ServerLevel, source: EntityId, receiver: EntityId) -> bool {
    if level.is_spectator(receiver) {
        return false;
    }
    if !level.is_spectator(source) {
        let broadcast_range = level.attribute_value(source, Attribute::WaypointTransmitRange).min(level.attribute_value(receiver, Attribute::WaypointReceiveRange));
        level.entity(source).distance_to(level.entity(receiver)) as f64 >= broadcast_range
    } else {
        true
    }
}

pub fn is_chunk_visible(level: &ServerLevel, chunk_pos: ChunkPos, receiver: EntityId) -> bool {
    level.player(receiver).chunk_tracking_view.is_some_and(|view| view.contains_with(chunk_pos.x, chunk_pos.z, false))
}

pub fn is_really_far(level: &ServerLevel, source: EntityId, receiver: EntityId) -> bool {
    level.entity(source).distance_to(level.entity(receiver)) > REALLY_FAR_DISTANCE as f32
}

fn send(level: &ServerLevel, receiver: EntityId, packet: ClientboundTrackedWaypointPacket) {
    if let Some(player) = level.players.get(&receiver) {
        player.connection.send_raw(packet.encode());
    }
}

pub enum Connection {
    EntityAzimuth(EntityAzimuthConnection),
    EntityBlock(EntityBlockConnection),
    EntityChunk(EntityChunkConnection),
}

impl Connection {
    pub fn connect(&self, level: &ServerLevel) {
        match self {
            Connection::EntityAzimuth(connection) => connection.connect(level),
            Connection::EntityBlock(connection) => connection.connect(level),
            Connection::EntityChunk(connection) => connection.connect(level),
        }
    }

    pub fn disconnect(&self, level: &ServerLevel) {
        let (source_uuid, receiver) = match self {
            Connection::EntityAzimuth(connection) => (connection.source_uuid, connection.receiver),
            Connection::EntityBlock(connection) => (connection.source_uuid, connection.receiver),
            Connection::EntityChunk(connection) => (connection.source_uuid, connection.receiver),
        };
        send(level, receiver, ClientboundTrackedWaypointPacket::remove_waypoint(source_uuid));
    }

    pub fn update(&mut self, level: &ServerLevel) {
        match self {
            Connection::EntityAzimuth(connection) => connection.update(level),
            Connection::EntityBlock(connection) => connection.update(level),
            Connection::EntityChunk(connection) => connection.update(level),
        }
    }

    pub fn is_broken(&self, level: &ServerLevel) -> bool {
        match self {
            Connection::EntityAzimuth(connection) => connection.is_broken(level),
            Connection::EntityBlock(connection) => connection.is_broken(level),
            Connection::EntityChunk(connection) => connection.is_broken(level),
        }
    }
}

fn azimuth(level: &ServerLevel, source: EntityId, receiver: EntityId) -> f32 {
    let direction = level.entity(receiver).position().subtract(level.entity(source).position()).rotate_clockwise_90();
    mth::atan2(direction.z, direction.x) as f32
}

pub struct EntityAzimuthConnection {
    source: EntityId,
    source_uuid: u128,
    icon: Icon,
    receiver: EntityId,
    last_angle: f32,
}

impl EntityAzimuthConnection {
    pub fn new(level: &ServerLevel, source: EntityId, icon: Icon, receiver: EntityId) -> EntityAzimuthConnection {
        let last_angle = azimuth(level, source, receiver);
        EntityAzimuthConnection { source, source_uuid: level.entity(source).uuid, icon, receiver, last_angle }
    }

    fn is_broken(&self, level: &ServerLevel) -> bool {
        does_source_ignore_receiver(level, self.source, self.receiver)
            || is_chunk_visible(level, level.entity(self.source).chunk_position(), self.receiver)
            || !is_really_far(level, self.source, self.receiver)
    }

    fn connect(&self, level: &ServerLevel) {
        send(level, self.receiver, ClientboundTrackedWaypointPacket::add_waypoint_azimuth(self.source_uuid, &self.icon, self.last_angle));
    }

    fn update(&mut self, level: &ServerLevel) {
        let current_angle = azimuth(level, self.source, self.receiver);
        if (current_angle - self.last_angle).abs() > 0.008726646 {
            send(level, self.receiver, ClientboundTrackedWaypointPacket::update_waypoint_azimuth(self.source_uuid, &self.icon, current_angle));
            self.last_angle = current_angle;
        }
    }
}

pub struct EntityBlockConnection {
    source: EntityId,
    source_uuid: u128,
    icon: Icon,
    receiver: EntityId,
    last_position: BlockPos,
}

impl EntityBlockConnection {
    pub fn new(level: &ServerLevel, source: EntityId, icon: Icon, receiver: EntityId) -> EntityBlockConnection {
        let e = level.entity(source);
        EntityBlockConnection { source, source_uuid: e.uuid, icon, receiver, last_position: e.block_position() }
    }

    fn connect(&self, level: &ServerLevel) {
        send(level, self.receiver, ClientboundTrackedWaypointPacket::add_waypoint_position(self.source_uuid, &self.icon, self.last_position));
    }

    fn update(&mut self, level: &ServerLevel) {
        let current_position = level.entity(self.source).block_position();
        if current_position.dist_manhattan(self.last_position) > 0 {
            send(level, self.receiver, ClientboundTrackedWaypointPacket::update_waypoint_position(self.source_uuid, &self.icon, current_position));
            self.last_position = current_position;
        }
    }

    fn distance_manhattan(&self, level: &ServerLevel) -> i32 {
        self.last_position.dist_manhattan(level.entity(self.source).block_position())
    }

    fn is_broken(&self, level: &ServerLevel) -> bool {
        self.distance_manhattan(level) > 1 || does_source_ignore_receiver(level, self.source, self.receiver)
    }
}

pub struct EntityChunkConnection {
    source: EntityId,
    source_uuid: u128,
    icon: Icon,
    receiver: EntityId,
    last_position: ChunkPos,
}

impl EntityChunkConnection {
    pub fn new(level: &ServerLevel, source: EntityId, icon: Icon, receiver: EntityId) -> EntityChunkConnection {
        let e = level.entity(source);
        EntityChunkConnection { source, source_uuid: e.uuid, icon, receiver, last_position: e.chunk_position() }
    }

    fn distance_chessboard(&self, level: &ServerLevel) -> i32 {
        self.last_position.chessboard_distance(level.entity(self.source).chunk_position())
    }

    fn connect(&self, level: &ServerLevel) {
        send(level, self.receiver, ClientboundTrackedWaypointPacket::add_waypoint_chunk(self.source_uuid, &self.icon, self.last_position));
    }

    fn update(&mut self, level: &ServerLevel) {
        let current_position = level.entity(self.source).chunk_position();
        if current_position.chessboard_distance(self.last_position) > 0 {
            send(level, self.receiver, ClientboundTrackedWaypointPacket::update_waypoint_chunk(self.source_uuid, &self.icon, current_position));
            self.last_position = current_position;
        }
    }

    fn is_broken(&self, level: &ServerLevel) -> bool {
        if self.distance_chessboard(level) <= 1 && !does_source_ignore_receiver(level, self.source, self.receiver) {
            is_chunk_visible(level, self.last_position, self.receiver)
        } else {
            true
        }
    }
}
