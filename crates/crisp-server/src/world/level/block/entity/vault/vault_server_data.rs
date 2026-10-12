use crisp_protocol::nbt::Nbt;

use crate::world::entity::entity_save::uuid_from_nbt;
use crate::world::item::ItemStack;
use crate::world::level::storage::uuid_ints;

pub const TAG_NAME: &str = "server_data";
const MAX_REWARD_PLAYERS: usize = 128;

pub fn decode_uuid_linked_set(tag: Option<&Nbt>) -> Vec<u128> {
    let Some(Nbt::List(list)) = tag else { return Vec::new() };
    let Some(uuids) = list.iter().map(uuid_from_nbt).collect::<Option<Vec<u128>>>() else { return Vec::new() };
    let mut set = Vec::with_capacity(uuids.len());
    for uuid in uuids {
        if !set.contains(&uuid) {
            set.push(uuid);
        }
    }
    set
}

pub fn encode_uuid_set(uuids: &[u128]) -> Nbt {
    Nbt::List(uuids.iter().map(|&uuid| uuid_ints(uuid)).collect())
}

#[derive(Clone, Debug, Default)]
pub struct VaultServerData {
    rewarded_players: Vec<u128>,
    state_updating_resumes_at: i64,
    items_to_eject: Vec<ItemStack>,
    last_insert_fail_timestamp: i64,
    total_ejections_needed: i32,
    pub is_dirty: bool,
}

impl VaultServerData {
    pub fn decode(tag: &Nbt) -> Result<VaultServerData, String> {
        let Nbt::Compound(_) = tag else { return Err("Not a map".into()) };
        let items_to_eject = match tag.get("items_to_eject") {
            Some(Nbt::List(list)) => list.iter().map(|item| ItemStack::from_nbt(item).ok()).collect::<Option<Vec<ItemStack>>>().unwrap_or_default(),
            _ => Vec::new(),
        };
        Ok(VaultServerData {
            rewarded_players: decode_uuid_linked_set(tag.get("rewarded_players")),
            state_updating_resumes_at: tag.get("state_updating_resumes_at").and_then(crate::world::level::block::entity::long_value).unwrap_or(0),
            items_to_eject,
            last_insert_fail_timestamp: 0,
            total_ejections_needed: tag.get("total_ejections_needed").and_then(crate::world::level::block::entity::int_value).unwrap_or(0),
            is_dirty: false,
        })
    }

    pub fn encode(&self) -> Nbt {
        let mut output = Vec::new();
        if !self.rewarded_players.is_empty() {
            output.push(("rewarded_players".into(), encode_uuid_set(&self.rewarded_players)));
        }
        if self.state_updating_resumes_at != 0 {
            output.push(("state_updating_resumes_at".into(), Nbt::Long(self.state_updating_resumes_at)));
        }
        if !self.items_to_eject.is_empty() {
            output.push(("items_to_eject".into(), Nbt::List(self.items_to_eject.iter().map(ItemStack::to_nbt).collect())));
        }
        if self.total_ejections_needed != 0 {
            output.push(("total_ejections_needed".into(), Nbt::Int(self.total_ejections_needed)));
        }
        Nbt::Compound(output)
    }

    pub fn set_last_insert_fail_timestamp(&mut self, last_insert_fail_timestamp: i64) {
        self.last_insert_fail_timestamp = last_insert_fail_timestamp;
    }

    pub fn get_last_insert_fail_timestamp(&self) -> i64 {
        self.last_insert_fail_timestamp
    }

    pub fn get_rewarded_players(&self) -> &[u128] {
        &self.rewarded_players
    }

    pub fn has_rewarded_player(&self, player: u128) -> bool {
        self.rewarded_players.contains(&player)
    }

    pub fn add_to_rewarded_players(&mut self, player: u128) {
        if !self.rewarded_players.contains(&player) {
            self.rewarded_players.push(player);
        }
        if self.rewarded_players.len() > MAX_REWARD_PLAYERS {
            self.rewarded_players.remove(0);
        }
        self.mark_changed();
    }

    pub fn state_updating_resumes_at(&self) -> i64 {
        self.state_updating_resumes_at
    }

    pub fn pause_state_updating_until(&mut self, state_updating_resumes_at: i64) {
        self.state_updating_resumes_at = state_updating_resumes_at;
        self.mark_changed();
    }

    pub fn get_items_to_eject(&self) -> &[ItemStack] {
        &self.items_to_eject
    }

    pub fn mark_ejection_finished(&mut self) {
        self.total_ejections_needed = 0;
        self.mark_changed();
    }

    pub fn set_items_to_eject(&mut self, new_items_to_eject: Vec<ItemStack>) {
        self.items_to_eject = new_items_to_eject;
        self.total_ejections_needed = self.items_to_eject.len() as i32;
        self.mark_changed();
    }

    pub fn get_next_item_to_eject(&self) -> ItemStack {
        self.items_to_eject.last().cloned().unwrap_or_else(ItemStack::empty)
    }

    pub fn pop_next_item_to_eject(&mut self) -> ItemStack {
        if self.items_to_eject.is_empty() {
            return ItemStack::empty();
        }
        self.mark_changed();
        self.items_to_eject.pop().unwrap_or_else(ItemStack::empty)
    }

    pub fn set(&mut self, from: &VaultServerData) {
        self.state_updating_resumes_at = from.state_updating_resumes_at;
        self.items_to_eject.clone_from(&from.items_to_eject);
        self.rewarded_players.clone_from(&from.rewarded_players);
    }

    fn mark_changed(&mut self) {
        self.is_dirty = true;
    }

    pub fn ejection_progress(&self) -> f32 {
        if self.total_ejections_needed == 1 {
            1.0
        } else {
            1.0 - crisp_core::mth::inverse_lerp_f(self.items_to_eject.len() as f32, 1.0, self.total_ejections_needed as f32)
        }
    }
}
