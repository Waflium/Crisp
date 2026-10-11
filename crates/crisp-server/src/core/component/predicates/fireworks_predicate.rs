use crisp_protocol::nbt::Nbt;

use super::firework_explosion_predicate::FireworkPredicate;
use crate::advancements::predicates::collection_predicate::CollectionPredicate;
use crate::advancements::predicates::min_max_bounds::Ints;
use crate::util::codec::{self, DataResult};
use crate::world::item::component::codec as component_codec;

#[derive(Debug)]
pub struct FireworksPredicate {
    explosions: Option<CollectionPredicate<FireworkPredicate>>,
    flight_duration: Ints,
}

impl FireworksPredicate {
    pub fn decode(tag: &Nbt) -> DataResult<FireworksPredicate> {
        codec::map(tag)?;
        Ok(FireworksPredicate {
            explosions: codec::decode_optional(tag, "explosions", |t| CollectionPredicate::decode(t, FireworkPredicate::decode))?,
            flight_duration: codec::decode_or(tag, "flight_duration", Ints::ANY, Ints::decode)?,
        })
    }

    pub fn matches(&self, value: &Nbt) -> bool {
        if let Some(explosions) = &self.explosions {
            let values = value.get("explosions").map_or(&[][..], Nbt::as_list);
            if !explosions.test(values, |p, explosion| p.test(explosion)) {
                return false;
            }
        }
        self.flight_duration.matches(value.get("flight_duration").map_or(0, component_codec::as_i32))
    }
}
