use crisp_core::BlockPos;
use crisp_core::fastutil::OpenHashMap;
use crisp_protocol::nbt::Nbt;

use super::poi_manager::Occupancy;
use super::poi_record::PoiRecord;
use super::poi_types::PoiType;
use crate::util::java_collections::JavaObjectHashSet;

pub struct PoiSection {
    records: OpenHashMap<i16, PoiRecord>,
    by_type: Vec<(PoiType, JavaObjectHashSet<BlockPos>)>,
    is_valid: bool,
}

pub fn section_relative_pos(pos: BlockPos) -> i16 {
    ((pos.x & 15) << 8 | (pos.z & 15) << 4 | (pos.y & 15)) as i16
}

impl Default for PoiSection {
    fn default() -> PoiSection {
        PoiSection::with_records(true, Vec::new())
    }
}

impl PoiSection {
    fn with_records(is_valid: bool, records: Vec<PoiRecord>) -> PoiSection {
        let mut section = PoiSection { records: OpenHashMap::new(), by_type: Vec::new(), is_valid };
        for record in records {
            section.add_record(record);
        }
        section
    }

    pub fn pack(&self) -> Nbt {
        let mut fields = Vec::new();
        if self.is_valid {
            fields.push(("Valid".to_string(), Nbt::Byte(1)));
        }
        fields.push(("Records".to_string(), Nbt::List(self.records.stream_values().map(PoiRecord::pack).collect())));
        Nbt::Compound(fields)
    }

    pub fn unpack(nbt: &Nbt) -> Option<PoiSection> {
        let is_valid = nbt.get("Valid").and_then(Nbt::as_i64).is_some_and(|v| v != 0);
        let records = nbt.get("Records")?.as_list().iter().map(PoiRecord::unpack).collect::<Option<Vec<_>>>()?;
        Some(PoiSection::with_records(is_valid, records))
    }

    pub fn get_records<'a>(&'a self, predicate: impl Fn(PoiType) -> bool + 'a, occupancy: Occupancy) -> impl Iterator<Item = &'a PoiRecord> + 'a {
        self.by_type
            .iter()
            .filter(move |(poi_type, _)| predicate(*poi_type))
            .flat_map(|(_, positions)| positions.iter())
            .filter_map(|pos| self.records.get(section_relative_pos(pos)))
            .filter(move |record| occupancy.test(record))
    }

    pub fn add(&mut self, pos: BlockPos, poi_type: PoiType) -> Option<PoiRecord> {
        let record = PoiRecord::new(pos, poi_type);
        self.add_record(record).then_some(record)
    }

    fn add_record(&mut self, record: PoiRecord) -> bool {
        let key = section_relative_pos(record.pos);
        if let Some(old) = self.records.get(key) {
            if old.poi_type == record.poi_type {
                return false;
            }
            error!("POI data mismatch: already registered at {:?}", record.pos);
        }
        self.records.insert(key, record);
        match self.by_type.iter_mut().find(|(poi_type, _)| *poi_type == record.poi_type) {
            Some((_, positions)) => {
                positions.insert(record.pos);
            }
            None => {
                let mut positions = JavaObjectHashSet::default();
                positions.insert(record.pos);
                self.by_type.push((record.poi_type, positions));
            }
        }
        true
    }

    pub fn remove(&mut self, pos: BlockPos) -> bool {
        let Some(record) = self.records.remove(section_relative_pos(pos)) else {
            error!("POI data mismatch: never registered at {pos:?}");
            return false;
        };
        if let Some((_, positions)) = self.by_type.iter_mut().find(|(poi_type, _)| *poi_type == record.poi_type) {
            positions.remove(&record.pos);
        }
        true
    }

    pub fn record_mut(&mut self, pos: BlockPos) -> Option<&mut PoiRecord> {
        self.records.get_mut(section_relative_pos(pos))
    }

    pub fn exists(&self, pos: BlockPos, predicate: impl Fn(PoiType) -> bool) -> bool {
        self.get_type(pos).is_some_and(predicate)
    }

    pub fn get_type(&self, pos: BlockPos) -> Option<PoiType> {
        self.records.get(section_relative_pos(pos)).map(|record| record.poi_type)
    }

    pub fn refresh(&mut self, updater: impl FnOnce(&mut dyn FnMut(BlockPos, PoiType))) -> bool {
        if self.is_valid {
            return false;
        }
        let old_records = std::mem::replace(&mut self.records, OpenHashMap::new());
        self.by_type.clear();
        updater(&mut |pos, poi_type| {
            let record = old_records.get(section_relative_pos(pos)).copied().unwrap_or_else(|| PoiRecord::new(pos, poi_type));
            self.add_record(record);
        });
        self.is_valid = true;
        true
    }

    pub fn is_valid(&self) -> bool {
        self.is_valid
    }
}
