use crisp_core::BlockPos;
use crisp_protocol::nbt::Nbt;

#[derive(Clone, PartialEq, Debug)]
pub struct MapFrame {
    pub pos: BlockPos,
    pub rotation: i32,
    pub entity_id: i32,
}

impl MapFrame {
    pub fn decode(tag: &Nbt) -> Option<MapFrame> {
        let Some(Nbt::IntArray(pos)) = tag.get("pos") else { return None };
        let [x, y, z] = pos[..] else { return None };
        let rotation = tag.get("rotation")?.as_i64()? as i32;
        let entity_id = tag.get("entity_id")?.as_i64()? as i32;
        Some(MapFrame { pos: BlockPos::new(x, y, z), rotation, entity_id })
    }

    pub fn encode(&self) -> Nbt {
        Nbt::Compound(vec![
            ("pos".into(), Nbt::IntArray(vec![self.pos.x, self.pos.y, self.pos.z])),
            ("rotation".into(), Nbt::Int(self.rotation)),
            ("entity_id".into(), Nbt::Int(self.entity_id)),
        ])
    }

    pub fn get_id(&self) -> String {
        frame_id(self.pos)
    }
}

pub fn frame_id(pos: BlockPos) -> String {
    format!("frame-{},{},{}", pos.x, pos.y, pos.z)
}
