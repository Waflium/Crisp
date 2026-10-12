use crisp_core::{BlockPos, Vec3};
use crisp_protocol::nbt::Nbt;

use super::criterion_trigger::CriterionTrigger;
use super::simple_criterion_trigger;
use crate::advancements::criterion_trigger_instance::CriterionTriggerInstance;
use crate::core::holder::Holder;
use crate::server::level::ServerLevel;
use crate::util::codec::DataResult;
use crate::world::entity::EntityId;
use crate::world::level::BlockGetter;
use crate::world::level::storage::loot::loot_context::{LootContext, LootContextBuilder};
use crate::world::level::storage::loot::loot_params::LootParams;
use crate::world::level::storage::loot::predicates::loot_item_condition::LootItemCondition;

#[derive(Clone, Copy)]
pub struct DefaultBlockInteractionTrigger(pub CriterionTrigger);

impl DefaultBlockInteractionTrigger {
    pub fn trigger(self, level: &mut ServerLevel, player: EntityId, pos: BlockPos) {
        let state = level.block_state(pos);
        simple_criterion_trigger::trigger(level, player, self.0, |level, instance| {
            let CriterionTriggerInstance::DefaultBlockInteraction(t) = instance else { return false };
            let mut params = LootParams::new(level);
            params.origin = Some(Vec3::at_center_of(pos));
            params.this_entity = Some(player);
            params.block_state = Some(state);
            t.matches(&mut LootContextBuilder::new(params).create(None))
        });
    }
}

pub struct TriggerInstance {
    pub player: Option<Holder<LootItemCondition>>,
    location: Option<Holder<LootItemCondition>>,
}

impl TriggerInstance {
    pub fn decode(tag: &Nbt) -> DataResult<TriggerInstance> {
        Ok(TriggerInstance { player: simple_criterion_trigger::decode_player(tag)?, location: simple_criterion_trigger::decode_condition(tag, "location")? })
    }

    pub fn matches(&self, location_context: &mut LootContext) -> bool {
        simple_criterion_trigger::matches(self.location.as_ref(), location_context)
    }
}
