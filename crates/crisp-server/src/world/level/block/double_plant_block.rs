use crisp_core::{Axis, BlockPos, Direction};
use crisp_data::properties::{DOUBLE_BLOCK_HALF, DoubleBlockHalf, WATERLOGGED};
use crisp_data::{Block, BlockState, Fluid, blocks};

use super::BlockBehaviour;
use super::vegetation_block::{self, Vegetation};
use crate::server::level::ServerLevel;
use crate::server::level::events::PARTICLES_AND_SOUND_DESTROY_BLOCK;
use crate::server::level::game_mode::GameType;
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::{BlockGetter, LevelReader};
use crate::world::level::update_flags::{UPDATE_ALL, UPDATE_CLIENTS, UPDATE_NEIGHBORS, UPDATE_SUPPRESS_DROPS};

pub fn update_shape(state: BlockState, level: &mut ServerLevel, pos: BlockPos, direction: Direction, neighbour_state: BlockState) -> BlockState {
    let half = state.get(DOUBLE_BLOCK_HALF);
    if direction.axis() != Axis::Y
        || (half == DoubleBlockHalf::Lower) != (direction == Direction::Up)
        || neighbour_state.is_block(state.block()) && neighbour_state.get(DOUBLE_BLOCK_HALF) != half
    {
        if half == DoubleBlockHalf::Lower && direction == Direction::Down && !super::of(state).can_survive(state, level, pos) {
            blocks::AIR.default_state()
        } else {
            vegetation_block::update_shape(state, level, pos)
        }
    } else {
        blocks::AIR.default_state()
    }
}

pub fn get_state_for_placement(block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
    let pos = ctx.clicked_pos();
    if pos.y >= level.chunks.max_y() {
        return None;
    }
    let above = level.block_state(pos.above());
    super::of(above).can_be_replaced(above, ctx).then(|| block.default_state())
}

pub fn set_placed_by(level: &mut ServerLevel, pos: BlockPos, state: BlockState) {
    let above = pos.above();
    let upper = copy_waterlogged_from(level, above, state.block().default_state().set(DOUBLE_BLOCK_HALF, DoubleBlockHalf::Upper));
    level.set_block(above, upper, UPDATE_ALL);
}

