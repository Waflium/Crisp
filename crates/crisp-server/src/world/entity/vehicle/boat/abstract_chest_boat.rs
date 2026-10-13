use crisp_core::Vec3;

use crate::server::level::ServerLevel;
use crate::world::container::ContainerRef;
use crate::world::containers;
use crate::world::damagesource::DamageSource;
use crate::world::entity::{EntityId, RemovalReason};
use crate::world::inventory::menu_provider::MenuProvider;
use crate::world::{InteractionHand, InteractionResult};

pub const CONTAINER_SIZE: usize = 27;
pub const SINGLE_PASSENGER_X_OFFSET: f32 = 0.15;
pub const MAX_PASSENGERS: usize = 1;

impl ServerLevel {
    pub fn abstract_chest_boat_destroy(&mut self, id: EntityId, source: &DamageSource) {
        let drop_item = self.vehicle_drop_item(id);
        self.destroy(id, drop_item);
        self.chest_vehicle_destroyed(id, source);
    }

    pub fn abstract_chest_boat_remove(&mut self, id: EntityId, reason: RemovalReason) {
        if reason.should_destroy() {
            containers::drop_contents_for_entity(self, id, ContainerRef::ChestVehicle(id));
        }
        self.set_removed(id, reason);
    }

    pub fn abstract_chest_boat_interact(&mut self, id: EntityId, player: EntityId, hand: InteractionHand, location: Vec3) -> InteractionResult {
        let super_interaction = self.abstract_boat_interact(id, player, hand, location);
        if super_interaction != InteractionResult::Pass {
            return super_interaction;
        }
        if self.can_add_passenger(id, player) && !self.player(player).is_secondary_use_active() {
            return InteractionResult::Pass;
        }
        self.interact_with_container_vehicle(id, player)
    }

    pub fn open_custom_inventory_screen(&mut self, id: EntityId, player: EntityId) {
        self.open_menu(player, MenuProvider::ChestVehicle(id));
    }
}
