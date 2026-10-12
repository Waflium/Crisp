use crisp_core::{BlockPos, Direction};
use crisp_data::properties::{CANDLES, LIT, WATERLOGGED};
use crisp_data::{Block, BlockState, Fluid, FluidState, block_tags, registry, registry_id};

use super::abstract_candle_block::extinguish;
use super::simple_waterlogged_block::{is_water_source, schedule_water_tick};
use super::{BlockBehaviour, BlockHitResult};
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::material::tick_delay;
use crate::world::level::{BlockGetter, LevelAccessor, LevelReader};
use crate::world::level::update_flags::UPDATE_ALL;
use crate::world::{InteractionHand, InteractionResult};

pub struct CandleBlock;

pub fn can_light(state: BlockState) -> bool {
    state.is(&block_tags::CANDLES) && state.has(LIT) && state.has(WATERLOGGED) && !state.get(LIT) && !state.get(WATERLOGGED)
}

impl BlockBehaviour for CandleBlock {
    fn use_item_on(
        &self,
        item: ItemStack,
        state: BlockState,
        level: &mut ServerLevel,
        pos: BlockPos,
        player: EntityId,
        _hand: InteractionHand,
        _hit: &BlockHitResult,
    ) -> InteractionResult {
        if item.is_empty() && level.player(player).abilities.may_build && state.get(LIT) {
            extinguish(state, level, pos);
            InteractionResult::SUCCESS
        } else {
            InteractionResult::TryWithEmptyHand
        }
    }

    fn can_be_replaced(&self, state: BlockState, ctx: &BlockPlaceContext) -> bool {
        if !ctx.is_secondary_use_active() && ctx.item.item == state.block().item() && state.get(CANDLES) < 4 {
            return true;
        }
        state.can_be_replaced() && (ctx.item.is_empty() || ctx.item.item != state.block().item())
    }

    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        let pos = ctx.clicked_pos();
        let state = level.block_state(pos);
        if state.is_block(block) {
            return Some(state.cycle(CANDLES));
        }
        Some(block.default_state().set(WATERLOGGED, is_water_source(level, pos)))
    }

    fn update_shape(&self, state: BlockState, level: &mut dyn LevelAccessor, pos: BlockPos, _: Direction, _: BlockPos, _: BlockState) -> BlockState {
        schedule_water_tick(state, level, pos);
        state
    }

    fn place_liquid(&self, level: &mut dyn LevelAccessor, pos: BlockPos, state: BlockState, fluid: FluidState) -> bool {
        if !state.get(WATERLOGGED) && fluid.fluid == Fluid::Water {
            let new_state = state.set(WATERLOGGED, true);
            if state.get(LIT) {
                level.set_block(pos, new_state.set(LIT, false), 11);
                level.play_sound(pos, registry_id(registry::SOUND_EVENT, "minecraft:block.candle.extinguish"), SoundSource::Blocks, 1.0, 1.0);
            } else {
                level.set_block(pos, new_state, UPDATE_ALL);
            }
            let delay = tick_delay(level, fluid.fluid);
            level.schedule_fluid_tick(pos, fluid.fluid, delay);
            true
        } else {
            false
        }
    }

    fn can_survive(&self, _state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> bool {
        super::can_support_center(level, pos.below(), Direction::Up)
    }
}
