use crisp_protocol::nbt::Nbt;

use super::map_id::MapId;

const NO_MAP_ID: i32 = -1;
pub const TYPE: &str = "maps/last_id";

pub struct MapIndex {
    last_map_id: i32,
    pub dirty: bool,
}

impl Default for MapIndex {
    fn default() -> MapIndex {
        MapIndex::new(NO_MAP_ID)
    }
}

impl MapIndex {
    pub fn new(last_map_id: i32) -> MapIndex {
        MapIndex { last_map_id, dirty: false }
    }

    pub fn get_next_map_id(&mut self) -> MapId {
        self.last_map_id += 1;
        let id = MapId(self.last_map_id);
        self.dirty = true;
        id
    }

    pub fn decode(tag: &Nbt) -> Option<MapIndex> {
        let last_map_id = match tag.get("map") {
            Some(map) => map.as_i64()? as i32,
            None => NO_MAP_ID,
        };
        Some(MapIndex::new(last_map_id))
    }

    pub fn encode(&self) -> Nbt {
        let mut fields = Vec::new();
        if self.last_map_id != NO_MAP_ID {
            fields.push(("map".into(), Nbt::Int(self.last_map_id)));
        }
        Nbt::Compound(fields)
    }
}
