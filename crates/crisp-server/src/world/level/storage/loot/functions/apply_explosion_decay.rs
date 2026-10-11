use crisp_core::random::RandomSource;

use crate::world::item::ItemStack;
use crate::world::level::storage::loot::loot_context::LootContext;

pub struct ApplyExplosionDecay;

impl ApplyExplosionDecay {
    pub fn run(&self, mut item_stack: ItemStack, context: &mut LootContext) -> ItemStack {
        if let Some(explosion_radius) = context.params.explosion_radius {
            let probability = 1.0 / explosion_radius;
            let current_count = item_stack.count;
            let mut result_count = 0;
            for _ in 0..current_count {
                if context.random().next_float() <= probability {
                    result_count += 1;
                }
            }
            item_stack.count = result_count;
        }
        item_stack
    }
}
