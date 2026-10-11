use crate::Axis;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SymmetricGroup3 {
    P123,
    P213,
    P132,
    P312,
    P231,
    P321,
}

impl SymmetricGroup3 {
    const fn p(self) -> [usize; 3] {
        match self {
            SymmetricGroup3::P123 => [0, 1, 2],
            SymmetricGroup3::P213 => [1, 0, 2],
            SymmetricGroup3::P132 => [0, 2, 1],
            SymmetricGroup3::P312 => [2, 0, 1],
            SymmetricGroup3::P231 => [1, 2, 0],
            SymmetricGroup3::P321 => [2, 1, 0],
        }
    }

    pub const fn inverse(self) -> SymmetricGroup3 {
        match self {
            SymmetricGroup3::P312 => SymmetricGroup3::P231,
            SymmetricGroup3::P231 => SymmetricGroup3::P312,
            other => other,
        }
    }

    pub const fn permute(self, i: usize) -> usize {
        self.p()[i]
    }

    pub const fn permute_axis(self, axis: Axis) -> Axis {
        Axis::VALUES[self.permute(axis as usize)]
    }
}
