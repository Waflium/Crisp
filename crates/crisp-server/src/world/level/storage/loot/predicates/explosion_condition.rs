use crisp_core::random::RandomSource;

use crate::world::level::storage::loot::loot_context::LootContext;

pub struct ExplosionCondition;

impl ExplosionCondition {
    pub fn test(&self, context: &mut LootContext) -> bool {
        let Some(explosion_radius) = context.params.explosion_radius else { return true };
        let probability = 1.0 / explosion_radius;
        context.random().next_float() <= probability
    }
}
