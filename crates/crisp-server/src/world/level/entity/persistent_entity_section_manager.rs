use crisp_core::fastutil::{Long2ObjectOpenHashMap, LongOpenHashSet};
use crisp_core::hash_map::{FastMap, FastSet};
use crisp_core::{BlockPos, ChunkPos, SectionPos};

use super::entity_lookup::EntityLookup;
use super::entity_section::EntitySection;
use super::entity_section_storage::EntitySectionStorage;
use super::visibility::Visibility;
use crate::server::level::ServerLevel;
use crate::server::level::chunk_level::FullChunkStatus;
use crate::world::entity::{EntityId, LevelEntity, RemovalReason};
use crate::world::level::chunk::storage::RegionStorage;
use crate::world::level::chunk::storage::entity_storage::EntityStorage;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
enum ChunkLoadStatus {
    #[default]
    Fresh,
    Pending,
    Loaded,
}

pub struct PersistentEntitySectionManager {
    known_uuids: FastSet<u128>,
    permanent_storage: EntityStorage,
    pub visible_entity_storage: EntityLookup,
    pub section_storage: EntitySectionStorage,
    chunk_visibility: FastMap<i64, Visibility>,
    chunk_load_statuses: Long2ObjectOpenHashMap<ChunkLoadStatus>,
    chunks_to_unload: LongOpenHashSet,
    callbacks: FastMap<EntityId, i64>,
}

fn section_key(pos: BlockPos) -> i64 {
    SectionPos::as_long(pos.x >> 4, pos.y >> 4, pos.z >> 4)
}

impl PersistentEntitySectionManager {
    pub fn new(storage: Option<RegionStorage>) -> PersistentEntitySectionManager {
        PersistentEntitySectionManager {
            known_uuids: FastSet::default(),
            permanent_storage: EntityStorage::new(storage),
            visible_entity_storage: EntityLookup::default(),
            section_storage: EntitySectionStorage::default(),
            chunk_visibility: FastMap::default(),
            chunk_load_statuses: Long2ObjectOpenHashMap::new(),
            chunks_to_unload: LongOpenHashSet::new(),
            callbacks: FastMap::default(),
        }
    }

    fn visibility(&self, chunk_key: i64) -> Visibility {
        self.chunk_visibility.get(&chunk_key).copied().unwrap_or_default()
    }

    fn load_status(&self, chunk_key: i64) -> ChunkLoadStatus {
        self.chunk_load_statuses.get(chunk_key).copied().unwrap_or_default()
    }

    fn add_entity_uuid(&mut self, uuid: u128) -> bool {
        if self.known_uuids.insert(uuid) {
            true
        } else {
            warn!("UUID of added entity already exists: {}", crate::world::level::storage::format_uuid(uuid));
            false
        }
    }

    pub fn add_player_uuid(&mut self, uuid: u128) {
        self.known_uuids.insert(uuid);
    }

    pub fn remove_player_uuid(&mut self, uuid: u128) {
        self.known_uuids.remove(&uuid);
    }

    fn get_or_create_section(&mut self, key: i64) -> &mut EntitySection {
        let PersistentEntitySectionManager { section_storage, chunk_visibility, .. } = self;
        section_storage.get_or_create_section(key, |chunk| chunk_visibility.get(&chunk).copied().unwrap_or_default())
    }

    fn remove_section_if_empty(&mut self, key: i64) {
        if self.section_storage.get_section(key).is_some_and(EntitySection::is_empty) {
            self.section_storage.remove(key);
        }
    }

    pub fn is_ticking(&self, pos: ChunkPos) -> bool {
        self.visibility(pos.pack()).is_ticking()
    }

    fn ensure_chunk_queued_for_load(&mut self, chunk_key: i64) {
        if self.load_status(chunk_key) == ChunkLoadStatus::Fresh {
            self.request_chunk_load(chunk_key);
        }
    }

    fn request_chunk_load(&mut self, chunk_key: i64) {
        self.chunk_load_statuses.insert(chunk_key, ChunkLoadStatus::Pending);
        self.permanent_storage.load_entities(ChunkPos::unpack(chunk_key));
    }

