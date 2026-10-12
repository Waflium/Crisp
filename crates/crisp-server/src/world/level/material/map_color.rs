use crisp_data::BlockState;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct MapColor {
    pub id: u8,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Brightness {
    Low = 0,
    Normal = 1,
    High = 2,
    Lowest = 3,
}

impl MapColor {
    pub const NONE: MapColor = MapColor { id: 0 };
    pub const WATER: MapColor = MapColor { id: 12 };
    pub const COLOR_ORANGE: MapColor = MapColor { id: 15 };
    pub const COLOR_BROWN: MapColor = MapColor { id: 26 };

    pub fn get_packed_id(self, brightness: Brightness) -> u8 {
        self.id << 2 | brightness as u8 & 3
    }
}

pub fn get_map_color(state: BlockState) -> MapColor {
    MapColor { id: state.map_color() }
}
