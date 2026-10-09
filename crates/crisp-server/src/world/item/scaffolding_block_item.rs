use crisp_core::Direction;
use crisp_data::Block;

use super::block_item::BlockItemBehaviour;
use super::context::BlockPlaceContext;
use crate::server::level::ServerLevel;
use crate::world::level::BlockGetter;
use crate::world::level::block;
use crate::world::level::block::scaffolding_block::get_distance;

pub struct ScaffoldingBlockItem {
    pub block: Block,
}

impl BlockItemBehaviour for ScaffoldingBlockItem {
    fn block(&self) -> Block {
        self.block
    }

    fn update_placement_context(&self, level: &mut ServerLevel, ctx: BlockPlaceContext) -> Option<BlockPlaceContext> {
        let pos = ctx.clicked_pos();
        let replaced = level.block_state(pos);
        if !replaced.is_block(self.block) {
            return (get_distance(level, pos) != 7).then_some(ctx);
        }
        let direction = if ctx.is_secondary_use_active() {
            if ctx.is_inside() { ctx.clicked_face().opposite() } else { ctx.clicked_face() }
        } else if ctx.clicked_face() == Direction::Up {
            ctx.horizontal_direction()
        } else {
            Direction::Up
        };
        let mut horizontal_distance = 0;
        let mut placement_pos = pos.relative(direction);
        while horizontal_distance < 7 {
            if !level.is_in_valid_bounds(placement_pos) {
                break;
            }
            let replaced = level.block_state(placement_pos);
            if !replaced.is_block(self.block) {
                if block::of(replaced).can_be_replaced(replaced, &ctx) {
                    return Some(ctx.at(level, placement_pos, direction));
                }
                break;
            }
            placement_pos = placement_pos.relative(direction);
            if direction.axis().is_horizontal() {
                horizontal_distance += 1;
            }
        }
        None
    }

    fn must_survive(&self) -> bool {
        false
    }
}
