use crisp_core::random::RandomSource;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RandomSpreadType {
    Linear,
    Triangular,
}

impl RandomSpreadType {
    pub fn by_name(name: &str) -> Option<RandomSpreadType> {
        match name {
            "linear" => Some(RandomSpreadType::Linear),
            "triangular" => Some(RandomSpreadType::Triangular),
            _ => None,
        }
    }

    pub fn evaluate(self, random: &mut impl RandomSource, limit: i32) -> i32 {
        match self {
            RandomSpreadType::Linear => random.next_int_bounded(limit),
            RandomSpreadType::Triangular => (random.next_int_bounded(limit) + random.next_int_bounded(limit)) / 2,
        }
    }
}
