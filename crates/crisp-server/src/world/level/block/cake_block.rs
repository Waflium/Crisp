use crisp_core::{BlockPos, Direction};
use crisp_data::properties::BITES;
use crisp_data::{BlockState, blocks, item_tags, registry, registry_id};

use super::candle_cake_block;
use super::{BlockBehaviour, BlockHitResult};
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;
use crate::world::level::{LevelAccessor, LevelReader};
use crate::world::level::update_flags::UPDATE_ALL;
use crate::world::{InteractionHand, InteractionResult};

pub const MAX_BITES: i32 = 6;

pub struct CakeBlock;

pub fn get_output_signal(bites_taken: i32) -> i32 {
    (7 - bites_taken) * 2
}

pub fn eat(level: &mut ServerLevel, pos: BlockPos, state: BlockState, player: EntityId) -> InteractionResult {
    if !level.can_eat(player, false) {
        return InteractionResult::Pass;
    }
    level.player_mut(player).food_data.eat_with_modifier(2, 0.1);
    let bites = state.get(BITES);
    if bites < MAX_BITES {
        level.set_block(pos, state.set(BITES, bites + 1), UPDATE_ALL);
    } else {
        level.remove_block(pos, false);
    }
    InteractionResult::SUCCESS
}

impl BlockBehaviour for CakeBlock {
    fn use_item_on(
        &self,
        item: ItemStack,
        state: BlockState,
        level: &mut ServerLevel,
        pos: BlockPos,
        player: EntityId,
        hand: InteractionHand,
        _hit: &BlockHitResult,
    ) -> InteractionResult {
        if item.item.is(&item_tags::CANDLES)
            && state.get(BITES) == 0
            && let Some(candle_block) = item.item.info().block
            && candle_block.is_instance_of("CandleBlock")
        {
            let mut item = item;
            item.consume(1, level, Some(player));
            level.player_mut(player).update_item_in_hand(hand, &item);
            level.play_sound(None, pos, registry_id(registry::SOUND_EVENT, "minecraft:block.cake.add_candle"), SoundSource::Blocks, 1.0, 1.0);
            level.set_block(pos, candle_cake_block::by_candle(candle_block), UPDATE_ALL);
            InteractionResult::SUCCESS
        } else {
            InteractionResult::TryWithEmptyHand
        }
    }

    fn use_without_item(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, player: EntityId, _hit: &BlockHitResult) -> InteractionResult {
        eat(level, pos, state, player)
    }

    fn update_shape(&self, state: BlockState, level: &mut dyn LevelAccessor, pos: BlockPos, direction: Direction, _: BlockPos, _: BlockState) -> BlockState {
        if direction == Direction::Down && !self.can_survive(state, level, pos) { blocks::AIR.default_state() } else { state }
    }

    fn can_survive(&self, _state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> bool {
        level.block_state(pos.below()).is_solid()
    }

    fn get_analog_output_signal(&self, state: BlockState, _level: &mut ServerLevel, _pos: BlockPos, _direction: Direction) -> i32 {
        get_output_signal(state.get(BITES))
    }
}
