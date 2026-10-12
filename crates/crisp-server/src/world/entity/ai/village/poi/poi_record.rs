use crisp_core::BlockPos;
use crisp_protocol::nbt::Nbt;

use super::poi_types::PoiType;

#[derive(Clone, Copy, Debug)]
pub struct PoiRecord {
    pub pos: BlockPos,
    pub poi_type: PoiType,
    pub free_tickets: i32,
}

impl PoiRecord {
    pub fn new(pos: BlockPos, poi_type: PoiType) -> PoiRecord {
        PoiRecord { pos, poi_type, free_tickets: poi_type.max_tickets() }
    }

    pub fn pack(&self) -> Nbt {
        let mut fields = vec![
            ("pos".to_string(), Nbt::IntArray(vec![self.pos.x, self.pos.y, self.pos.z])),
            ("type".to_string(), Nbt::String(self.poi_type.name().into())),
        ];
        if self.free_tickets != 0 {
            fields.push(("free_tickets".to_string(), Nbt::Int(self.free_tickets)));
        }
        Nbt::Compound(fields)
    }

    pub fn unpack(nbt: &Nbt) -> Option<PoiRecord> {
        let pos = match nbt.get("pos")? {
            Nbt::IntArray(p) if p.len() == 3 => BlockPos::new(p[0], p[1], p[2]),
            _ => return None,
        };
        let poi_type = PoiType::by_name(nbt.get("type")?.as_str()?)?;
        let free_tickets = nbt.get("free_tickets").and_then(Nbt::as_i64).map_or(0, |v| v as i32);
        Some(PoiRecord { pos, poi_type, free_tickets })
    }

    pub fn acquire_ticket(&mut self) -> bool {
        if self.free_tickets <= 0 {
            return false;
        }
        self.free_tickets -= 1;
        true
    }

    pub fn release_ticket(&mut self) -> bool {
        if self.free_tickets >= self.poi_type.max_tickets() {
            return false;
        }
        self.free_tickets += 1;
        true
    }

    pub fn has_space(&self) -> bool {
        self.free_tickets > 0
    }

    pub fn is_occupied(&self) -> bool {
        self.free_tickets != self.poi_type.max_tickets()
    }
}
