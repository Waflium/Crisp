use crisp_core::BlockPos;
use crisp_protocol::nbt::Nbt;

#[derive(Clone)]
pub struct Occupant {
    pub entity_data: Nbt,
    pub ticks_in_hive: i32,
    pub min_ticks_in_hive: i32,
}

impl Occupant {
    pub fn create(ticks_in_hive: i32) -> Occupant {
        Occupant { entity_data: Nbt::Compound(vec![("id".into(), Nbt::String("minecraft:bee".into()))]), ticks_in_hive, min_ticks_in_hive: 600 }
    }

    fn save(&self) -> Nbt {
        Nbt::Compound(vec![
            ("entity_data".into(), self.entity_data.clone()),
            ("ticks_in_hive".into(), Nbt::Int(self.ticks_in_hive)),
            ("min_ticks_in_hive".into(), Nbt::Int(self.min_ticks_in_hive)),
        ])
    }

    fn load(tag: &Nbt) -> Option<Occupant> {
        let entity_data = tag.get("entity_data").filter(|data| data.get("id").and_then(Nbt::as_str).is_some())?.clone();
        let int = |key: &str| match tag.get(key)? {
            Nbt::Byte(v) => Some(*v as i32),
            Nbt::Short(v) => Some(*v as i32),
            Nbt::Int(v) => Some(*v),
            Nbt::Long(v) => Some(*v as i32),
            _ => None,
        };
        Some(Occupant { entity_data, ticks_in_hive: int("ticks_in_hive")?, min_ticks_in_hive: int("min_ticks_in_hive")? })
    }
}

#[derive(Clone)]
struct BeeData {
    occupant: Occupant,
    ticks_in_hive: i32,
}

impl BeeData {
    fn new(occupant: Occupant) -> BeeData {
        let ticks_in_hive = occupant.ticks_in_hive;
        BeeData { occupant, ticks_in_hive }
    }

    fn to_occupant(&self) -> Occupant {
        Occupant { entity_data: self.occupant.entity_data.clone(), ticks_in_hive: self.ticks_in_hive, min_ticks_in_hive: self.occupant.min_ticks_in_hive }
    }
}

#[derive(Clone, Default)]
pub struct BeehiveBlockEntity {
    stored: Vec<BeeData>,
    saved_flower_pos: Option<BlockPos>,
}

impl BeehiveBlockEntity {
    pub fn store_bee(&mut self, occupant: Occupant) {
        self.stored.push(BeeData::new(occupant));
    }

    pub fn load_additional(&mut self, input: &Nbt) {
        self.stored.clear();
        if let Some(Nbt::List(bees)) = input.get("bees") {
            let occupants: Option<Vec<Occupant>> = bees.iter().map(Occupant::load).collect();
            occupants.unwrap_or_default().into_iter().for_each(|occupant| self.store_bee(occupant));
        }
        self.saved_flower_pos = match input.get("flower_pos") {
            Some(Nbt::IntArray(v)) if v.len() == 3 => Some(BlockPos::new(v[0], v[1], v[2])),
            _ => None,
        };
    }

    pub fn save_additional(&self, output: &mut Vec<(String, Nbt)>) {
        output.push(("bees".into(), Nbt::List(self.stored.iter().map(|bee| bee.to_occupant().save()).collect())));
        if let Some(pos) = self.saved_flower_pos {
            output.push(("flower_pos".into(), Nbt::IntArray(vec![pos.x, pos.y, pos.z])));
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BeeReleaseStatus {
    BeeReleased,
    Emergency,
}
