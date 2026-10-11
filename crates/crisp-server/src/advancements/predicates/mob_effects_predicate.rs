use crisp_data::MobEffect;
use crisp_protocol::nbt::Nbt;

use super::min_max_bounds::Ints;
use crate::util::codec::{self, DataResult};
use crate::world::effect::MobEffectInstance;

#[derive(Debug)]
pub struct MobEffectInstancePredicate {
    amplifier: Ints,
    duration: Ints,
    ambient: Option<bool>,
    visible: Option<bool>,
}

impl MobEffectInstancePredicate {
    fn decode(tag: &Nbt) -> DataResult<MobEffectInstancePredicate> {
        codec::map(tag)?;
        Ok(MobEffectInstancePredicate {
            amplifier: codec::decode_or(tag, "amplifier", Ints::ANY, Ints::decode)?,
            duration: codec::decode_or(tag, "duration", Ints::ANY, Ints::decode)?,
            ambient: codec::decode_optional(tag, "ambient", codec::bool_value)?,
            visible: codec::decode_optional(tag, "visible", codec::bool_value)?,
        })
    }

    pub fn matches(&self, instance: Option<&MobEffectInstance>) -> bool {
        let Some(instance) = instance else { return false };
        if !self.amplifier.matches(instance.amplifier) || !self.duration.matches(instance.duration) {
            return false;
        }
        if self.ambient.is_some_and(|ambient| ambient != instance.ambient) {
            return false;
        }
        self.visible.is_none_or(|visible| visible == instance.visible)
    }
}

#[derive(Debug)]
pub struct MobEffectsPredicate {
    effect_map: Vec<(MobEffect, MobEffectInstancePredicate)>,
}

impl MobEffectsPredicate {
    pub fn decode(tag: &Nbt) -> DataResult<MobEffectsPredicate> {
        let effect_map = codec::unbounded_map(
            tag,
            |key| {
                let id = crate::resources::identifier::read(key)?;
                MobEffect::by_name(&id).ok_or_else(|| format!("Unknown registry key in ResourceKey[minecraft:root / minecraft:mob_effect]: {id}"))
            },
            MobEffectInstancePredicate::decode,
        )?;
        Ok(MobEffectsPredicate { effect_map })
    }

    pub fn matches(&self, effects: &[MobEffectInstance]) -> bool {
        self.effect_map.iter().all(|(effect, predicate)| predicate.matches(effects.iter().find(|e| e.effect == *effect)))
    }

    pub fn test(&self, mob_effect: &MobEffectInstance) -> bool {
        self.effect_map.iter().find(|(effect, _)| *effect == mob_effect.effect).is_some_and(|(_, predicate)| predicate.matches(Some(mob_effect)))
    }
}
