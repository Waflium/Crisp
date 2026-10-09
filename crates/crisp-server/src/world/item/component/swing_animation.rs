use crisp_protocol::Writer;
use crisp_protocol::nbt::Nbt;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SwingAnimationType {
    None,
    Whack,
    Stab,
}

impl SwingAnimationType {
    pub fn by_name(name: &str) -> Option<SwingAnimationType> {
        match name {
            "none" => Some(SwingAnimationType::None),
            "whack" => Some(SwingAnimationType::Whack),
            "stab" => Some(SwingAnimationType::Stab),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SwingAnimation {
    pub ty: SwingAnimationType,
    pub duration: i32,
}

impl SwingAnimation {
    pub const DEFAULT: SwingAnimation = SwingAnimation { ty: SwingAnimationType::Whack, duration: 6 };

    pub fn from_nbt(nbt: &Nbt) -> SwingAnimation {
        let ty = nbt.get("type").and_then(Nbt::as_str).and_then(SwingAnimationType::by_name).unwrap_or(SwingAnimation::DEFAULT.ty);
        let duration = nbt.get("duration").and_then(Nbt::as_i64).map_or(SwingAnimation::DEFAULT.duration, |d| d as i32);
        SwingAnimation { ty, duration }
    }

    pub fn write(&self, w: &mut Writer) {
        w.varint(self.ty as i32);
        w.varint(self.duration);
    }
}
