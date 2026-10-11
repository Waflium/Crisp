use crisp_protocol::nbt::Nbt;

use super::min_max_bounds::{Doubles, Ints};
use crate::util::codec::{self, DataResult};
use crate::world::food::FoodData;

#[derive(Debug)]
pub struct FoodPredicate {
    level: Ints,
    saturation: Doubles,
}

impl FoodPredicate {
    pub const ANY: FoodPredicate = FoodPredicate { level: Ints::ANY, saturation: Doubles::ANY };

    pub fn decode(tag: &Nbt) -> DataResult<FoodPredicate> {
        codec::map(tag)?;
        Ok(FoodPredicate { level: codec::decode_or(tag, "level", Ints::ANY, Ints::decode)?, saturation: codec::decode_or(tag, "saturation", Doubles::ANY, Doubles::decode)? })
    }

    pub fn matches(&self, food: &FoodData) -> bool {
        self.level.matches(food.food_level) && self.saturation.matches(food.saturation_level as f64)
    }
}
