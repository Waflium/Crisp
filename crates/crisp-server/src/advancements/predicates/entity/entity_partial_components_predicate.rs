use crisp_protocol::nbt::Nbt;

use crate::core::component::predicates::data_component_predicate::DataComponentPredicate;
use crate::server::level::ServerLevel;
use crate::util::codec::DataResult;
use crate::world::entity::EntityId;
use crate::world::entity::entity_components::EntityComponents;

#[derive(Debug)]
pub struct EntityPartialComponentsPredicate {
    predicates: Vec<DataComponentPredicate>,
}

impl EntityPartialComponentsPredicate {
    pub fn decode(tag: &Nbt) -> DataResult<EntityPartialComponentsPredicate> {
        Ok(EntityPartialComponentsPredicate { predicates: DataComponentPredicate::decode_map(tag)? })
    }

    pub fn matches(&self, level: &ServerLevel, entity: EntityId) -> bool {
        let components = EntityComponents { level, id: entity };
        self.predicates.iter().all(|predicate| predicate.matches(&components))
    }
}
