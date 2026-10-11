use crisp_protocol::nbt::Nbt;

use crate::advancements::predicates::entity::entity_predicate::EntityPredicate;
use crate::util::codec::{self, DataResult};
use crate::world::level::storage::loot::loot_context::{EntityTarget, LootContext};

pub struct LootItemEntityPropertyCondition {
    predicate: Option<EntityPredicate>,
    entity_target: EntityTarget,
}

impl LootItemEntityPropertyCondition {
    pub fn decode(tag: &Nbt) -> DataResult<LootItemEntityPropertyCondition> {
        Ok(LootItemEntityPropertyCondition {
            predicate: codec::decode_optional(tag, "predicate", EntityPredicate::decode)?,
            entity_target: codec::decode_field(tag, "entity", EntityTarget::decode)?,
        })
    }

    pub fn test(&self, context: &mut LootContext) -> bool {
        let Some(predicate) = &self.predicate else { return true };
        let entity = self.entity_target.get(&context.params);
        let pos = context.params.origin;
        predicate.matches(context.level(), pos, entity)
    }
}
