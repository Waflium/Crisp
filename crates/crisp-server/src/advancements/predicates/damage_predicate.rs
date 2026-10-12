use crisp_protocol::nbt::Nbt;

use super::damage_source_predicate::DamageSourcePredicate;
use super::entity::entity_predicate::EntityPredicate;
use super::min_max_bounds::Doubles;
use crate::server::level::ServerLevel;
use crate::util::codec::{self, DataResult};
use crate::world::damagesource::DamageSource;
use crate::world::entity::EntityId;

#[derive(Debug)]
pub struct DamagePredicate {
    dealt_damage: Doubles,
    taken_damage: Doubles,
    source_entity: Option<EntityPredicate>,
    blocked: Option<bool>,
    ty: Option<DamageSourcePredicate>,
}

impl DamagePredicate {
    pub fn decode(tag: &Nbt) -> DataResult<DamagePredicate> {
        codec::map(tag)?;
        Ok(DamagePredicate {
            dealt_damage: codec::decode_or(tag, "dealt", Doubles::ANY, Doubles::decode)?,
            taken_damage: codec::decode_or(tag, "taken", Doubles::ANY, Doubles::decode)?,
            source_entity: codec::decode_optional(tag, "source_entity", EntityPredicate::decode)?,
            blocked: codec::decode_optional(tag, "blocked", codec::bool_value)?,
            ty: codec::decode_optional(tag, "type", DamageSourcePredicate::decode)?,
        })
    }

    pub fn matches(&self, level: &mut ServerLevel, player: EntityId, source: &DamageSource, original_damage: f32, actual_damage: f32, blocked: bool) -> bool {
        if !self.dealt_damage.matches(original_damage as f64) {
            return false;
        }
        if !self.taken_damage.matches(actual_damage as f64) {
            return false;
        }
        if self.source_entity.as_ref().is_some_and(|source_entity| !source_entity.matches_player(level, player, source.causing_entity)) {
            return false;
        }
        if self.blocked.is_some_and(|expected| expected != blocked) {
            return false;
        }
        self.ty.as_ref().is_none_or(|ty| {
            let position = level.entity(player).position();
            ty.matches(level, Some(position), source)
        })
    }
}
