use crisp_data::packets::play::clientbound;
use crisp_protocol::Writer;
use indexmap::IndexMap;

use super::world_border::WorldBorder;
use crate::network::connection::{Outbound, encode};
use crate::world::entity::EntityId;

#[derive(Default)]
pub struct BorderChangeListener {
    connections: IndexMap<EntityId, Outbound>,
}

impl BorderChangeListener {
    pub fn add_connection(&mut self, player: EntityId, connection: Outbound) {
        self.connections.insert(player, connection);
    }

    pub fn remove_connection(&mut self, player: EntityId) {
        self.connections.shift_remove(&player);
    }

    fn broadcast_all(&self, id: i32, body: impl FnOnce(&mut Writer)) {
        if self.connections.is_empty() {
            return;
        }
        let packet = encode(id, body);
        for connection in self.connections.values() {
            connection.send_raw(packet.clone());
        }
    }

    pub fn on_set_size(&self, border: &WorldBorder) {
        self.broadcast_all(clientbound::SET_BORDER_SIZE, |p| p.f64(border.get_lerp_target()));
    }

    pub fn on_lerp_size(&self, border: &WorldBorder) {
        self.broadcast_all(clientbound::SET_BORDER_LERP_SIZE, |p| {
            p.f64(border.get_size());
            p.f64(border.get_lerp_target());
            p.varlong(border.get_lerp_time());
        });
    }

    pub fn on_set_center(&self, border: &WorldBorder) {
        self.broadcast_all(clientbound::SET_BORDER_CENTER, |p| {
            p.f64(border.get_center_x());
            p.f64(border.get_center_z());
        });
    }

    pub fn on_set_warning_time(&self, border: &WorldBorder) {
        self.broadcast_all(clientbound::SET_BORDER_WARNING_DELAY, |p| p.varint(border.get_warning_time()));
    }

    pub fn on_set_warning_blocks(&self, border: &WorldBorder) {
        self.broadcast_all(clientbound::SET_BORDER_WARNING_DISTANCE, |p| p.varint(border.get_warning_blocks()));
    }
}

pub fn write_initialize_border(p: &mut Writer, border: &WorldBorder) {
    p.f64(border.get_center_x());
    p.f64(border.get_center_z());
    p.f64(border.get_size());
    p.f64(border.get_lerp_target());
    p.varlong(border.get_lerp_time());
    p.varint(border.get_absolute_max_size());
    p.varint(border.get_warning_blocks());
    p.varint(border.get_warning_time());
}