pub fn can_survive<V: Vegetation + ?Sized>(vegetation: &V, state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> bool {
    if state.get(DOUBLE_BLOCK_HALF) != DoubleBlockHalf::Upper {
        return vegetation_block::can_survive(vegetation, level, pos);
    }
    let below = level.block_state(pos.below());
    below.is_block(state.block()) && below.get(DOUBLE_BLOCK_HALF) == DoubleBlockHalf::Lower
}

pub fn place_at(level: &mut ServerLevel, state: BlockState, lower_pos: BlockPos, flags: i32) {
    let upper_pos = lower_pos.above();
    let lower = copy_waterlogged_from(level, lower_pos, state.set(DOUBLE_BLOCK_HALF, DoubleBlockHalf::Lower));
    level.set_block(lower_pos, lower, flags);
    let upper = copy_waterlogged_from(level, upper_pos, state.set(DOUBLE_BLOCK_HALF, DoubleBlockHalf::Upper));
    level.set_block(upper_pos, upper, flags);
}

pub fn copy_waterlogged_from(level: &mut ServerLevel, pos: BlockPos, state: BlockState) -> BlockState {
    if state.has(WATERLOGGED) { state.set(WATERLOGGED, level.is_water_at(pos)) } else { state }
}

pub fn player_will_destroy(level: &mut ServerLevel, pos: BlockPos, state: BlockState, player: EntityId) -> BlockState {
    let p = level.player(player);
    if p.game_mode.game_type == GameType::Creative {
        prevent_drop_from_bottom_part(level, pos, state, player);
    } else {
        let tool = p.inventory.held().cloned();
        level.drop_resources(state, pos, Some(player), tool);
    }
    level.level_event(Some(player), PARTICLES_AND_SOUND_DESTROY_BLOCK, pos, state.id() as i32);
    state
}

pub fn player_destroy(level: &mut ServerLevel, player: EntityId, pos: BlockPos, tool: Option<ItemStack>) {
    level.drop_resources(blocks::AIR.default_state(), pos, Some(player), tool);
}

pub fn prevent_drop_from_bottom_part(level: &mut ServerLevel, pos: BlockPos, state: BlockState, player: EntityId) {
    if state.get(DOUBLE_BLOCK_HALF) != DoubleBlockHalf::Upper {
        return;
    }
    let bottom_pos = pos.below();
    let bottom = level.block_state(bottom_pos);
    if bottom.is_block(state.block()) && bottom.get(DOUBLE_BLOCK_HALF) == DoubleBlockHalf::Lower {
        let replacement = if bottom.fluid_state().fluid == Fluid::Water { blocks::WATER.default_state() } else { blocks::AIR.default_state() };
        level.set_block(bottom_pos, replacement, UPDATE_NEIGHBORS | UPDATE_CLIENTS | UPDATE_SUPPRESS_DROPS);
        level.level_event(Some(player), PARTICLES_AND_SOUND_DESTROY_BLOCK, bottom_pos, bottom.id() as i32);
    }
}

pub struct DoublePlantBlock;

impl Vegetation for DoublePlantBlock {}

impl BlockBehaviour for DoublePlantBlock {
    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        get_state_for_placement(block, ctx, level)
    }

    fn set_placed_by(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState, _placer: Option<EntityId>, _item: ItemStack) {
        set_placed_by(level, pos, state);
    }

    fn update_shape(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, direction: Direction, _: BlockPos, neighbour_state: BlockState) -> BlockState {
        update_shape(state, level, pos, direction, neighbour_state)
    }

    fn can_survive(&self, state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> bool {
        can_survive(self, state, level, pos)
    }

    fn player_will_destroy(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState, player: EntityId) -> BlockState {
        player_will_destroy(level, pos, state, player)
    }

    fn player_destroy(&self, level: &mut ServerLevel, player: EntityId, pos: BlockPos, _state: BlockState, _block_entity: Option<&crate::world::level::block::entity::BlockEntity>, tool: Option<ItemStack>) {
        player_destroy(level, player, pos, tool);
    }
}

#[cfg(test)]
mod tests {
    use crisp_core::{BlockPos, Direction, Vec3};
    use crisp_data::properties::{DOUBLE_BLOCK_HALF, DoubleBlockHalf};
    use crisp_data::{blocks, items};

    use crate::server::level::game_mode::GameType;
    use crate::server::level::{ServerLevel, testing};
    use crate::world::InteractionHand;
    use crate::world::entity::EntityId;
    use crate::world::item::ItemStack;
    use crate::world::level::BlockGetter;
    use crate::world::level::block::BlockHitResult;
    use crate::world::level::update_flags::UPDATE_ALL;

    const LOWER: BlockPos = BlockPos::new(0, -60, 0);

    fn place_sunflower() -> (ServerLevel, EntityId) {
        let mut level = testing::level();
        let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 3.5), 180.0, 30.0);
        level.player_mut(player).inventory.slots[36] = Some(ItemStack::new(items::SUNFLOWER, 1));
        let hit = BlockHitResult { location: Vec3::new(0.5, -60.0, 0.5), direction: Direction::Up, block_pos: LOWER.below(), inside: false, world_border_hit: false };
        level.use_item_on(player, InteractionHand::MainHand, hit);
        (level, player)
    }

    fn dropped(level: &ServerLevel) -> Vec<ItemStack> {
        level.entities.values().filter_map(|e| e.as_item()).map(|i| i.item.clone()).collect()
    }

    #[test]
    fn placing_a_tall_flower_places_both_halves() {
        let (mut level, _) = place_sunflower();
        let lower = blocks::SUNFLOWER.default_state().set(DOUBLE_BLOCK_HALF, DoubleBlockHalf::Lower);
        assert_eq!(level.block_state(LOWER), lower);
        assert_eq!(level.block_state(LOWER.above()), lower.set(DOUBLE_BLOCK_HALF, DoubleBlockHalf::Upper));
    }

    #[test]
    fn breaking_the_top_half_removes_the_plant_with_one_drop() {
        let (mut level, player) = place_sunflower();
        level.destroy_block_by_player(player, LOWER.above());
        assert!(level.block_state(LOWER).is_air());
        assert!(level.block_state(LOWER.above()).is_air());
        assert_eq!(dropped(&level), vec![ItemStack::new(items::SUNFLOWER, 1)]);
    }

    #[test]
    fn breaking_the_top_half_in_creative_drops_nothing() {
        let (mut level, player) = place_sunflower();
        level.player_mut(player).game_mode.game_type = GameType::Creative;
        level.destroy_block_by_player(player, LOWER.above());
        assert!(level.block_state(LOWER).is_air());
        assert!(level.block_state(LOWER.above()).is_air());
        assert!(dropped(&level).is_empty());
    }

    #[test]
    fn the_top_half_breaks_with_the_ground() {
        let (mut level, _) = place_sunflower();
        level.set_block(LOWER.below(), blocks::STONE.default_state(), UPDATE_ALL);
        assert!(level.block_state(LOWER).is_air());
        assert!(level.block_state(LOWER.above()).is_air());
        assert_eq!(dropped(&level), vec![ItemStack::new(items::SUNFLOWER, 1)]);
    }
}
