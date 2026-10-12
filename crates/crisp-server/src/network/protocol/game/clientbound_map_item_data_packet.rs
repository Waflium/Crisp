use crisp_protocol::Writer;

use crate::world::level::saveddata::maps::map_decoration::MapDecoration;
use crate::world::level::saveddata::maps::map_id::MapId;
use crate::world::level::saveddata::maps::map_item_saved_data::MapPatch;

#[derive(Clone, PartialEq, Debug)]
pub struct ClientboundMapItemDataPacket {
    pub map_id: MapId,
    pub scale: i8,
    pub locked: bool,
    pub decorations: Option<Vec<MapDecoration>>,
    pub color_patch: Option<MapPatch>,
}

impl ClientboundMapItemDataPacket {
    pub fn write(&self, w: &mut Writer) {
        w.varint(self.map_id.id());
        w.u8(self.scale as u8);
        w.bool(self.locked);
        w.bool(self.decorations.is_some());
        if let Some(decorations) = &self.decorations {
            w.varint(decorations.len() as i32);
            decorations.iter().for_each(|decoration| decoration.write(w));
        }
        MapPatch::write(self.color_patch.as_ref(), w);
    }
}
