use crisp_core::random::LegacyRandomSource;
use crisp_core::{BlockPos, Direction};
use crisp_data::{Block, BlockState};

use super::BlockBehaviour;
use super::multiface_block::MultifaceBlock;
use super::multiface_spreader::{MultifaceSpreader, SpreadConfig, SpreadRandom};
use crate::server::level::ServerLevel;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::{LevelAccessor, LevelReader};

pub struct GlowLichenBlock {
    spreader: MultifaceSpreader,
}

impl GlowLichenBlock {
    pub fn new(block: Block) -> GlowLichenBlock {
        GlowLichenBlock { spreader: MultifaceSpreader::new(SpreadConfig::Default(block)) }
    }
}

impl BlockBehaviour for GlowLichenBlock {
    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        MultifaceBlock.get_state_for_placement(block, ctx, level)
    }

    fn can_be_replaced(&self, state: BlockState, ctx: &BlockPlaceContext) -> bool {
        MultifaceBlock.can_be_replaced(state, ctx)
    }

    fn update_shape(&self, state: BlockState, level: &mut dyn LevelAccessor, pos: BlockPos, direction: Direction, neighbour_pos: BlockPos, neighbour_state: BlockState) -> BlockState {
        MultifaceBlock.update_shape(state, level, pos, direction, neighbour_pos, neighbour_state)
    }

    fn can_survive(&self, state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> bool {
        MultifaceBlock.can_survive(state, level, pos)
    }

    fn is_valid_bonemeal_target(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState) -> bool {
        Direction::VALUES.into_iter().any(|face| self.spreader.can_spread_in_any_direction(state, level, pos, face.opposite()))
    }

    fn is_bonemeal_success(&self, _level: &mut ServerLevel, _pos: BlockPos, _state: BlockState) -> bool {
        true
    }

    fn perform_bonemeal(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState) {
        self.spreader.spread_from_random_face_toward_random_direction(state, level, pos, &mut SpreadRandom::<LegacyRandomSource>::Level);
    }
}

#[cfg(test)]
mod tests {
    use crisp_core::{BlockPos, Direction, Vec3};
    use crisp_data::properties::WEST;
    use crisp_data::{blocks, items};

    use crate::server::level::testing;
    use crate::world::InteractionHand;
    use crate::world::item::ItemStack;
    use crate::world::level::BlockGetter;
    use crate::world::level::block::BlockHitResult;
    use crate::world::level::update_flags::UPDATE_ALL;

    const STONE: BlockPos = BlockPos::new(0, -58, 0);

    #[test]
    fn glow_lichen_attaches_to_the_clicked_face_and_drops_without_support() {
        let mut level = testing::level();
        level.set_block(STONE, blocks::STONE.default_state(), UPDATE_ALL);
        let player = testing::player(&mut level, Vec3::new(2.5, -60.0, 0.5), 90.0, 0.0);
        level.player_mut(player).inventory.slots[36] = Some(ItemStack::new(items::GLOW_LICHEN, 1));
        let hit = BlockHitResult { location: Vec3::new(1.0, -57.5, 0.5), direction: Direction::East, block_pos: STONE, inside: false, world_border_hit: false };
        level.use_item_on(player, InteractionHand::MainHand, hit);
        assert_eq!(level.block_state(STONE.east()), blocks::GLOW_LICHEN.default_state().set(WEST, true));
        level.set_block(STONE, blocks::AIR.default_state(), UPDATE_ALL);
        assert!(level.block_state(STONE.east()).is_air());
    }
}
