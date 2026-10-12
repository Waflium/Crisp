use crisp_core::{BlockPos, Direction};
use crisp_data::properties::{HORIZONTAL_FACING, WATERLOGGED};
use crisp_data::{Block, BlockState, Fluid};

use super::mirror::Mirror;
use super::rotation::Rotation;
use super::simple_waterlogged_block::schedule_water_tick;
use super::{BlockBehaviour, BlockHitResult};
use crate::server::level::ServerLevel;
use crate::world::InteractionResult;
use crate::world::entity::EntityId;
use crate::world::inventory::menu_provider::MenuProvider;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::block::entity::BlockEntityKind;
use crate::world::level::{BlockGetter, LevelAccessor};

pub struct EnderChestBlock;

impl BlockBehaviour for EnderChestBlock {
    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        let replaced_fluid_state = level.fluid_state(ctx.clicked_pos());
        Some(block.default_state().set(HORIZONTAL_FACING, ctx.horizontal_direction().opposite()).set(WATERLOGGED, replaced_fluid_state.fluid == Fluid::Water))
    }

    fn use_without_item(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos, player: EntityId, _hit: &BlockHitResult) -> InteractionResult {
        let Some(entity) = level.get_block_entity(pos) else { return InteractionResult::SUCCESS };
        let BlockEntityKind::EnderChest(_) = entity.kind else { return InteractionResult::SUCCESS };
        let id = entity.id();
        let above = pos.above();
        if level.block_state(above).is_redstone_conductor() {
            return InteractionResult::SUCCESS;
        }
        level.player_mut(player).ender_chest_inventory.set_active_chest(Some((pos, id)));
        level.open_menu(player, MenuProvider::EnderChest);
        level.award_stat(player, crate::stats::stats::OPEN_ENDERCHEST, 1);
        InteractionResult::SUCCESS
    }

    fn rotate(&self, state: BlockState, rotation: Rotation) -> BlockState {
        state.set(HORIZONTAL_FACING, rotation.rotate(state.get(HORIZONTAL_FACING)))
    }

    fn mirror(&self, state: BlockState, mirror: Mirror) -> BlockState {
        super::rotate(state, mirror.get_rotation(state.get(HORIZONTAL_FACING)))
    }

    fn update_shape(&self, state: BlockState, level: &mut dyn LevelAccessor, pos: BlockPos, _: Direction, _: BlockPos, _: BlockState) -> BlockState {
        schedule_water_tick(state, level, pos);
        state
    }

    fn tick(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        level.ender_chest_recheck_open(pos);
    }

    fn trigger_event(&self, _state: BlockState, _level: &mut ServerLevel, _pos: BlockPos, b0: i32, _b1: i32) -> bool {
        b0 == 1
    }
}
