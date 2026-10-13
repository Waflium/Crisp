use crisp_core::Vec3;

use super::abstract_minecart::{AbstractMinecart, MinecartKind};
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::vehicle::vehicle_entity;
use crate::world::{InteractionHand, InteractionResult};

#[derive(Default)]
pub struct Minecart {
    rotation_offset: f32,
    player_rotation_offset: f32,
}

impl Minecart {
    pub fn create() -> AbstractMinecart {
        AbstractMinecart::new("minecraft:minecart", MinecartKind::Minecart(Minecart::default()))
    }
}

impl ServerLevel {
    fn minecart_kind_mut(&mut self, id: EntityId) -> &mut Minecart {
        match &mut self.minecart_mut(id).kind {
            MinecartKind::Minecart(minecart) => minecart,
        }
    }

    pub fn minecart_interact(&mut self, id: EntityId, player: EntityId, _hand: InteractionHand, _location: Vec3) -> InteractionResult {
        if !self.player(player).is_secondary_use_active() && !self.is_vehicle(id) {
            let minecart = self.minecart_kind_mut(id);
            minecart.player_rotation_offset = minecart.rotation_offset;
            if self.start_riding(player, id, false, true) { InteractionResult::SUCCESS_SERVER } else { InteractionResult::CONSUME }
        } else {
            InteractionResult::Pass
        }
    }

    pub fn minecart_activate_minecart(&mut self, id: EntityId, _xt: i32, _yt: i32, _zt: i32, state: bool) {
        if state {
            if self.is_vehicle(id) {
                self.eject_passengers(id);
            }
            let e = self.entity_mut(id);
            if vehicle_entity::get_hurt_time(e) == 0 {
                let hurt_dir = vehicle_entity::get_hurt_dir(e);
                vehicle_entity::set_hurt_dir(e, -hurt_dir);
                vehicle_entity::set_hurt_time(e, 10);
                vehicle_entity::set_damage(e, 50.0);
                e.sync_velocity = true;
            }
        }
    }

    pub fn minecart_tick(&mut self, id: EntityId) {
        self.abstract_minecart_tick(id);
    }
}