    fn all_chunks_to_save(&self) -> LongOpenHashSet {
        let mut result = self.section_storage.all_chunks_with_existing_sections();
        for (chunk_key, &status) in self.chunk_load_statuses.iter() {
            if status == ChunkLoadStatus::Loaded {
                result.add(chunk_key);
            }
        }
        result
    }

    pub fn is_loaded(&self, uuid: u128) -> bool {
        self.known_uuids.contains(&uuid)
    }

    pub fn are_entities_loaded(&self, chunk_key: i64) -> bool {
        self.load_status(chunk_key) == ChunkLoadStatus::Loaded
    }
}

fn should_be_saved(entity: &LevelEntity) -> bool {
    entity.entity().removal.is_none_or(RemovalReason::should_save)
}

impl ServerLevel {
    fn start_tracking(&mut self, id: EntityId) {
        let entity = &self.entities[&id];
        let (uuid, config) = (entity.entity().uuid, entity.tracking_config());
        self.entity_manager.visible_entity_storage.add(id, uuid);
        self.track_entity(id, config);
    }

    fn stop_tracking(&mut self, id: EntityId) {
        self.untrack_entity(id);
        let uuid = self.entities[&id].entity().uuid;
        self.entity_manager.visible_entity_storage.remove(id, uuid);
    }

    fn start_ticking(&mut self, id: EntityId) {
        self.tick_list.add(id);
    }

    fn stop_ticking(&mut self, id: EntityId) {
        self.tick_list.remove(id);
    }

    pub(crate) fn add_entity(&mut self, entity: LevelEntity) -> bool {
        let e = entity.entity();
        if !self.entity_manager.add_entity_uuid(e.uuid) {
            return false;
        }
        let (id, key) = (e.id, section_key(e.block_position()));
        let section = self.entity_manager.get_or_create_section(key);
        section.add(id);
        let status = section.status();
        self.entity_manager.callbacks.insert(id, key);
        self.entities.insert(id, entity);
        if status.is_accessible() {
            self.start_tracking(id);
        }
        if status.is_ticking() {
            self.start_ticking(id);
        }
        true
    }

    pub fn update_chunk_status(&mut self, pos: ChunkPos, full_status: FullChunkStatus) {
        let status = Visibility::from_full_chunk_status(full_status);
        let chunk_key = pos.pack();
        let manager = &mut self.entity_manager;
        if status == Visibility::Hidden {
            manager.chunk_visibility.remove(&chunk_key);
            manager.chunks_to_unload.add(chunk_key);
        } else {
            manager.chunk_visibility.insert(chunk_key, status);
            manager.chunks_to_unload.remove(chunk_key);
            manager.ensure_chunk_queued_for_load(chunk_key);
        }
        let sections: Vec<i64> = manager.section_storage.existing_section_positions_in_chunk(chunk_key).collect();
        let mut entities = Vec::new();
        for key in sections {
            let Some(section) = self.entity_manager.section_storage.get_section_mut(key) else { continue };
            let previous = section.update_chunk_status(status);
            entities.clear();
            entities.extend(section.entities().iter().filter(|&&id| !self.players.contains_key(&id)));
            let (was_accessible, is_accessible) = (previous.is_accessible(), status.is_accessible());
            let (was_ticking, is_ticking) = (previous.is_ticking(), status.is_ticking());
            if was_ticking && !is_ticking {
                entities.iter().for_each(|&id| self.stop_ticking(id));
            }
            if was_accessible && !is_accessible {
                entities.iter().for_each(|&id| self.stop_tracking(id));
            } else if !was_accessible && is_accessible {
                entities.iter().for_each(|&id| self.start_tracking(id));
            }
            if !was_ticking && is_ticking {
                entities.iter().for_each(|&id| self.start_ticking(id));
            }
        }
    }

