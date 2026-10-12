use crisp_data::registry;

use super::map_decoration_types::TYPES;

pub struct MapDecorationType {
    pub name: &'static str,
    pub asset_id: &'static str,
    pub show_on_item_frame: bool,
    pub track_count: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct MapDecorationTypeHolder(pub u16);

impl MapDecorationTypeHolder {
    pub fn value(self) -> &'static MapDecorationType {
        &TYPES[self.0 as usize]
    }

    pub fn key(self) -> &'static str {
        registry::MAP_DECORATION_TYPE[self.0 as usize]
    }

    pub fn by_name(name: &str) -> Option<MapDecorationTypeHolder> {
        let name = if name.contains(':') { name.to_string() } else { format!("minecraft:{name}") };
        registry::MAP_DECORATION_TYPE.iter().position(|&key| key == name).map(|id| MapDecorationTypeHolder(id as u16))
    }

    pub fn is(self, other: MapDecorationTypeHolder) -> bool {
        self == other
    }
}
