use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

use crisp_data::WORLD_VERSION;
use crisp_protocol::nbt::Nbt;

use super::{read_dat, write_dat};
use crate::world::entity::EntityId;
use crate::world::level::saveddata::maps::map_id::MapId;
use crate::world::level::saveddata::maps::map_index::{self, MapIndex};
use crate::world::level::saveddata::maps::map_item_saved_data::MapItemSavedData;

pub struct SavedDataStorage {
    data_folder: Option<PathBuf>,
    maps: HashMap<MapId, Option<Rc<RefCell<MapItemSavedData>>>>,
    map_index: Option<Option<MapIndex>>,
}

impl SavedDataStorage {
    pub fn new(data_folder: Option<PathBuf>) -> SavedDataStorage {
        SavedDataStorage { data_folder, maps: HashMap::new(), map_index: None }
    }

    fn get_data_file(&self, id: &str) -> Option<PathBuf> {
        Some(self.data_folder.as_ref()?.join("minecraft").join(format!("{id}.dat")))
    }

    fn read_saved_data<T>(&self, id: &str, decode: impl FnOnce(&Nbt) -> Result<T, String>) -> Option<T> {
        let file = self.get_data_file(id)?;
        match read_dat(&file) {
            Ok(Some(tag)) => match tag.get("data").ok_or_else(|| "No key data in MapLike".to_string()).and_then(decode) {
                Ok(data) => Some(data),
                Err(error) => {
                    error!("Failed to parse saved data for 'SavedDataType[minecraft:{id}]': {error}");
                    None
                }
            },
            Ok(None) => None,
            Err(_) => {
                error!("Error loading saved data: SavedDataType[minecraft:{id}]");
                None
            }
        }
    }

    pub fn get_map_data(&mut self, id: MapId) -> Option<Rc<RefCell<MapItemSavedData>>> {
        if !self.maps.contains_key(&id) {
            let data = self.read_saved_data(&id.key(), MapItemSavedData::decode).map(|data| Rc::new(RefCell::new(data)));
            self.maps.insert(id, data);
        }
        self.maps[&id].clone()
    }

    pub fn set_map_data(&mut self, id: MapId, mut data: MapItemSavedData) {
        data.set_dirty();
        self.maps.insert(id, Some(Rc::new(RefCell::new(data))));
    }

    pub fn map_index(&mut self) -> &mut MapIndex {
        if self.map_index.is_none() {
            self.map_index = Some(self.read_saved_data(map_index::TYPE, |tag| MapIndex::decode(tag).ok_or_else(|| "Not a number".to_string())));
        }
        let index = self.map_index.as_mut().unwrap();
        if index.is_none() {
            let mut new_data = MapIndex::default();
            new_data.dirty = true;
            *index = Some(new_data);
        }
        index.as_mut().unwrap()
    }

    pub fn player_removed(&self, player: EntityId) {
        for data in self.maps.values().flatten() {
            data.borrow_mut().player_removed(player);
        }
    }

    pub fn save(&mut self) {
        let mut to_save = Vec::new();
        for (id, data) in &self.maps {
            if let Some(data) = data {
                let mut data = data.borrow_mut();
                if data.dirty {
                    to_save.push((id.key(), data.encode()));
                    data.dirty = false;
                }
            }
        }
        if let Some(Some(index)) = &mut self.map_index
            && index.dirty
        {
            to_save.push((map_index::TYPE.to_string(), index.encode()));
            index.dirty = false;
        }
        for (id, data) in to_save {
            let Some(file) = self.get_data_file(&id) else { continue };
            let tag = Nbt::Compound(vec![("data".into(), data), ("DataVersion".into(), Nbt::Int(WORLD_VERSION))]);
            if write_dat(&file, &tag).is_err() {
                error!("Could not save data to {}", file.file_name().map_or_else(String::new, |name| name.to_string_lossy().into_owned()));
            }
        }
    }
}