    fn store_chunk_sections(&mut self, chunk_key: i64, unload: bool) -> bool {
        let status = self.entity_manager.load_status(chunk_key);
        if status == ChunkLoadStatus::Pending {
            return false;
        }
        let manager = &self.entity_manager;
        let root_entities_to_save: Vec<EntityId> = manager
            .section_storage
            .existing_section_positions_in_chunk(chunk_key)
            .filter_map(|key| manager.section_storage.get_section(key))
            .flat_map(|section| section.entities().iter().copied())
            .filter(|id| self.entities.get(id).is_some_and(should_be_saved))
            .collect();
        let pos = ChunkPos::unpack(chunk_key);
        if root_entities_to_save.is_empty() {
            if status == ChunkLoadStatus::Loaded {
                self.entity_manager.permanent_storage.store_entities(pos, std::iter::empty());
            }
            return true;
        }
        if status == ChunkLoadStatus::Fresh {
            self.entity_manager.request_chunk_load(chunk_key);
            return false;
        }
        let entities = &self.entities;
        self.entity_manager.permanent_storage.store_entities(pos, root_entities_to_save.iter().map(|id| &entities[id]));
        if unload {
            for id in root_entities_to_save {
                self.unload_entity(id);
            }
        }
        true
    }

    fn process_chunk_unload(&mut self, chunk_key: i64) -> bool {
        if !self.store_chunk_sections(chunk_key, true) {
            return false;
        }
        self.entity_manager.chunk_load_statuses.remove(chunk_key);
        true
    }

    fn unload_entity(&mut self, id: EntityId) {
        let entity = self.entities.get_mut(&id).expect("entity to unload").entity_mut();
        entity.removal.get_or_insert(RemovalReason::UnloadedToChunk);
        self.entity_removed(id, Some(RemovalReason::UnloadedToChunk));
        self.entities.swap_remove(&id);
    }

    fn process_unloads(&mut self) {
        let mut chunks_to_unload = std::mem::take(&mut self.entity_manager.chunks_to_unload);
        chunks_to_unload.retain(|chunk_key| self.entity_manager.visibility(chunk_key) == Visibility::Hidden && !self.process_chunk_unload(chunk_key));
        self.entity_manager.chunks_to_unload = chunks_to_unload;
    }

    pub fn process_pending_loads(&mut self) {
        while let Some((pos, read)) = self.entity_manager.permanent_storage.poll_read() {
            let entities = match read {
                Ok(Some(tag)) => self.read_chunk_entities(pos, &tag),
                Ok(None) => {
                    self.entity_manager.permanent_storage.mark_empty(pos);
                    Vec::new()
                }
                Err(e) => {
                    error!("Failed to load entity chunk {pos:?}: {e}");
                    continue;
                }
            };
            for entity in entities {
                self.add_entity(entity);
            }
            self.entity_manager.chunk_load_statuses.insert(pos.pack(), ChunkLoadStatus::Loaded);
        }
    }

    pub fn wait_for_entities_around(&mut self, center: ChunkPos, radius: i32) {
        loop {
            self.process_pending_loads();
            let loaded = (-radius..=radius)
                .all(|dx| (-radius..=radius).all(|dz| self.entity_manager.are_entities_loaded(ChunkPos::new(center.x + dx, center.z + dz).pack())));
            if loaded {
                return;
            }
            self.wait_for_chunk_work(std::time::Duration::from_millis(1));
        }
    }

    pub fn wait_for_entities(&mut self) {
        self.entity_manager.permanent_storage.flush(false);
        self.process_pending_loads();
    }

    pub fn tick_entity_manager(&mut self) {
        self.process_pending_loads();
        self.process_unloads();
    }

    pub fn auto_save_entities(&mut self) {
        let chunks: Vec<i64> = self.entity_manager.all_chunks_to_save().iter().collect();
        for chunk_key in chunks {
            if self.entity_manager.visibility(chunk_key) == Visibility::Hidden {
                self.process_chunk_unload(chunk_key);
            } else {
                self.store_chunk_sections(chunk_key, false);
            }
        }
    }

