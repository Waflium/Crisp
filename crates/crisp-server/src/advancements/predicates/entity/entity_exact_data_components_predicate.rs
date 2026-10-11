use crisp_protocol::nbt::Nbt;

use crate::core::component::data_component_exact_predicate::DataComponentExactPredicate;
use crate::server::level::ServerLevel;
use crate::util::codec::DataResult;
use crate::world::entity::EntityId;
use crate::world::entity::entity_components::EntityComponents;

#[derive(Debug)]
pub struct EntityExactDataComponentsPredicate {
    predicate: DataComponentExactPredicate,
}

impl EntityExactDataComponentsPredicate {
    pub fn decode(tag: &Nbt) -> DataResult<EntityExactDataComponentsPredicate> {
        Ok(EntityExactDataComponentsPredicate { predicate: DataComponentExactPredicate::decode(tag)? })
    }

    pub fn matches(&self, level: &ServerLevel, entity: EntityId) -> bool {
        self.predicate.test(&EntityComponents { level, id: entity })
    }
}
