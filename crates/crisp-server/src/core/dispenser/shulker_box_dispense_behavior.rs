use crisp_core::Direction;

use super::block_source::BlockSource;
use crate::server::level::ServerLevel;
use crate::world::item::context::BlockPlaceContext;
use crate::world::item::{ItemStack, block_item};
use crate::world::level::BlockGetter;

pub fn execute(level: &mut ServerLevel, source: &BlockSource, mut dispensed: ItemStack, success: &mut bool) -> ItemStack {
    *success = false;
    if let Some(block_item) = crate::world::item::block_item_behaviour(dispensed.item) {
        let facing = source.facing();
        let relative_pos = source.pos.relative(facing);
        let clicked_face = if level.block_state(relative_pos.below()).is_air() { facing } else { Direction::Up };
        let context = BlockPlaceContext::directional(level, relative_pos, facing, dispensed.clone(), clicked_face);
        *success = block_item::place(block_item, level, context).consumes_action();
        if *success {
            dispensed.shrink(1);
        }
    }
    dispensed
}
