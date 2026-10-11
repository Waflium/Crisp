use crisp_core::AABB;

use super::ServerLevel;
use crate::world::entity::ai::targeting::targeting_conditions::TargetingConditions;
use crate::world::entity::{EntityId, LevelEntity};

impl ServerLevel {
    pub fn get_nearest_player(&mut self, target_conditions: &TargetingConditions, source: Option<EntityId>, x: f64, y: f64, z: f64) -> Option<EntityId> {
        let mut best = -1.0;
        let mut result = None;
        for i in 0..self.players.len() {
            let player = *self.players.get_index(i).expect("a player").0;
            if target_conditions.test(self, source, player) {
                let dist = self.entity(player).distance_to_sqr_xyz(x, y, z);
                if best == -1.0 || dist < best {
                    best = dist;
                    result = Some(player);
                }
            }
        }
        result
    }

    pub fn get_nearest_entity(&mut self, entities: &[EntityId], target_conditions: &TargetingConditions, source: Option<EntityId>, x: f64, y: f64, z: f64) -> Option<EntityId> {
        let mut best = -1.0;
        let mut result = None;
        for &entity in entities {
            if target_conditions.test(self, source, entity) {
                let dist = self.entity(entity).distance_to_sqr_xyz(x, y, z);
                if best == -1.0 || dist < best {
                    best = dist;
                    result = Some(entity);
                }
            }
        }
        result
    }

    pub fn get_entities_of_class_same_kind(&self, mob: EntityId, bb: &AABB) -> Vec<EntityId> {
        let mut entities = self.get_entities(None, bb);
        entities.retain(|&entity| self.is_same_mob_class(mob, entity));
        entities
    }

    pub fn get_nearby_entities_same_kind(&mut self, target_conditions: &TargetingConditions, source: EntityId, bb: &AABB) -> Vec<EntityId> {
        let nearby = self.get_entities_of_class_same_kind(source, bb);
        nearby.into_iter().filter(|&entity| target_conditions.test(self, Some(source), entity)).collect()
    }

    fn is_same_mob_class(&self, mob: EntityId, other: EntityId) -> bool {
        match (self.entities.get(&mob), self.entities.get(&other)) {
            (Some(LevelEntity::Mob(a)), Some(LevelEntity::Mob(b))) => std::mem::discriminant(&a.kind) == std::mem::discriminant(&b.kind),
            _ => false,
        }
    }
}
