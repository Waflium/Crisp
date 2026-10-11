use crisp_core::{BlockPos, Direction};
use crisp_data::properties::{POWERED, RAIL_SHAPE_STRAIGHT, RailShape};
use crisp_data::{Block, BlockState};

use super::BlockBehaviour;
use super::base_rail_block::{self as base, BaseRailBlock};
use crate::server::level::ServerLevel;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::{BlockGetter, LevelReader};

pub struct PoweredRailBlock {
    pub block: Block,
}

impl PoweredRailBlock {
    fn find_powered_rail_signal(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState, forward: bool, search_depth: i32) -> bool {
        if search_depth >= 8 {
            return false;
        }
        let (mut x, mut y, mut z) = (pos.x, pos.y, pos.z);
        let mut check_below = true;
        let mut shape = state.get(RAIL_SHAPE_STRAIGHT);
        match shape {
            RailShape::NorthSouth => {
                if forward {
                    z += 1;
                } else {
                    z -= 1;
                }
            }
            RailShape::EastWest => {
                if forward {
                    x -= 1;
                } else {
                    x += 1;
                }
            }
            RailShape::AscendingEast => {
                if forward {
                    x -= 1;
                } else {
                    x += 1;
                    y += 1;
                    check_below = false;
                }
                shape = RailShape::EastWest;
            }
            RailShape::AscendingWest => {
                if forward {
                    x -= 1;
                    y += 1;
                    check_below = false;
                } else {
                    x += 1;
                }
                shape = RailShape::EastWest;
            }
            RailShape::AscendingNorth => {
                if forward {
                    z += 1;
                } else {
                    z -= 1;
                    y += 1;
                    check_below = false;
                }
                shape = RailShape::NorthSouth;
            }
            RailShape::AscendingSouth => {
                if forward {
                    z += 1;
                    y += 1;
                    check_below = false;
                } else {
                    z -= 1;
                }
                shape = RailShape::NorthSouth;
            }
            _ => {}
        }
        self.is_same_rail_with_power(level, BlockPos::new(x, y, z), forward, search_depth, shape)
            || check_below && self.is_same_rail_with_power(level, BlockPos::new(x, y - 1, z), forward, search_depth, shape)
    }

    fn is_same_rail_with_power(&self, level: &mut ServerLevel, pos: BlockPos, forward: bool, search_depth: i32, dir: RailShape) -> bool {
        let state = level.block_state(pos);
        if !state.is_block(self.block) {
            return false;
        }
        let my_shape = state.get(RAIL_SHAPE_STRAIGHT);
        if dir == RailShape::EastWest && matches!(my_shape, RailShape::NorthSouth | RailShape::AscendingNorth | RailShape::AscendingSouth) {
            return false;
        }
        if dir == RailShape::NorthSouth && matches!(my_shape, RailShape::EastWest | RailShape::AscendingEast | RailShape::AscendingWest) {
            return false;
        }
        if !state.get(POWERED) {
            return false;
        }
        level.has_neighbor_signal(pos) || self.find_powered_rail_signal(level, pos, state, forward, search_depth + 1)
    }
}

impl BaseRailBlock for PoweredRailBlock {
    fn block(&self) -> Block {
        self.block
    }

    fn update_state_for_neighbor(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, _block: Block) {
        let is_powered = state.get(POWERED);
        let should_power = level.has_neighbor_signal(pos)
            || self.find_powered_rail_signal(level, pos, state, true, 0)
            || self.find_powered_rail_signal(level, pos, state, false, 0);
        if should_power != is_powered {
            level.set_block_simple(pos, state.set(POWERED, should_power));
            level.update_neighbors_at(pos.below(), self.block);
            if base::is_slope(state.get(RAIL_SHAPE_STRAIGHT)) {
                level.update_neighbors_at(pos.above(), self.block);
            }
        }
    }
}

impl BlockBehaviour for PoweredRailBlock {
    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        Some(base::get_state_for_placement(block, ctx, level))
    }

    fn can_survive(&self, _state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> bool {
        base::can_survive(level, pos)
    }

    fn on_place(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, old_state: BlockState, moved_by_piston: bool) {
        base::on_place(self, state, level, pos, old_state, moved_by_piston);
    }

    fn neighbor_changed(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, block: Block, moved_by_piston: bool) {
        base::neighbor_changed(self, state, level, pos, block, moved_by_piston);
    }

    fn affect_neighbors_after_removal(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, moved_by_piston: bool) {
        base::affect_neighbors_after_removal(self, state, level, pos, moved_by_piston);
    }

    fn update_shape(
        &self,
        state: BlockState,
        level: &mut ServerLevel,
        pos: BlockPos,
        _direction: Direction,
        _neighbour_pos: BlockPos,
        _neighbour_state: BlockState,
    ) -> BlockState {
        base::update_shape(state, level, pos)
    }
}
