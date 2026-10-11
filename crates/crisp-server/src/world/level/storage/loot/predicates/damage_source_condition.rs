use crisp_protocol::nbt::Nbt;

use crate::advancements::predicates::damage_source_predicate::DamageSourcePredicate;
use crate::util::codec::{self, DataResult};
use crate::world::level::storage::loot::loot_context::LootContext;

pub struct DamageSourceCondition {
    predicate: Option<DamageSourcePredicate>,
}

impl DamageSourceCondition {
    pub fn decode(tag: &Nbt) -> DataResult<DamageSourceCondition> {
        Ok(DamageSourceCondition { predicate: codec::decode_optional(tag, "predicate", DamageSourcePredicate::decode)? })
    }

    pub fn test(&self, context: &mut LootContext) -> bool {
        let (Some(damage_source), Some(pos)) = (context.params.damage_source, context.params.origin) else { return false };
        self.predicate.as_ref().is_none_or(|predicate| predicate.matches(context.level(), Some(pos), &damage_source))
    }
}
