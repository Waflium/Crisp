use std::sync::LazyLock;

use crisp_core::BlockPos;
use crisp_data::{BlockState, block_tags};

use super::{BlockBehaviour, BlockHitResult};
use crate::server::level::ServerLevel;
use crate::world::InteractionResult;
use crate::world::entity::EntityId;
use crate::world::inventory::menu_provider::MenuProvider;
use crate::world::level::BlockGetter;
use crate::world::level::block::entity::BlockEntityKind;

pub static BOOKSHELF_OFFSETS: LazyLock<Vec<BlockPos>> = LazyLock::new(|| {
    BlockPos::between_closed(BlockPos::new(-2, 0, -2), BlockPos::new(2, 1, 2)).filter(|pos| pos.x.abs() == 2 || pos.z.abs() == 2).collect()
});

pub fn is_valid_book_shelf(level: &mut ServerLevel, pos: BlockPos, offset: BlockPos) -> bool {
    level.block_state(pos.offset(offset.x, offset.y, offset.z)).is(&block_tags::ENCHANTMENT_POWER_PROVIDER)
        && level.block_state(pos.offset(offset.x / 2, offset.y, offset.z / 2)).is(&block_tags::ENCHANTMENT_POWER_TRANSMITTER)
}

pub struct EnchantingTableBlock;

impl BlockBehaviour for EnchantingTableBlock {
    fn use_without_item(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos, player: EntityId, _hit: &BlockHitResult) -> InteractionResult {
        if let Some(BlockEntityKind::EnchantingTable(_)) = level.get_block_entity(pos).map(|entity| &entity.kind) {
            level.open_menu(player, MenuProvider::BlockEntity(pos));
        }
        InteractionResult::SUCCESS
    }
}
