use crate::Tag;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[repr(u8)]
pub enum Fluid {
    Empty,
    FlowingWater,
    Water,
    FlowingLava,
    Lava,
}

impl Fluid {
    pub(crate) fn from_id(id: u8) -> Fluid {
        [Fluid::Empty, Fluid::FlowingWater, Fluid::Water, Fluid::FlowingLava, Fluid::Lava][id as usize]
    }

    pub fn is(self, tag: &Tag) -> bool {
        tag.contains(self as usize)
    }

    pub fn is_empty(self) -> bool {
        self == Fluid::Empty
    }

    pub fn source(self) -> Fluid {
        match self {
            Fluid::FlowingWater => Fluid::Water,
            Fluid::FlowingLava => Fluid::Lava,
            other => other,
        }
    }

    pub fn flowing(self) -> Fluid {
        match self {
            Fluid::Water => Fluid::FlowingWater,
            Fluid::Lava => Fluid::FlowingLava,
            other => other,
        }
    }

    pub fn is_randomly_ticking(self) -> bool {
        matches!(self, Fluid::FlowingLava | Fluid::Lava)
    }

    pub fn is_same(self, other: Fluid) -> bool {
        self.source() == other.source()
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FluidState {
    pub fluid: Fluid,
    pub amount: u8,
    pub falling: bool,
}

impl FluidState {
    pub const EMPTY: FluidState = FluidState { fluid: Fluid::Empty, amount: 0, falling: false };

    pub fn is_empty(self) -> bool {
        self.fluid.is_empty()
    }

    pub fn is_source(self) -> bool {
        matches!(self.fluid, Fluid::Water | Fluid::Lava)
    }

    pub fn is(self, tag: &Tag) -> bool {
        self.fluid.is(tag)
    }

    pub fn is_randomly_ticking(self) -> bool {
        self.fluid.is_randomly_ticking()
    }

    pub fn is_source_of_type(self, fluid: Fluid) -> bool {
        self.fluid == fluid && self.is_source()
    }

    pub fn own_height(self) -> f32 {
        self.amount as f32 / 9.0
    }
}
