use crisp_core::{BlockPos, ChunkPos};
use crisp_data::packets::play::clientbound;

use crate::network::connection::encode;
use crate::world::waypoints::tracked_waypoint::TrackedWaypoint;
use crate::world::waypoints::waypoint::Icon;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Operation {
    Track,
    Untrack,
    Update,
}

pub struct ClientboundTrackedWaypointPacket<'a> {
    operation: Operation,
    waypoint: TrackedWaypoint<'a>,
}

impl ClientboundTrackedWaypointPacket<'_> {
    pub fn remove_waypoint(identifier: u128) -> ClientboundTrackedWaypointPacket<'static> {
        ClientboundTrackedWaypointPacket { operation: Operation::Untrack, waypoint: TrackedWaypoint::empty(identifier) }
    }

    pub fn add_waypoint_position(identifier: u128, icon: &Icon, position: BlockPos) -> ClientboundTrackedWaypointPacket<'_> {
        ClientboundTrackedWaypointPacket { operation: Operation::Track, waypoint: TrackedWaypoint::set_position(identifier, icon, position) }
    }

    pub fn update_waypoint_position(identifier: u128, icon: &Icon, position: BlockPos) -> ClientboundTrackedWaypointPacket<'_> {
        ClientboundTrackedWaypointPacket { operation: Operation::Update, waypoint: TrackedWaypoint::set_position(identifier, icon, position) }
    }

    pub fn add_waypoint_chunk(identifier: u128, icon: &Icon, chunk: ChunkPos) -> ClientboundTrackedWaypointPacket<'_> {
        ClientboundTrackedWaypointPacket { operation: Operation::Track, waypoint: TrackedWaypoint::set_chunk(identifier, icon, chunk) }
    }

    pub fn update_waypoint_chunk(identifier: u128, icon: &Icon, chunk: ChunkPos) -> ClientboundTrackedWaypointPacket<'_> {
        ClientboundTrackedWaypointPacket { operation: Operation::Update, waypoint: TrackedWaypoint::set_chunk(identifier, icon, chunk) }
    }

    pub fn add_waypoint_azimuth(identifier: u128, icon: &Icon, angle: f32) -> ClientboundTrackedWaypointPacket<'_> {
        ClientboundTrackedWaypointPacket { operation: Operation::Track, waypoint: TrackedWaypoint::set_azimuth(identifier, icon, angle) }
    }

    pub fn update_waypoint_azimuth(identifier: u128, icon: &Icon, angle: f32) -> ClientboundTrackedWaypointPacket<'_> {
        ClientboundTrackedWaypointPacket { operation: Operation::Update, waypoint: TrackedWaypoint::set_azimuth(identifier, icon, angle) }
    }

    pub fn encode(&self) -> Vec<u8> {
        encode(clientbound::WAYPOINT, |w| {
            w.varint(self.operation as i32);
            self.waypoint.write(w);
        })
    }
}