    pub fn save_all_entities(&mut self) {
        let mut chunks_to_save = self.entity_manager.all_chunks_to_save();
        while !chunks_to_save.is_empty() {
            self.entity_manager.permanent_storage.flush(false);
            self.process_pending_loads();
            chunks_to_save.retain(|chunk_key| {
                let done = if self.entity_manager.visibility(chunk_key) == Visibility::Hidden {
                    self.process_chunk_unload(chunk_key)
                } else {
                    self.store_chunk_sections(chunk_key, false)
                };
                !done
            });
        }
        self.entity_manager.permanent_storage.flush(true);
    }

    pub fn add_player_to_section(&mut self, id: EntityId) {
        let key = section_key(self.players[&id].entity.block_position());
        self.entity_manager.get_or_create_section(key).add(id);
        self.entity_manager.callbacks.insert(id, key);
    }

    pub fn remove_player_from_section(&mut self, id: EntityId) {
        let Some(key) = self.entity_manager.callbacks.remove(&id) else { return };
        if let Some(section) = self.entity_manager.section_storage.get_section_mut(key) {
            section.remove(id);
        }
        self.entity_manager.remove_section_if_empty(key);
    }

    pub fn entity_moved(&mut self, id: EntityId) {
        let Some(&current) = self.entity_manager.callbacks.get(&id) else { return };
        let Some(entity) = self.entity_ref(id) else { return };
        let new_key = section_key(entity.block_position());
        if new_key == current {
            return;
        }
        if self.players.contains_key(&id) {
            self.remove_player_from_section(id);
            self.add_player_to_section(id);
            return;
        }
        let manager = &mut self.entity_manager;
        let section = manager.section_storage.get_section_mut(current).expect("the entity's section");
        let previous = section.status();
        if !section.remove(id) {
            warn!("Entity {id} wasn't found in section {current} (moving to {new_key})");
        }
        manager.remove_section_if_empty(current);
        let section = manager.get_or_create_section(new_key);
        section.add(id);
        let status = section.status();
        manager.callbacks.insert(id, new_key);
        self.update_entity_status(id, previous, status);
    }

    fn update_entity_status(&mut self, id: EntityId, previous: Visibility, status: Visibility) {
        if previous == status {
            return;
        }
        let (was_accessible, is_accessible) = (previous.is_accessible(), status.is_accessible());
        if was_accessible && !is_accessible {
            self.stop_tracking(id);
        } else if !was_accessible && is_accessible {
            self.start_tracking(id);
        }
        let (was_ticking, is_ticking) = (previous.is_ticking(), status.is_ticking());
        if was_ticking && !is_ticking {
            self.stop_ticking(id);
        } else if !was_ticking && is_ticking {
            self.start_ticking(id);
        }
    }

    pub(crate) fn entity_removed(&mut self, id: EntityId, reason: Option<RemovalReason>) {
        let Some(key) = self.entity_manager.callbacks.remove(&id) else { return };
        let Some(section) = self.entity_manager.section_storage.get_section_mut(key) else { return };
        if !section.remove(id) {
            warn!("Entity {id} wasn't found in section {key} (destroying due to {reason:?})");
        }
        let status = section.status();
        if status.is_ticking() {
            self.stop_ticking(id);
        }
        if status.is_accessible() {
            self.stop_tracking(id);
        }
        if reason.is_some_and(RemovalReason::should_destroy) {
            let name = self.scoreboard_name(id);
            self.scoreboard.entity_removed(&name);
        }
        let uuid = self.entities[&id].entity().uuid;
        self.entity_manager.known_uuids.remove(&uuid);
        self.entity_manager.remove_section_if_empty(key);
    }
}

impl PersistentEntitySectionManager {
    pub fn gather_stats(&self) -> String {
        format!(
            "{},{},{},{},{},{},{}",
            self.known_uuids.len(),
            self.visible_entity_storage.count(),
            self.section_storage.count(),
            self.chunk_load_statuses.len(),
            self.chunk_visibility.len(),
            self.permanent_storage.loading_inbox_size(),
            self.chunks_to_unload.len()
        )
    }
}
