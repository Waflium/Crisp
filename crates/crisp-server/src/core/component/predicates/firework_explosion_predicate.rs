use crisp_protocol::nbt::Nbt;

use crate::util::codec::{self, DataResult};
use crate::world::item::component::codec as component_codec;

const SHAPES: &[(&str, &str)] = &[("small_ball", "small_ball"), ("large_ball", "large_ball"), ("star", "star"), ("creeper", "creeper"), ("burst", "burst")];

#[derive(Debug)]
pub struct FireworkPredicate {
    shape: Option<&'static str>,
    twinkle: Option<bool>,
    trail: Option<bool>,
}

impl FireworkPredicate {
    pub fn decode(tag: &Nbt) -> DataResult<FireworkPredicate> {
        codec::map(tag)?;
        Ok(FireworkPredicate {
            shape: codec::decode_optional(tag, "shape", |t| codec::string_enum(t, SHAPES))?,
            twinkle: codec::decode_optional(tag, "has_twinkle", codec::bool_value)?,
            trail: codec::decode_optional(tag, "has_trail", codec::bool_value)?,
        })
    }

    pub fn test(&self, firework_explosion: &Nbt) -> bool {
        let flag = |key| firework_explosion.get(key).is_some_and(|v| component_codec::as_i64(v) != 0);
        if self.shape.is_some_and(|shape| firework_explosion.get("shape").and_then(Nbt::as_str) != Some(shape)) {
            return false;
        }
        if self.twinkle.is_some_and(|twinkle| twinkle != flag("has_twinkle")) {
            return false;
        }
        self.trail.is_none_or(|trail| trail == flag("has_trail"))
    }
}

#[derive(Debug)]
pub struct FireworkExplosionPredicate {
    predicate: FireworkPredicate,
}

impl FireworkExplosionPredicate {
    pub fn decode(tag: &Nbt) -> DataResult<FireworkExplosionPredicate> {
        Ok(FireworkExplosionPredicate { predicate: FireworkPredicate::decode(tag)? })
    }

    pub fn matches(&self, value: &Nbt) -> bool {
        self.predicate.test(value)
    }
}
