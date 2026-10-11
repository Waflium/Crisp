use crisp_protocol::nbt::Nbt;

use crate::advancements::predicates::min_max_bounds::Doubles;
use crate::server::level::ServerLevel;
use crate::util::codec::{self, DataResult};
use crate::world::entity::EntityId;

#[derive(Debug)]
pub struct MovementPredicate {
    x: Doubles,
    y: Doubles,
    z: Doubles,
    speed: Doubles,
    horizontal_speed: Doubles,
    vertical_speed: Doubles,
    fall_distance: Doubles,
}

impl MovementPredicate {
    pub fn decode(tag: &Nbt) -> DataResult<MovementPredicate> {
        codec::map(tag)?;
        let bound = |key| codec::decode_or(tag, key, Doubles::ANY, Doubles::decode);
        Ok(MovementPredicate {
            x: bound("x")?,
            y: bound("y")?,
            z: bound("z")?,
            speed: bound("speed")?,
            horizontal_speed: bound("horizontal_speed")?,
            vertical_speed: bound("vertical_speed")?,
            fall_distance: bound("fall_distance")?,
        })
    }

    pub fn matches(&self, x: f64, y: f64, z: f64, fall_distance: f64) -> bool {
        if !(self.x.matches(x) && self.y.matches(y) && self.z.matches(z)) {
            return false;
        }
        if !self.speed.matches_sqr(x * x + y * y + z * z) {
            return false;
        }
        if !self.horizontal_speed.matches_sqr(x * x + z * z) {
            return false;
        }
        self.vertical_speed.matches(y.abs()) && self.fall_distance.matches(fall_distance)
    }

    pub fn matches_entity(&self, level: &ServerLevel, entity: EntityId) -> bool {
        let velocity = level.known_movement(entity).scale(20.0);
        self.matches(velocity.x, velocity.y, velocity.z, level.entity(entity).fall_distance)
    }
}
