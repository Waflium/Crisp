use crisp_core::{BlockPos, Vec3};
use crisp_protocol::nbt::Nbt;

use super::{end_portal_block, nether_portal_block};
use crate::server::Server;
use crate::server::level::ServerLevel;
use crate::world::entity::portal_processor::PortalProcessor;
use crate::world::entity::{EntityDimensions, EntityId};
use crate::world::level::portal::teleport_transition::TeleportTransition;
use crate::server::{OVERWORLD, THE_END, THE_NETHER};
use crate::world::phys::shapes::collision_context::CollisionContext;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Portal {
    NetherPortal,
    EndPortal,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Transition {
    Confusion,
    None,
}

impl Portal {
    pub fn get_portal_transition_time(self, level: &ServerLevel, entity: EntityId) -> i32 {
        match self {
            Portal::NetherPortal => nether_portal_block::get_portal_transition_time(level, entity),
            Portal::EndPortal => 0,
        }
    }

    pub fn get_portal_destination(self, server: &mut Server, current_level: usize, entity: &PortalEntity, portal_entry_pos: BlockPos) -> Option<TeleportTransition> {
        match self {
            Portal::NetherPortal => nether_portal_block::get_portal_destination(server, current_level, entity, portal_entry_pos),
            Portal::EndPortal => end_portal_block::get_portal_destination(server, current_level, entity, portal_entry_pos),
        }
    }

    pub fn get_local_transition(self) -> Transition {
        match self {
            Portal::NetherPortal => Transition::Confusion,
            Portal::EndPortal => Transition::None,
        }
    }

    pub fn get_destination_dimension(self, current_dimension: &str) -> Option<&'static str> {
        match self {
            Portal::NetherPortal if current_dimension == THE_NETHER => Some(OVERWORLD),
            Portal::NetherPortal => Some(THE_NETHER),
            Portal::EndPortal if current_dimension == THE_END => None,
            Portal::EndPortal => Some(THE_END),
        }
    }
}

#[derive(Clone, Debug)]
pub struct PortalEntity {
    pub id: EntityId,
    pub entity_type: i32,
    pub position: Vec3,
    pub delta_movement: Vec3,
    pub y_rot: f32,
    pub x_rot: f32,
    pub dimensions: EntityDimensions,
    pub collision_context: CollisionContext,
    pub is_spectator: bool,
    pub portal_cooldown: i32,
    pub portal_process: Option<PortalProcessor>,
    pub data: Option<Vec<(String, Nbt)>>,
}

#[derive(Clone, Debug)]
pub struct PendingPortalTeleport {
    pub entity: PortalEntity,
    pub portal: Portal,
    pub entry_position: BlockPos,
    pub removed: bool,
}
