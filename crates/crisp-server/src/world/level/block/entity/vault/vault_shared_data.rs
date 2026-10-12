use crisp_core::BlockPos;
use crisp_protocol::nbt::Nbt;

use super::vault_config::{DEFAULT_DEACTIVATION_RANGE, VaultConfig, double, lenient_item_stack};
use super::vault_server_data::{VaultServerData, decode_uuid_linked_set, encode_uuid_set};
use crate::server::level::ServerLevel;
use crate::util::java_collections::JavaObjectHashSet;
use crate::world::item::ItemStack;

pub const TAG_NAME: &str = "shared_data";

#[derive(Clone, Debug)]
pub struct VaultSharedData {
    display_item: ItemStack,
    connected_players: Vec<u128>,
    connected_particles_range: f64,
    pub is_dirty: bool,
}

impl Default for VaultSharedData {
    fn default() -> VaultSharedData {
        VaultSharedData { display_item: ItemStack::empty(), connected_players: Vec::new(), connected_particles_range: DEFAULT_DEACTIVATION_RANGE, is_dirty: false }
    }
}

impl VaultSharedData {
    pub fn decode(tag: &Nbt) -> Result<VaultSharedData, String> {
        let Nbt::Compound(_) = tag else { return Err("Not a map".into()) };
        Ok(VaultSharedData {
            display_item: lenient_item_stack(tag.get("display_item")),
            connected_players: decode_uuid_linked_set(tag.get("connected_players")),
            connected_particles_range: double(tag.get("connected_particles_range"), DEFAULT_DEACTIVATION_RANGE),
            is_dirty: false,
        })
    }

    pub fn encode(&self) -> Nbt {
        let mut output = Vec::new();
        if !self.display_item.is_empty() {
            output.push(("display_item".into(), self.display_item.to_nbt()));
        }
        if !self.connected_players.is_empty() {
            output.push(("connected_players".into(), encode_uuid_set(&self.connected_players)));
        }
        if self.connected_particles_range != DEFAULT_DEACTIVATION_RANGE {
            output.push(("connected_particles_range".into(), Nbt::Double(self.connected_particles_range)));
        }
        Nbt::Compound(output)
    }

    pub fn get_display_item(&self) -> &ItemStack {
        &self.display_item
    }

    pub fn has_display_item(&self) -> bool {
        !self.display_item.is_empty()
    }

    pub fn set_display_item(&mut self, stack: &ItemStack) {
        if !ItemStack::matches(&self.display_item, stack) {
            self.display_item = stack.copy();
            self.mark_dirty();
        }
    }

    pub fn has_connected_players(&self) -> bool {
        !self.connected_players.is_empty()
    }

    pub fn get_connected_players(&self) -> &[u128] {
        &self.connected_players
    }

    pub fn update_connected_players_within_range(&mut self, level: &ServerLevel, pos: BlockPos, server_data: &VaultServerData, config: &VaultConfig, limit: f64) {
        let mut current_connected_players = JavaObjectHashSet::default();
        for uuid in config.player_detector.detect(level, config.entity_selector, pos, limit) {
            if !server_data.get_rewarded_players().contains(&uuid) {
                current_connected_players.insert(uuid);
            }
        }
        let equal = self.connected_players.len() == current_connected_players.len() && self.connected_players.iter().all(|uuid| current_connected_players.contains(uuid));
        if !equal {
            self.connected_players = current_connected_players.iter().collect();
            self.mark_dirty();
        }
    }

    fn mark_dirty(&mut self) {
        self.is_dirty = true;
    }

    pub fn set(&mut self, from: &VaultSharedData) {
        self.display_item = from.display_item.clone();
        self.connected_players.clone_from(&from.connected_players);
        self.connected_particles_range = from.connected_particles_range;
    }
}
