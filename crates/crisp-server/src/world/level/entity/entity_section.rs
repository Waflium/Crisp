use super::visibility::Visibility;
use crate::world::entity::EntityId;

#[derive(Default)]
pub struct EntitySection {
    storage: Vec<EntityId>,
    chunk_status: Visibility,
}

impl EntitySection {
    pub fn new(chunk_status: Visibility) -> EntitySection {
        EntitySection { storage: Vec::new(), chunk_status }
    }

    pub fn add(&mut self, entity: EntityId) {
        self.storage.push(entity);
    }

    pub fn remove(&mut self, entity: EntityId) -> bool {
        match self.storage.iter().position(|&e| e == entity) {
            Some(i) => {
                self.storage.remove(i);
                true
            }
            None => false,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.storage.is_empty()
    }

    pub fn entities(&self) -> &[EntityId] {
        &self.storage
    }

    pub fn status(&self) -> Visibility {
        self.chunk_status
    }

    pub fn update_chunk_status(&mut self, chunk_status: Visibility) -> Visibility {
        std::mem::replace(&mut self.chunk_status, chunk_status)
    }
}
