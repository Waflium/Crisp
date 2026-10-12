use crisp_core::BlockPos;
use crisp_data::{MOB_EFFECTS, MobEffect};

use super::abstract_container_menu::{AbstractContainerMenu, MenuKind};
use crate::server::level::ServerLevel;
use crate::util::Prediction;
use crate::world::entity::EntityId;
use crate::world::level::block::entity::beacon_block_entity::{DATA_LEVELS, DATA_PRIMARY, DATA_SECONDARY, validate_effects};

pub const PAYMENT_SLOT: usize = 0;

pub fn encode_effect(mob_effect: Option<MobEffect>) -> i32 {
    mob_effect.map_or(0, |effect| effect.protocol_id() + 1)
}

pub fn decode_effect(id: i32) -> Option<MobEffect> {
    if id == 0 { None } else { usize::try_from(id - 1).ok().filter(|&id| id < MOB_EFFECTS.len()).map(|id| MobEffect(id as u8)) }
}

impl AbstractContainerMenu {
    pub fn beacon(container_id: i32, pos: BlockPos) -> AbstractContainerMenu {
        let mut menu = AbstractContainerMenu::new(container_id, MenuKind::Beacon, None);
        menu.access = Some(pos);
        menu
    }
}

impl ServerLevel {
    pub fn beacon_menu_removed(&mut self, menu: &mut AbstractContainerMenu, player: EntityId) {
        let item_stack = self.slot_remove(menu, player, PAYMENT_SLOT, 1);
        if !item_stack.is_empty() {
            self.player_drop(player, item_stack, false, Prediction::ServerOnly);
        }
    }

    pub fn beacon_menu_update_effects(&mut self, menu: &mut AbstractContainerMenu, player: EntityId, primary: Option<MobEffect>, secondary: Option<MobEffect>) -> bool {
        if !self.slot_has_item(menu, player, PAYMENT_SLOT) {
            return false;
        }
        let levels = self.menu_data(menu, DATA_LEVELS);
        if !validate_effects(primary, secondary, levels) {
            return false;
        }
        if let Some(pos) = menu.access {
            self.beacon_data_set(pos, DATA_PRIMARY, encode_effect(primary));
            self.beacon_data_set(pos, DATA_SECONDARY, encode_effect(secondary));
        }
        self.slot_remove(menu, player, PAYMENT_SLOT, 1);
        if let Some(pos) = menu.access {
            self.block_entity_changed(pos);
        }
        true
    }
}
