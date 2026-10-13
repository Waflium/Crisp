use crisp_core::Vec3;

use super::entity_hit_result::EntityHitResult;
use crate::world::level::block::BlockHitResult;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Type {
    Miss,
    Block,
    Entity,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum HitResult {
    Block { hit: BlockHitResult, miss: bool },
    Entity(EntityHitResult),
}

impl HitResult {
    pub fn get_type(&self) -> Type {
        match self {
            HitResult::Block { miss: true, .. } => Type::Miss,
            HitResult::Block { .. } => Type::Block,
            HitResult::Entity(_) => Type::Entity,
        }
    }

    pub fn get_location(&self) -> Vec3 {
        match self {
            HitResult::Block { hit, .. } => hit.location,
            HitResult::Entity(hit) => hit.location,
        }
    }
}
