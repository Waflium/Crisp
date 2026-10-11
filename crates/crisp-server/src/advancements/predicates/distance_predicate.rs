use crisp_protocol::nbt::Nbt;

use super::min_max_bounds::Doubles;
use crate::util::codec::{self, DataResult};

#[derive(Debug)]
pub struct DistancePredicate {
    x: Doubles,
    y: Doubles,
    z: Doubles,
    horizontal: Doubles,
    absolute: Doubles,
}

impl DistancePredicate {
    pub fn decode(tag: &Nbt) -> DataResult<DistancePredicate> {
        codec::map(tag)?;
        let bound = |key| codec::decode_or(tag, key, Doubles::ANY, Doubles::decode);
        Ok(DistancePredicate { x: bound("x")?, y: bound("y")?, z: bound("z")?, horizontal: bound("horizontal")?, absolute: bound("absolute")? })
    }

    pub fn matches(&self, x0: f64, y0: f64, z0: f64, x1: f64, y1: f64, z1: f64) -> bool {
        let xd = (x0 - x1) as f32;
        let yd = (y0 - y1) as f32;
        let zd = (z0 - z1) as f32;
        if !self.x.matches(xd.abs() as f64) || !self.y.matches(yd.abs() as f64) || !self.z.matches(zd.abs() as f64) {
            return false;
        }
        self.horizontal.matches_sqr((xd * xd + zd * zd) as f64) && self.absolute.matches_sqr((xd * xd + yd * yd + zd * zd) as f64)
    }
}
