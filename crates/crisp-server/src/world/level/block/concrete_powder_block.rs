use crisp_core::{BlockPos, Direction};
use crisp_data::{Block, BlockState, SupportType, fluid_tags};

use super::BlockBehaviour;
use crate::server::level::ServerLevel;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::{BlockGetter, LevelAccessor, LevelReader};

pub struct ConcretePowderBlock {
    concrete: Block,
}

impl ConcretePowderBlock {
    pub fn new(block: Block) -> ConcretePowderBlock {
        let fields: serde_json::Value = serde_json::from_str(block.info().fields).unwrap_or_default();
        let concrete = fields["concrete"].as_str().and_then(Block::by_name).expect("concrete powder hardens into a block");
        ConcretePowderBlock { concrete }
    }
}

fn should_solidify(level: &mut ServerLevel, pos: BlockPos, replaced_block: BlockState) -> bool {
    can_solidify(replaced_block) || touches_liquid(level, pos)
}

fn touches_liquid(level: &mut dyn LevelReader, pos: BlockPos) -> bool {
    let mut test_pos = pos;
    for direction in Direction::VALUES {
        let block_state = level.block_state(test_pos);
        if direction != Direction::Down || can_solidify(block_state) {
            test_pos = pos.relative(direction);
            let block_state = level.block_state(test_pos);
            if can_solidify(block_state) && !block_state.is_face_sturdy(direction.opposite(), SupportType::Full) {
                return true;
            }
        }
    }
    false
}

fn can_solidify(state: BlockState) -> bool {
    state.fluid_state().is(&fluid_tags::WATER)
}

impl BlockBehaviour for ConcretePowderBlock {
    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        let pos = ctx.clicked_pos();
        let replaced_block = level.block_state(pos);
        Some(if should_solidify(level, pos, replaced_block) { self.concrete.default_state() } else { block.default_state() })
    }

    fn update_shape(
        &self,
        state: BlockState,
        level: &mut dyn LevelAccessor,
        pos: BlockPos,
        _direction: Direction,
        _neighbour_pos: BlockPos,
        _neighbour_state: BlockState,
    ) -> BlockState {
        if touches_liquid(level, pos) { self.concrete.default_state() } else { state }
    }
}

#[cfg(test)]
mod tests {
    use crisp_core::{BlockPos, Direction, Vec3};
    use crisp_data::{blocks, items};

    use crate::server::level::testing;
    use crate::world::InteractionHand;
    use crate::world::item::ItemStack;
    use crate::world::level::BlockGetter;
    use crate::world::level::block::BlockHitResult;
    use crate::world::level::update_flags::UPDATE_ALL;

    #[test]
    fn concrete_powder_hardens_next_to_water() {
        let mut level = testing::level();
        let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 2.5), 180.0, 30.0);
        level.player_mut(player).inventory.slots[36] = Some(ItemStack::new(items::WHITE_CONCRETE_POWDER, 2));
        level.set_block(BlockPos::new(1, -60, 0), blocks::WATER.default_state(), UPDATE_ALL);
        let grass = BlockPos::new(0, -61, 0);
        let hit = BlockHitResult { location: Vec3::new(0.5, -60.0, 0.5), direction: Direction::Up, block_pos: grass, inside: false, world_border_hit: false };
        level.use_item_on(player, InteractionHand::MainHand, hit);
        assert_eq!(level.block_state(grass.above()), blocks::WHITE_CONCRETE.default_state());

        let grass = BlockPos::new(-2, -61, 1);
        let hit = BlockHitResult { location: Vec3::new(-1.5, -60.0, 1.5), direction: Direction::Up, block_pos: grass, inside: false, world_border_hit: false };
        level.use_item_on(player, InteractionHand::MainHand, hit);
        assert_eq!(level.block_state(grass.above()), blocks::WHITE_CONCRETE_POWDER.default_state());
        level.set_block(grass.above().west(), blocks::WATER.default_state(), UPDATE_ALL);
        assert_eq!(level.block_state(grass.above()), blocks::WHITE_CONCRETE.default_state());
    }
}
