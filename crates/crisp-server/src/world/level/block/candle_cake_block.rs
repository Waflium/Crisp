use std::sync::LazyLock;

use crisp_core::{BlockPos, Direction};
use crisp_data::properties::LIT;
use crisp_data::{Block, BlockState, blocks, items};

use super::abstract_candle_block::extinguish;
use super::cake_block;
use super::vegetation_block::{block_field, fields};
use super::{BlockBehaviour, BlockHitResult};
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;
use crate::world::level::{LevelAccessor, LevelReader};
use crate::world::{InteractionHand, InteractionResult};

static BY_CANDLE: LazyLock<Vec<(Block, Block)>> = LazyLock::new(|| {
    (0..Block::count())
        .map(|id| Block(id as u16))
        .filter(|block| block.is_instance_of("CandleCakeBlock"))
        .map(|candle_cake| (block_field(&fields(candle_cake), "candleBlock"), candle_cake))
        .collect()
});

pub fn by_candle(candle_block: Block) -> BlockState {
    BY_CANDLE.iter().find(|(candle, _)| *candle == candle_block).map(|(_, cake)| cake.default_state()).expect("every candle has a candle cake")
}

pub fn can_light(state: BlockState) -> bool {
    state.is(&crisp_data::block_tags::CANDLE_CAKES) && state.has(LIT) && !state.get(LIT)
}

fn candle_hit(hit: &BlockHitResult) -> bool {
    hit.location.y - hit.block_pos.y as f64 > 0.5
}

pub struct CandleCakeBlock;

impl BlockBehaviour for CandleCakeBlock {
    fn on_projectile_hit(&self, level: &mut ServerLevel, state: BlockState, hit_result: &super::BlockHitResult, projectile: EntityId) {
        if level.is_on_fire(projectile) && !state.get(LIT) {
            super::abstract_candle_block::set_lit(level, state, hit_result.block_pos, true);
        }
    }

    fn use_item_on(
        &self,
        item: ItemStack,
        state: BlockState,
        level: &mut ServerLevel,
        pos: BlockPos,
        _player: EntityId,
        _hand: InteractionHand,
        hit: &BlockHitResult,
    ) -> InteractionResult {
        if item.item == items::FLINT_AND_STEEL || item.item == items::FIRE_CHARGE {
            InteractionResult::Pass
        } else if candle_hit(hit) && item.is_empty() && state.get(LIT) {
            extinguish(state, level, pos);
            InteractionResult::SUCCESS
        } else {
            InteractionResult::TryWithEmptyHand
        }
    }

    fn use_without_item(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, player: EntityId, _hit: &BlockHitResult) -> InteractionResult {
        let eat_result = cake_block::eat(level, pos, blocks::CAKE.default_state(), player);
        if eat_result.consumes_action() {
            level.drop_resources(state, pos, None, None);
        }
        eat_result
    }

    fn get_clone_item_stack(&self, _level: &ServerLevel, _pos: BlockPos, _state: BlockState, _include_data: bool) -> ItemStack {
        ItemStack::new(blocks::CAKE.item(), 1)
    }

    fn update_shape(&self, state: BlockState, level: &mut dyn LevelAccessor, pos: BlockPos, direction: Direction, _: BlockPos, _: BlockState) -> BlockState {
        if direction == Direction::Down && !self.can_survive(state, level, pos) { blocks::AIR.default_state() } else { state }
    }

    fn can_survive(&self, _state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> bool {
        level.block_state(pos.below()).is_solid()
    }

    fn get_analog_output_signal(&self, _state: BlockState, _level: &mut ServerLevel, _pos: BlockPos, _direction: Direction) -> i32 {
        cake_block::get_output_signal(0)
    }
}
