use crisp_core::hash_map::FastMap;

use super::entity_tick_list::LinkedIds;
use crate::world::entity::EntityId;

#[derive(Default)]
pub struct EntityLookup {
    by_id: LinkedIds,
    by_uuid: FastMap<u128, EntityId>,
}

impl EntityLookup {
    pub fn count(&self) -> usize {
        self.by_uuid.len()
    }

    pub fn add(&mut self, id: EntityId, uuid: u128) {
        if let std::collections::hash_map::Entry::Vacant(entry) = self.by_uuid.entry(uuid) {
            entry.insert(id);
            self.by_id.add(id);
        } else {
            warn!("Duplicate entity UUID {}: {id}", crate::world::level::storage::format_uuid(uuid));
        }
    }

    pub fn remove(&mut self, id: EntityId, uuid: u128) {
        self.by_uuid.remove(&uuid);
        self.by_id.remove(id);
    }

    pub fn all_entities(&self) -> impl Iterator<Item = EntityId> + '_ {
        self.by_id.iter()
    }
}
