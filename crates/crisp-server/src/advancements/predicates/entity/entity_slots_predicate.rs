use crisp_protocol::nbt::Nbt;

use crate::advancements::predicates::slots_predicate::SlotsPredicate;
use crate::server::level::ServerLevel;
use crate::util::codec::DataResult;
use crate::world::entity::EntityId;

#[derive(Debug)]
pub struct EntitySlotsPredicate {
    slots: SlotsPredicate,
}

impl EntitySlotsPredicate {
    pub fn decode(tag: &Nbt) -> DataResult<EntitySlotsPredicate> {
        Ok(EntitySlotsPredicate { slots: SlotsPredicate::decode(tag)? })
    }

    pub fn matches(&self, level: &ServerLevel, entity: EntityId) -> bool {
        self.slots.matches(|slot| level.get_slot_item(entity, slot))
    }
}
