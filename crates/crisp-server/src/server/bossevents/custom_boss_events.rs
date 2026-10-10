use std::io;
use std::path::PathBuf;

use crisp_core::random::LegacyRandomSource;
use crisp_data::WORLD_VERSION;
use crisp_protocol::nbt::Nbt;

use super::custom_boss_event::CustomBossEvent;
use crate::network::chat::Component;
use crate::server::level::ServerLevel;
use crate::util::java_collections::{JavaHashMap, identifier_hash};
use crate::world::entity::{EntityId, create_insecure_uuid};
use crate::world::level::storage::{read_dat, write_dat};

pub struct CustomBossEvents {
    dir: Option<PathBuf>,
    events: JavaHashMap<CustomBossEvent>,
    dirty: bool,
}

impl CustomBossEvents {
    fn file(dir: &std::path::Path) -> PathBuf {
        dir.join("minecraft").join("custom_boss_events.dat")
    }

    pub fn load(dir: Option<PathBuf>) -> CustomBossEvents {
        let data = dir.as_deref().and_then(|dir| match read_dat(&CustomBossEvents::file(dir)) {
            Ok(tag) => tag.and_then(|t| t.get("data").cloned()),
            Err(_) => {
                error!("Error loading saved data: SavedDataType[minecraft:custom_boss_events]");
                None
            }
        });
        let mut events = JavaHashMap::new(identifier_hash);
        let dirty = data.is_none();
        if let Some(Nbt::Compound(entries)) = data {
            let mut random = LegacyRandomSource::create();
            for (id, packed) in entries {
                if let Some(event) = CustomBossEvent::load(create_insecure_uuid(&mut random), id.clone(), &packed) {
                    events.insert(id, event);
                }
            }
        }
        CustomBossEvents { dir, events, dirty }
    }

    pub fn get(&self, id: &str) -> Option<&CustomBossEvent> {
        self.events.get(id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut CustomBossEvent> {
        self.events.get_mut(id)
    }

    pub fn create(&mut self, level: &mut ServerLevel, id: &str, name: Component) -> &mut CustomBossEvent {
        let event = CustomBossEvent::new(create_insecure_uuid(&mut level.random), id.to_string(), name);
        self.events.insert(id.to_string(), event);
        self.dirty = true;
        self.events.get_mut(id).unwrap()
    }

    pub fn remove(&mut self, id: &str) {
        if self.events.remove(id).is_some() {
            self.dirty = true;
        }
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn events(&self) -> impl Iterator<Item = &CustomBossEvent> {
        self.events.iter().map(|(_, e)| e)
    }

    pub fn on_player_connect(&mut self, level: &ServerLevel, player: EntityId) {
        for event in self.events.values_mut() {
            event.on_player_connect(level, player);
        }
    }

    pub fn on_player_disconnect(&mut self, level: &ServerLevel, player: EntityId) {
        for event in self.events.values_mut() {
            event.on_player_disconnect(level, player);
        }
    }

    fn is_dirty(&mut self) -> bool {
        self.dirty || self.events.iter().any(|(_, e)| e.dirty)
    }

    pub fn save(&mut self) -> io::Result<()> {
        let Some(dir) = self.dir.clone() else { return Ok(()) };
        if !self.is_dirty() {
            return Ok(());
        }
        let events = self.events.iter().map(|(id, e)| (id.to_string(), e.pack())).collect();
        let file = Nbt::Compound(vec![("data".into(), Nbt::Compound(events)), ("DataVersion".into(), Nbt::Int(WORLD_VERSION))]);
        write_dat(&CustomBossEvents::file(&dir), &file)?;
        self.dirty = false;
        self.events.values_mut().for_each(|e| e.dirty = false);
        Ok(())
    }
}
