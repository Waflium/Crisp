use crisp_core::{BlockPos, ChunkPos};
use crisp_protocol::Writer;

use super::waypoint::Icon;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Type {
    Empty,
    Vec3i(BlockPos),
    Chunk(ChunkPos),
    Azimuth(f32),
}

impl Type {
    fn id(self) -> i32 {
        match self {
            Type::Empty => 0,
            Type::Vec3i(_) => 1,
            Type::Chunk(_) => 2,
            Type::Azimuth(_) => 3,
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct TrackedWaypoint<'a> {
    identifier: u128,
    icon: &'a Icon,
    ty: Type,
}

impl<'a> TrackedWaypoint<'a> {
    pub fn write(&self, w: &mut Writer) {
        w.bool(true);
        w.uuid(self.identifier);
        self.icon.write(w);
        w.varint(self.ty.id());
        self.write_contents(w);
    }

    fn write_contents(&self, w: &mut Writer) {
        match self.ty {
            Type::Empty => {}
            Type::Vec3i(vector) => {
                w.varint(vector.x);
                w.varint(vector.y);
                w.varint(vector.z);
            }
            Type::Chunk(chunk_pos) => {
                w.varint(chunk_pos.x);
                w.varint(chunk_pos.z);
            }
            Type::Azimuth(angle) => w.f32(angle),
        }
    }

    pub fn set_position(identifier: u128, icon: &'a Icon, position: BlockPos) -> TrackedWaypoint<'a> {
        TrackedWaypoint { identifier, icon, ty: Type::Vec3i(position) }
    }

    pub fn set_chunk(identifier: u128, icon: &'a Icon, chunk: ChunkPos) -> TrackedWaypoint<'a> {
        TrackedWaypoint { identifier, icon, ty: Type::Chunk(chunk) }
    }

    pub fn set_azimuth(identifier: u128, icon: &'a Icon, angle: f32) -> TrackedWaypoint<'a> {
        TrackedWaypoint { identifier, icon, ty: Type::Azimuth(angle) }
    }

    pub fn empty(identifier: u128) -> TrackedWaypoint<'static> {
        TrackedWaypoint { identifier, icon: &super::waypoint::NULL, ty: Type::Empty }
    }
}
