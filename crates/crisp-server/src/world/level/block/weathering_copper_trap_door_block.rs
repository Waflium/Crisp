use crisp_core::{BlockPos, Direction};
use crisp_data::{Block, BlockState};

use super::mirror::Mirror;
use super::rotation::Rotation;
use super::trap_door_block::TrapDoorBlock;
use super::weathering_copper::change_over_time;
use super::{BlockBehaviour, BlockHitResult};
use crate::server::level::ServerLevel;
use crate::world::InteractionResult;
use crate::world::entity::EntityId;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::LevelAccessor;

pub struct WeatheringCopperTrapDoorBlock {
    trap_door: TrapDoorBlock,
}

impl WeatheringCopperTrapDoorBlock {
    pub fn new(block: Block) -> WeatheringCopperTrapDoorBlock {
        WeatheringCopperTrapDoorBlock { trap_door: TrapDoorBlock::new(block) }
    }
}

impl BlockBehaviour for WeatheringCopperTrapDoorBlock {
    fn rotate(&self, state: BlockState, rotation: Rotation) -> BlockState {
        self.trap_door.rotate(state, rotation)
    }

    fn mirror(&self, state: BlockState, mirror: Mirror) -> BlockState {
        self.trap_door.mirror(state, mirror)
    }

    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        self.trap_door.get_state_for_placement(block, ctx, level)
    }

    fn use_without_item(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, player: EntityId, hit: &BlockHitResult) -> InteractionResult {
        self.trap_door.use_without_item(state, level, pos, player, hit)
    }

    fn neighbor_changed(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, block: Block, moved_by_piston: bool) {
        self.trap_door.neighbor_changed(state, level, pos, block, moved_by_piston);
    }

    fn update_shape(
        &self,
        state: BlockState,
        level: &mut dyn LevelAccessor,
        pos: BlockPos,
        direction: Direction,
        neighbour_pos: BlockPos,
        neighbour_state: BlockState,
    ) -> BlockState {
        self.trap_door.update_shape(state, level, pos, direction, neighbour_pos, neighbour_state)
    }

    fn random_tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        change_over_time(state, level, pos);
    }
}
