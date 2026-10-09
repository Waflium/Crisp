use crisp_core::{BlockPos, Direction};
use crisp_data::properties::{BOTTOM, STABILITY_DISTANCE, WATERLOGGED};
use crisp_data::{Block, BlockState, SupportType, blocks};

use super::BlockBehaviour;
use super::simple_waterlogged_block::{is_water_source, schedule_water_tick};
use crate::server::level::ServerLevel;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::BlockGetter;
use crate::world::level::update_flags::UPDATE_ALL;

const TICK_DELAY: i32 = 1;
const STABILITY_MAX_DISTANCE: i32 = 7;

pub struct ScaffoldingBlock;

fn is_bottom(block: Block, level: &mut ServerLevel, pos: BlockPos, distance: i32) -> bool {
    distance > 0 && !level.block_state(pos.below()).is_block(block)
}

pub fn get_distance(level: &mut ServerLevel, pos: BlockPos) -> i32 {
    let below_pos = pos.below();
    let below = level.block_state(below_pos);
    let mut distance = STABILITY_MAX_DISTANCE;
    if below.is_block(blocks::SCAFFOLDING) {
        distance = below.get(STABILITY_DISTANCE);
    } else if below.is_face_sturdy(Direction::Up, SupportType::Full) {
        return 0;
    }
    for direction in Direction::PLANE_HORIZONTAL {
        let relative = level.block_state(pos.relative(direction));
        if relative.is_block(blocks::SCAFFOLDING) {
            distance = distance.min(relative.get(STABILITY_DISTANCE) + 1);
            if distance == 1 {
                break;
            }
        }
    }
    distance
}

impl BlockBehaviour for ScaffoldingBlock {
    fn can_be_replaced(&self, state: BlockState, ctx: &BlockPlaceContext) -> bool {
        ctx.item.item == state.block().item()
    }

    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        let pos = ctx.clicked_pos();
        let distance = get_distance(level, pos);
        let water = is_water_source(level, pos);
        let bottom = is_bottom(block, level, pos, distance);
        Some(block.default_state().set(WATERLOGGED, water).set(STABILITY_DISTANCE, distance).set(BOTTOM, bottom))
    }

    fn on_place(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, _old_state: BlockState, _moved_by_piston: bool) {
        level.schedule_tick(pos, state.block(), TICK_DELAY);
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
        schedule_water_tick(state, level, pos);
        level.schedule_tick(pos, state.block(), TICK_DELAY);
        state
    }

    fn tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        let distance = get_distance(level, pos);
        let new_state = state.set(STABILITY_DISTANCE, distance).set(BOTTOM, is_bottom(state.block(), level, pos, distance));
        if new_state.get(STABILITY_DISTANCE) == STABILITY_MAX_DISTANCE {
            if state.get(STABILITY_DISTANCE) != STABILITY_MAX_DISTANCE {
                level.destroy_block(pos, true, None, crate::world::level::update_flags::UPDATE_LIMIT);
            }
        } else if state != new_state {
            level.set_block(pos, new_state, UPDATE_ALL);
        }
    }

    fn can_survive(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos) -> bool {
        get_distance(level, pos) < STABILITY_MAX_DISTANCE
    }
}

#[cfg(test)]
mod tests {
    use crisp_core::{BlockPos, Direction, Vec3};
    use crisp_data::properties::{BOTTOM, STABILITY_DISTANCE};
    use crisp_data::{blocks, items};

    use crate::server::level::testing;
    use crate::world::item::ItemStack;
    use crate::world::level::BlockGetter;
    use crate::world::level::block::click;

    #[test]
    fn distance_and_breaking() {
        let mut level = testing::level();
        let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 5.5), 180.0, 30.0);
        level.player_mut(player).inventory.slots[36] = Some(ItemStack::new(items::SCAFFOLDING, 64));
        let base = BlockPos::new(0, -60, 0);
        click(&mut level, player, base.below(), Direction::Up, Vec3::new(0.5, -60.0, 0.5));
        assert_eq!(level.block_state(base).get(STABILITY_DISTANCE), 0);
        click(&mut level, player, base, Direction::South, Vec3::new(0.5, -59.5, 1.0));
        let top = base.above();
        assert!(level.block_state(top).is_block(blocks::SCAFFOLDING));
        assert_eq!(level.block_state(top).get(STABILITY_DISTANCE), 0);
        click(&mut level, player, top, Direction::Up, Vec3::new(0.5, -58.0, 0.5));
        let arm = top.north();
        let state = level.block_state(arm);
        assert!(state.is_block(blocks::SCAFFOLDING), "{state:?}");
        assert_eq!(state.get(STABILITY_DISTANCE), 1);
        assert!(state.get(BOTTOM));

        level.set_block(top, blocks::AIR.default_state(), crate::world::level::update_flags::UPDATE_ALL);
        level.tick();
        assert!(level.block_state(arm).is_air());
    }
}
