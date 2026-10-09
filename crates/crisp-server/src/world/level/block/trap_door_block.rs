use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, Direction};
use crisp_data::properties::{HALF, HORIZONTAL_FACING, Half, OPEN, POWERED, WATERLOGGED};
use crisp_data::{Block, BlockState};

use super::block_set_type::{BlockSetType, fields};
use super::simple_waterlogged_block::{is_water_source, schedule_water_tick};
use super::{BlockBehaviour, BlockHitResult};
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::InteractionResult;
use crate::world::entity::EntityId;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::update_flags::UPDATE_CLIENTS;

pub struct TrapDoorBlock {
    set_type: BlockSetType,
}

impl TrapDoorBlock {
    pub fn new(block: Block) -> TrapDoorBlock {
        TrapDoorBlock { set_type: BlockSetType::from_json(&fields(block)["type"]) }
    }

    fn toggle(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, player: Option<EntityId>) {
        let updated = state.cycle(OPEN);
        level.set_block(pos, updated, UPDATE_CLIENTS);
        schedule_water_tick(updated, level, pos);
        self.play_sound(player, level, pos, updated.get(OPEN));
    }

    fn play_sound(&self, player: Option<EntityId>, level: &mut ServerLevel, pos: BlockPos, opening: bool) {
        let pitch = level.random.next_float() * 0.1 + 0.9;
        let sound = if opening { self.set_type.trapdoor_open } else { self.set_type.trapdoor_close };
        level.play_sound(player, pos, sound, SoundSource::Blocks, 1.0, pitch);
    }
}

impl BlockBehaviour for TrapDoorBlock {
    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        let mut state = block.default_state();
        let water = is_water_source(level, ctx.clicked_pos());
        let clicked_face = ctx.clicked_face();
        if !ctx.replacing_clicked_on_block() && clicked_face.axis().is_horizontal() {
            let half = if ctx.click_location().y - ctx.clicked_pos().y as f64 > 0.5 { Half::Top } else { Half::Bottom };
            state = state.set(HORIZONTAL_FACING, clicked_face).set(HALF, half);
        } else {
            let half = if clicked_face == Direction::Up { Half::Bottom } else { Half::Top };
            state = state.set(HORIZONTAL_FACING, ctx.horizontal_direction().opposite()).set(HALF, half);
        }
        if level.has_neighbor_signal(ctx.clicked_pos()) {
            state = state.set(OPEN, true).set(POWERED, true);
        }
        Some(state.set(WATERLOGGED, water))
    }

    fn use_without_item(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, player: EntityId, _hit: &BlockHitResult) -> InteractionResult {
        if !self.set_type.can_open_by_hand {
            return InteractionResult::Pass;
        }
        self.toggle(state, level, pos, Some(player));
        InteractionResult::SUCCESS
    }

    fn neighbor_changed(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, _block: Block, _moved_by_piston: bool) {
        let signal = level.has_neighbor_signal(pos);
        if signal == state.get(POWERED) {
            return;
        }
        let mut state = state;
        if state.get(OPEN) != signal {
            state = state.set(OPEN, signal);
            self.play_sound(None, level, pos, signal);
        }
        level.set_block(pos, state.set(POWERED, signal), UPDATE_CLIENTS);
        schedule_water_tick(state, level, pos);
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
        state
    }
}

#[cfg(test)]
mod tests {
    use crisp_core::{BlockPos, Direction, Vec3};
    use crisp_data::properties::{HALF, HORIZONTAL_FACING, Half, OPEN, POWERED};
    use crisp_data::{blocks, items};

    use crate::server::level::testing;
    use crate::world::InteractionResult;
    use crate::world::item::ItemStack;
    use crate::world::level::BlockGetter;
    use crate::world::level::block::click;
    use crate::world::level::update_flags::UPDATE_ALL;

    #[test]
    fn placement_opening_and_power() {
        let mut level = testing::level();
        let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 3.5), 180.0, 30.0);
        level.player_mut(player).inventory.slots[36] = Some(ItemStack::new(items::OAK_TRAPDOOR, 64));
        let wall = BlockPos::new(0, -60, 0);
        level.set_block(wall, blocks::STONE.default_state(), UPDATE_ALL);
        click(&mut level, player, wall, Direction::South, Vec3::new(0.5, -59.2, 1.0));
        let pos = wall.south();
        let state = level.block_state(pos);
        assert_eq!(state.get(HORIZONTAL_FACING), Direction::South);
        assert_eq!(state.get(HALF), Half::Top);
        click(&mut level, player, BlockPos::new(3, -61, 0), Direction::Up, Vec3::new(3.5, -60.0, 0.5));
        let ground = level.block_state(BlockPos::new(3, -60, 0));
        assert_eq!(ground.get(HORIZONTAL_FACING), Direction::South);
        assert_eq!(ground.get(HALF), Half::Bottom);

        level.player_mut(player).inventory.slots[36] = None;
        assert_eq!(click(&mut level, player, pos, Direction::South, Vec3::new(0.5, -59.5, 2.0)), InteractionResult::SUCCESS);
        assert!(level.block_state(pos).get(OPEN));

        level.set_block(pos.east(), blocks::LIGHTNING_ROD.default_state().set(POWERED, true), UPDATE_ALL);
        let state = level.block_state(pos);
        assert!(state.get(OPEN) && state.get(POWERED));
        level.set_block(pos.east(), blocks::AIR.default_state(), UPDATE_ALL);
        let state = level.block_state(pos);
        assert!(!state.get(OPEN) && !state.get(POWERED));

        level.set_block(pos, blocks::IRON_TRAPDOOR.default_state(), UPDATE_ALL);
        assert_eq!(click(&mut level, player, pos, Direction::South, Vec3::new(0.5, -59.5, 2.0)), InteractionResult::Pass);
    }
}
