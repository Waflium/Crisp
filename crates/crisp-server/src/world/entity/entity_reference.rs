use crisp_protocol::nbt::Nbt;

use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::entity_save::uuid_from_nbt;
use crate::world::level::storage::uuid_ints;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct EntityReference {
    uuid: u128,
}

impl EntityReference {
    pub fn of(uuid: u128) -> EntityReference {
        EntityReference { uuid }
    }

    pub fn get_uuid(self) -> u128 {
        self.uuid
    }

    pub fn get_entity(self, level: &ServerLevel) -> Option<EntityId> {
        level.entity_manager.visible_entity_storage.get(self.uuid).filter(|&id| level.entity_ref(id).is_some_and(|e| !e.is_removed()))
    }

    pub fn store(reference: Option<EntityReference>, output: &mut Vec<(String, Nbt)>, key: &str) {
        if let Some(reference) = reference {
            output.push((key.into(), uuid_ints(reference.get_uuid())));
        }
    }

    pub fn read(input: &Nbt, key: &str) -> Option<EntityReference> {
        input.get(key).and_then(uuid_from_nbt).map(EntityReference::of)
    }

    pub fn get_living_entity(reference: Option<EntityReference>, level: &ServerLevel) -> Option<EntityId> {
        reference?.get_entity(level).filter(|&id| level.is_living(id))
    }
}
