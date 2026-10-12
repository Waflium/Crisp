use crisp_core::BlockPos;
use crisp_data::BlockState;

use crate::server::level::ServerLevel;
use crate::world::item::ItemStack;
use crate::world::level::block::entity::BlockEntityKind;

pub fn get_clone_item_stack(level: &ServerLevel, pos: BlockPos, state: BlockState, _include_data: bool) -> ItemStack {
    match level.chunks.block_entity_ref(pos) {
        Some(entity) if matches!(entity.kind, BlockEntityKind::Banner(_)) => {
            let mut item_stack = ItemStack::new(entity.block_state().block().item(), 1);
            for (ty, value) in entity.collect_components() {
                item_stack.set(ty, value);
            }
            item_stack
        }
        _ => ItemStack::new(state.block().item(), 1),
    }
}
