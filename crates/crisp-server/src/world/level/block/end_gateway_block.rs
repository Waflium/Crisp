use crisp_core::{BlockPos, Vec3};
use crisp_data::{BlockState, Fluid};

use super::BlockBehaviour;
use crate::server::Server;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::inside_block::InsideBlockEffects;
use crate::world::entity::relative::relative;
use crate::world::level::block::portal::{Portal, PortalEntity};
use crate::world::level::portal::teleport_transition::{PostTeleportTransition, TeleportTransition};

pub struct EndGatewayBlock;

impl BlockBehaviour for EndGatewayBlock {
    fn can_be_replaced_by_fluid(&self, _state: BlockState, _fluid: Fluid) -> bool {
        false
    }

    fn trigger_event(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos, b0: i32, b1: i32) -> bool {
        level.get_block_entity(pos).is_some_and(|e| e.trigger_event(b0, b1))
    }

    fn entity_inside(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, entity: EntityId, _effects: &mut InsideBlockEffects, _is_precise: bool) {
        if level.can_use_portal(entity, false) && level.end_gateway_is_cooling_down(pos) == Some(false) {
            level.set_as_inside_portal(entity, Portal::EndGateway, pos);
            level.end_gateway_trigger_cooldown(pos, state);
        }
    }
}

pub fn get_portal_destination(server: &mut Server, current_level: usize, _entity: &PortalEntity, portal_entry_pos: BlockPos) -> Option<TeleportTransition> {
    get_portal_destination_in_level(&mut server.levels[current_level], portal_entry_pos)
}

pub fn get_portal_destination_in_level(current_level: &mut ServerLevel, portal_entry_pos: BlockPos) -> Option<TeleportTransition> {
    let teleport_position = current_level.end_gateway_get_portal_position(portal_entry_pos)?;
    Some(TeleportTransition::with_relatives(
        current_level.index,
        teleport_position,
        Vec3::ZERO,
        0.0,
        0.0,
        relative::DELTA | relative::ROTATION,
        PostTeleportTransition::PLACE_PORTAL_TICKET,
    ))
}
