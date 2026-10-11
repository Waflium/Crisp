use crisp_protocol::nbt::Nbt;

use crate::advancements::predicates::collection_predicate::CollectionPredicate;
use crate::advancements::predicates::mob_effects_predicate::MobEffectsPredicate;
use crate::core::holder_set::HolderSet;
use crate::util::codec::{self, DataResult};
use crate::world::item::alchemy::potion_contents::PotionContents;
use crate::world::item::component::codec::registry_id;

#[derive(Debug)]
pub struct PotionsPredicate {
    potions: Option<HolderSet>,
    effects: Option<CollectionPredicate<MobEffectsPredicate>>,
}

impl PotionsPredicate {
    pub fn decode(tag: &Nbt) -> DataResult<PotionsPredicate> {
        codec::map(tag)?;
        Ok(PotionsPredicate {
            potions: codec::decode_optional(tag, "potions", |t| HolderSet::decode("minecraft:potion", t))?,
            effects: codec::decode_optional(tag, "effects", |t| CollectionPredicate::decode(t, MobEffectsPredicate::decode))?,
        })
    }

    pub fn matches(&self, value: &Nbt) -> bool {
        let potion_contents = PotionContents::new(value);
        if let Some(potions) = &self.potions {
            let potion = potion_contents.potion_id().and_then(|id| registry_id("minecraft:potion", id).ok());
            if !potion.is_some_and(|id| potions.contains(id)) {
                return false;
            }
        }
        self.effects.as_ref().is_none_or(|effects| effects.test(&potion_contents.get_all_effects(), |p, effect| p.test(effect)))
    }
}
