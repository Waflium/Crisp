use crisp_protocol::Writer;
use crisp_protocol::nbt::Nbt;

use super::map_decoration_type::MapDecorationTypeHolder;

#[derive(Clone, PartialEq, Debug)]
pub struct MapDecoration {
    pub ty: MapDecorationTypeHolder,
    pub x: i8,
    pub y: i8,
    pub rot: i8,
    pub name: Option<Nbt>,
}

impl MapDecoration {
    pub fn new(ty: MapDecorationTypeHolder, x: i8, y: i8, rot: i8, name: Option<Nbt>) -> MapDecoration {
        MapDecoration { ty, x, y, rot: rot & 15, name }
    }

    pub fn write(&self, w: &mut Writer) {
        w.varint(self.ty.0 as i32);
        w.u8(self.x as u8);
        w.u8(self.y as u8);
        w.u8(self.rot as u8);
        w.bool(self.name.is_some());
        if let Some(name) = &self.name {
            name.write(w);
        }
    }
}
