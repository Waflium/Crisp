use crisp_protocol::nbt::Nbt;

use crate::advancements::predicates::item_predicate::ItemPredicate;
use crate::util::codec::{self, DataResult};
use crate::world::level::storage::loot::loot_context::LootContext;

pub struct MatchTool {
    predicate: Option<ItemPredicate>,
}

impl MatchTool {
    pub fn decode(tag: &Nbt) -> DataResult<MatchTool> {
        Ok(MatchTool { predicate: codec::decode_optional(tag, "predicate", ItemPredicate::decode)? })
    }

    pub fn test(&self, context: &mut LootContext) -> bool {
        let Some(tool) = context.params.tool else { return false };
        self.predicate.as_ref().is_none_or(|predicate| predicate.test(tool))
    }
}
