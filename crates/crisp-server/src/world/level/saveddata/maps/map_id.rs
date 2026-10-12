use crisp_protocol::nbt::Nbt;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct MapId(pub i32);

impl MapId {
    pub fn id(self) -> i32 {
        self.0
    }

    pub fn key(self) -> String {
        format!("maps/{}", self.0)
    }

    pub fn from_nbt(tag: &Nbt) -> Option<MapId> {
        tag.as_i64().map(|id| MapId(id as i32))
    }

    pub fn to_nbt(self) -> Nbt {
        Nbt::Int(self.0)
    }
}
