use crisp_protocol::nbt::Nbt;

use crate::advancements::predicates::min_max_bounds::Ints;
use crate::core::component::data_component_getter::DataComponentGetter;
use crate::util::codec::{self, DataResult};
use crate::world::item::component::{self as dc, codec as component_codec};

#[derive(Debug)]
pub struct DamagePredicate {
    durability: Ints,
    damage: Ints,
}

impl DamagePredicate {
    pub fn decode(tag: &Nbt) -> DataResult<DamagePredicate> {
        codec::map(tag)?;
        Ok(DamagePredicate {
            durability: codec::decode_or(tag, "durability", Ints::ANY, Ints::decode)?,
            damage: codec::decode_or(tag, "damage", Ints::ANY, Ints::decode)?,
        })
    }

    pub fn matches(&self, components: &(impl DataComponentGetter + ?Sized)) -> bool {
        let Some(damage) = components.get(dc::DAMAGE) else { return false };
        let damage = component_codec::as_i32(&damage);
        let max_damage = components.get(dc::MAX_DAMAGE).map_or(0, |v| component_codec::as_i32(&v));
        self.durability.matches(max_damage - damage) && self.damage.matches(damage)
    }
}
