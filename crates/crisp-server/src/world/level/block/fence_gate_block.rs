use crisp_core::random::RandomSource;
use crisp_core::{Axis, BlockPos, Direction};
use crisp_data::properties::{HORIZONTAL_FACING, IN_WALL, OPEN, POWERED};
use crisp_data::{Block, BlockState, block_tags};

use super::block_set_type::{WoodType, fields};
use super::{BlockBehaviour, BlockHitResult};
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::InteractionResult;
use crate::world::entity::EntityId;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::BlockGetter;
use crate::world::level::update_flags::{UPDATE_CLIENTS, UPDATE_IMMEDIATE};

pub struct FenceGateBlock {
    wood_type: WoodType,
}

impl FenceGateBlock {
    pub fn new(block: Block) -> FenceGateBlock {
        FenceGateBlock { wood_type: WoodType::from_json(&fields(block)["type"]) }
    }

    fn is_wall(state: BlockState) -> bool {
        state.is(&block_tags::WALLS)
    }

    fn play_sound(&self, except: Option<EntityId>, level: &mut ServerLevel, pos: BlockPos, opens: bool) {
        let pitch = level.random.next_float() * 0.1 + 0.9;
        let sound = if opens { self.wood_type.fence_gate_open } else { self.wood_type.fence_gate_close };
        level.play_sound(except, pos, sound, SoundSource::Blocks, 1.0, pitch);
    }
}

pub fn connects_to_direction(state: BlockState, direction: Direction) -> bool {
    state.get(HORIZONTAL_FACING).axis() == direction.clockwise().axis()
}

impl BlockBehaviour for FenceGateBlock {
    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        let pos = ctx.clicked_pos();
        let is_open = level.has_neighbor_signal(pos);
        let direction = ctx.horizontal_direction();
        let axis = direction.axis();
        let in_wall = axis == Axis::Z && (Self::is_wall(level.block_state(pos.west())) || Self::is_wall(level.block_state(pos.east())))
            || axis == Axis::X && (Self::is_wall(level.block_state(pos.north())) || Self::is_wall(level.block_state(pos.south())));
        Some(block.default_state().set(HORIZONTAL_FACING, direction).set(OPEN, is_open).set(POWERED, is_open).set(IN_WALL, in_wall))
    }

    fn update_shape(
        &self,
        state: BlockState,
        level: &mut ServerLevel,
        pos: BlockPos,
        direction: Direction,
        _neighbour_pos: BlockPos,
        neighbour_state: BlockState,
    ) -> BlockState {
        if state.get(HORIZONTAL_FACING).clockwise().axis() != direction.axis() {
            return state;
        }
        let in_wall = Self::is_wall(neighbour_state) || Self::is_wall(level.block_state(pos.relative(direction.opposite())));
        state.set(IN_WALL, in_wall)
    }

    fn use_without_item(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, player: EntityId, _hit: &BlockHitResult) -> InteractionResult {
        let mut state = state;
        if state.get(OPEN) {
            state = state.set(OPEN, false);
            level.set_block(pos, state, UPDATE_CLIENTS | UPDATE_IMMEDIATE);
        } else {
            let direction = Direction::from_y_rot(level.player(player).entity.y_rot as f64);
            if state.get(HORIZONTAL_FACING) == direction.opposite() {
                state = state.set(HORIZONTAL_FACING, direction);
            }
            state = state.set(OPEN, true);
            level.set_block(pos, state, UPDATE_CLIENTS | UPDATE_IMMEDIATE);
        }
        self.play_sound(Some(player), level, pos, state.get(OPEN));
        InteractionResult::SUCCESS
    }

    fn neighbor_changed(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, _block: Block, _moved_by_piston: bool) {
        let has_power = level.has_neighbor_signal(pos);
        if state.get(POWERED) != has_power {
            level.set_block(pos, state.set(POWERED, has_power).set(OPEN, has_power), UPDATE_CLIENTS);
            if state.get(OPEN) != has_power {
                self.play_sound(None, level, pos, has_power);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crisp_core::{BlockPos, Direction, Vec3};
    use crisp_data::properties::{HORIZONTAL_FACING, IN_WALL, OPEN, POWERED};
    use crisp_data::{blocks, items};

    use crate::server::level::testing;
    use crate::world::item::ItemStack;
    use crate::world::level::BlockGetter;
    use crate::world::level::block::click;
    use crate::world::level::update_flags::UPDATE_ALL;

    #[test]
    fn opens_away_from_the_player_and_with_power() {
        let mut level = testing::level();
        let pos = BlockPos::new(0, -60, 0);
        level.set_block(pos.west(), blocks::COBBLESTONE_WALL.default_state(), UPDATE_ALL);
        let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 3.5), 180.0, 30.0);
        level.player_mut(player).inventory.slots[36] = Some(ItemStack::new(items::OAK_FENCE_GATE, 64));
        click(&mut level, player, pos.below(), Direction::Up, Vec3::new(0.5, -60.0, 0.5));
        let gate = level.block_state(pos);
        assert_eq!(gate.get(HORIZONTAL_FACING), Direction::North);
        assert!(gate.get(IN_WALL));
        assert!(!gate.get(OPEN));

        level.player_mut(player).inventory.slots[36] = None;
        level.player_mut(player).entity.y_rot = 0.0;
        click(&mut level, player, pos, Direction::North, Vec3::new(0.5, -59.5, 0.0));
        let gate = level.block_state(pos);
        assert!(gate.get(OPEN));
        assert_eq!(gate.get(HORIZONTAL_FACING), Direction::South);
        click(&mut level, player, pos, Direction::North, Vec3::new(0.5, -59.5, 0.0));
        assert!(!level.block_state(pos).get(OPEN));

        let rod = blocks::LIGHTNING_ROD.default_state().set(POWERED, true);
        level.set_block(pos.east(), rod, UPDATE_ALL);
        let gate = level.block_state(pos);
        assert!(gate.get(OPEN) && gate.get(POWERED));
        level.set_block(pos.east(), blocks::AIR.default_state(), UPDATE_ALL);
        let gate = level.block_state(pos);
        assert!(!gate.get(OPEN) && !gate.get(POWERED));
        assert!(gate.get(IN_WALL));
    }
}
