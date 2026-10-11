use crisp_core::AABB;
use crisp_protocol::nbt::Nbt;

use super::codec::as_f32;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AttackRange {
    pub min_reach: f32,
    pub max_reach: f32,
    pub min_creative_reach: f32,
    pub max_creative_reach: f32,
    pub hitbox_margin: f32,
    pub mob_factor: f32,
}

impl AttackRange {
    pub fn from_nbt(tag: &Nbt) -> AttackRange {
        let field = |key: &str, default: f32| tag.get(key).map_or(default, as_f32);
        AttackRange {
            min_reach: field("min_reach", 0.0),
            max_reach: field("max_reach", 3.0),
            min_creative_reach: field("min_creative_reach", 0.0),
            max_creative_reach: field("max_creative_reach", 5.0),
            hitbox_margin: field("hitbox_margin", 0.3),
            mob_factor: field("mob_factor", 1.0),
        }
    }

    pub fn default_for(interaction_range: f32) -> AttackRange {
        AttackRange { min_reach: 0.0, max_reach: interaction_range, min_creative_reach: 0.0, max_creative_reach: interaction_range, hitbox_margin: 0.0, mob_factor: 1.0 }
    }

    pub fn effective_min_range(&self, level: &ServerLevel, entity: EntityId) -> f32 {
        match level.players.get(&entity) {
            Some(player) if player.game_mode.game_type.is_creative() => self.min_creative_reach,
            Some(_) => self.min_reach,
            None => self.min_reach * self.mob_factor,
        }
    }

    pub fn effective_max_range(&self, level: &ServerLevel, entity: EntityId) -> f32 {
        match level.players.get(&entity) {
            Some(player) if player.game_mode.game_type.is_creative() => self.max_creative_reach,
            Some(_) => self.max_reach,
            None => self.max_reach * self.mob_factor,
        }
    }

    pub fn is_in_range(&self, level: &ServerLevel, attacker: EntityId, bounding_box: &AABB, extra_buffer: f64) -> bool {
        let distance = bounding_box.distance_to_sqr(level.entity(attacker).eye_position()).sqrt();
        let min_reach = (self.effective_min_range(level, attacker) - self.hitbox_margin) as f64 - extra_buffer;
        let max_reach = (self.effective_max_range(level, attacker) + self.hitbox_margin) as f64 + extra_buffer;
        distance >= min_reach && distance <= max_reach
    }
}
