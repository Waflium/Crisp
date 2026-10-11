use crisp_protocol::nbt::Nbt;

use crate::advancements::predicates::mob_effects_predicate::MobEffectsPredicate;
use crate::server::level::ServerLevel;
use crate::util::codec::DataResult;
use crate::world::entity::EntityId;

#[derive(Debug)]
pub struct EntityEffectsPredicate {
    effects: MobEffectsPredicate,
}

impl EntityEffectsPredicate {
    pub fn decode(tag: &Nbt) -> DataResult<EntityEffectsPredicate> {
        Ok(EntityEffectsPredicate { effects: MobEffectsPredicate::decode(tag)? })
    }

    pub fn matches(&self, level: &ServerLevel, entity: EntityId) -> bool {
        level.is_living(entity) && self.effects.matches(&level.living(entity).active_effects)
    }
}
