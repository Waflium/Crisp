use std::collections::HashMap;
use std::sync::LazyLock;

use crisp_core::{BlockPos, Direction};
use crisp_data::{Block, BlockState, blocks};

use super::vegetation_block::{block_field, fields};
use super::{BlockBehaviour, BlockHitResult};
use crate::server::level::ServerLevel;
use crate::server::level::game_mode::GameType;
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;
use crate::world::level::LevelAccessor;
use crate::world::level::update_flags::UPDATE_ALL;
use crate::world::{InteractionHand, InteractionResult};

static POTTED_BY_CONTENT: LazyLock<HashMap<Block, Block>> = LazyLock::new(|| {
    (0..Block::count())
        .map(|id| Block(id as u16))
        .filter(|block| block.is_instance_of("FlowerPotBlock"))
        .map(|pot| (block_field(&fields(pot), "potted"), pot))
        .collect()
});

pub struct FlowerPotBlock {
    potted: Block,
}

impl FlowerPotBlock {
    pub fn new(block: Block) -> FlowerPotBlock {
        FlowerPotBlock { potted: block_field(&fields(block), "potted") }
    }

    fn is_empty(&self) -> bool {
        self.potted == blocks::AIR
    }
}

fn consume_one(level: &mut ServerLevel, player: EntityId, hand: InteractionHand) {
    let p = level.player_mut(player);
    if p.game_mode.game_type == GameType::Creative {
        return;
    }
    let slot = p.hand_slot(hand);
    if let Some(held) = &mut p.inventory.slots[slot] {
        held.count -= 1;
        if held.count <= 0 {
            p.inventory.slots[slot] = None;
        }
    }
    p.dirty_slots.push(slot);
}

impl BlockBehaviour for FlowerPotBlock {
    fn get_clone_item_stack(&self, _level: &ServerLevel, _pos: BlockPos, state: BlockState, _include_data: bool) -> ItemStack {
        ItemStack::new(if self.potted == blocks::AIR { state.block() } else { self.potted }.item(), 1)
    }

    fn use_item_on(
        &self,
        item: ItemStack,
        _state: BlockState,
        level: &mut ServerLevel,
        pos: BlockPos,
        player: EntityId,
        hand: InteractionHand,
        _hit: &BlockHitResult,
    ) -> InteractionResult {
        let info = item.item.info();
        let content = match info.block {
            Some(block) if !item.is_empty() && info.classes.contains(&"BlockItem") => POTTED_BY_CONTENT.get(&block).copied().unwrap_or(blocks::AIR),
            _ => blocks::AIR,
        };
        let new_contents = content.default_state();
        if new_contents.is_air() {
            return InteractionResult::TryWithEmptyHand;
        }
        if !self.is_empty() {
            return InteractionResult::CONSUME;
        }
        level.set_block(pos, new_contents, UPDATE_ALL);
        consume_one(level, player, hand);
        InteractionResult::SUCCESS
    }

    fn use_without_item(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos, player: EntityId, _hit: &BlockHitResult) -> InteractionResult {
        if self.is_empty() {
            return InteractionResult::CONSUME;
        }
        let plant = ItemStack::new(self.potted.item(), 1);
        let p = level.player_mut(player);
        let left = p.pick_up(plant.clone());
        if left > 0 && p.game_mode.game_type != GameType::Creative {
            level.player_drop(player, plant.copy_with_count(left), false, crate::util::Prediction::Predicted);
        }
        level.set_block(pos, blocks::FLOWER_POT.default_state(), UPDATE_ALL);
        InteractionResult::SUCCESS
    }

    fn update_shape(&self, state: BlockState, level: &mut dyn LevelAccessor, pos: BlockPos, direction: Direction, _: BlockPos, _: BlockState) -> BlockState {
        if direction == Direction::Down && !self.can_survive(state, level, pos) { blocks::AIR.default_state() } else { state }
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
    fn plants_go_into_and_come_out_of_pots() {
        let mut level = testing::level();
        let pot = BlockPos::new(0, -60, 0);
        level.set_block(pot, blocks::FLOWER_POT.default_state(), UPDATE_ALL);
        let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 3.5), 180.0, 30.0);
        level.player_mut(player).inventory.slots[36] = Some(ItemStack::new(items::POPPY, 1));
        let hit = BlockHitResult { location: Vec3::new(0.5, -59.7, 0.5), direction: Direction::Up, block_pos: pot, inside: false, world_border_hit: false };

        assert!(level.use_item_on(player, InteractionHand::MainHand, hit).consumes_action());
        assert_eq!(level.block_state(pot), blocks::POTTED_POPPY.default_state());
        assert_eq!(level.player(player).inventory.slots[36], None);

        assert!(level.use_item_on(player, InteractionHand::MainHand, hit).consumes_action());
        assert_eq!(level.block_state(pot), blocks::FLOWER_POT.default_state());
        assert_eq!(level.player(player).inventory.slots[36], Some(ItemStack::new(items::POPPY, 1)));
    }
}
