use crisp_protocol::nbt::Nbt;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MapPostProcessing {
    Lock,
    Scale,
}

impl MapPostProcessing {
    pub fn from_nbt(tag: &Nbt) -> Option<MapPostProcessing> {
        match tag.as_str()? {
            "lock" => Some(MapPostProcessing::Lock),
            "scale" => Some(MapPostProcessing::Scale),
            _ => None,
        }
    }

    pub fn to_nbt(self) -> Nbt {
        Nbt::String(match self {
            MapPostProcessing::Lock => "lock",
            MapPostProcessing::Scale => "scale",
        }.into())
    }
}
