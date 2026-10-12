use crisp_core::{BlockPos, Direction};
use crisp_data::properties::{FACING, TRIGGERED};
use crisp_data::{Block, BlockState, block_entity_types};

use super::mirror::Mirror;
use super::rotation::Rotation;
use super::{BlockBehaviour, BlockHitResult};
use crate::core::dispenser::block_source::BlockSource;
use crate::core::dispenser::dispense_item_behavior::{DispenseItemBehavior, get_dispense_method};
use crate::server::level::ServerLevel;
use crate::server::level::events::SOUND_DISPENSER_FAIL;
use crate::world::InteractionResult;
use crate::world::container::ContainerRef;
use crate::world::entity::EntityId;
use crate::world::inventory::menu_provider::MenuProvider;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::block::entity::BlockEntityKind;
use crate::world::level::update_flags::UPDATE_CLIENTS;

const TRIGGER_DURATION: i32 = 4;

pub struct DispenserBlock {
    block: Block,
    dropper: bool,
}

impl DispenserBlock {
    pub fn new(block: Block) -> DispenserBlock {
        DispenserBlock { block, dropper: block.is_instance_of("DropperBlock") }
    }

    fn dispense_from(&self, level: &mut ServerLevel, state: BlockState, pos: BlockPos) {
        let expected = if self.dropper { block_entity_types::DROPPER } else { block_entity_types::DISPENSER };
        if !level.get_block_entity(pos).is_some_and(|e| e.ty() == expected) {
            warn!("Ignoring dispensing attempt for {} without matching block entity at {pos:?}", if self.dropper { "Dropper" } else { "Dispenser" });
            return;
        }
        level.unpack_loot_table(pos, None);
        let slot = {
            let ServerLevel { chunks, random, .. } = level;
            match chunks.block_entity_ref(pos).map(|e| &e.kind) {
                Some(BlockEntityKind::Dispenser(dispenser)) => dispenser.get_random_slot(random),
                _ => -1,
            }
        };
        if slot < 0 {
            level.level_event(None, SOUND_DISPENSER_FAIL, pos, 0);
            return;
        }
        let source = BlockSource { pos, state };
        let container = ContainerRef::block(level, pos);
        let slot = slot as usize;
        let stack = level.get_item(container, slot);
        let remaining = if self.dropper {
            if stack.is_empty() {
                return;
            }
            let direction = crate::world::level::BlockGetter::block_state(level, pos).get(FACING);
            match level.get_container_at(pos.relative(direction)) {
                None => DispenseItemBehavior::Default.dispense(level, &source, stack),
                Some(into) => {
                    let rest = level.add_item(Some(container), into, stack.copy_with_count(1), Some(direction.opposite()));
                    let mut remaining = stack.copy();
                    if rest.is_empty() {
                        remaining.shrink(1);
                    }
                    remaining
                }
            }
        } else {
            get_dispense_method(&stack).dispense(level, &source, stack)
        };
        level.set_item(container, slot, remaining);
    }
}

impl BlockBehaviour for DispenserBlock {
    fn rotate(&self, state: BlockState, rotation: Rotation) -> BlockState {
        state.set(FACING, rotation.rotate(state.get(FACING)))
    }

    fn mirror(&self, state: BlockState, mirror: Mirror) -> BlockState {
        super::rotate(state, mirror.get_rotation(state.get(FACING)))
    }

    fn use_without_item(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos, player: EntityId, _hit: &BlockHitResult) -> InteractionResult {
        if let Some(entity) = level.get_block_entity(pos).filter(|e| matches!(e.kind, BlockEntityKind::Dispenser(_))) {
            let stat = if entity.ty() == crisp_data::block_entity_types::DROPPER { crate::stats::stats::INSPECT_DROPPER } else { crate::stats::stats::INSPECT_DISPENSER };
            level.open_menu(player, MenuProvider::BlockEntity(pos));
            level.award_stat(player, stat, 1);
        }
        InteractionResult::SUCCESS
    }

    fn neighbor_changed(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, _block: Block, _moved_by_piston: bool) {
        let should_trigger = level.has_neighbor_signal(pos) || level.has_neighbor_signal(pos.above());
        let is_triggered = state.get(TRIGGERED);
        if should_trigger && !is_triggered {
            level.schedule_tick(pos, self.block, TRIGGER_DURATION);
            level.set_block(pos, state.set(TRIGGERED, true), UPDATE_CLIENTS);
        } else if !should_trigger && is_triggered {
            level.set_block(pos, state.set(TRIGGERED, false), UPDATE_CLIENTS);
        }
    }

    fn tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        self.dispense_from(level, state, pos);
    }

    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, _level: &mut ServerLevel) -> Option<BlockState> {
        Some(block.default_state().set(FACING, ctx.nearest_looking_direction().opposite()))
    }

    fn affect_neighbors_after_removal(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, _moved_by_piston: bool) {
        crate::world::containers::update_neighbours_after_destroy(level, state, pos);
    }

    fn get_analog_output_signal(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos, _direction: Direction) -> i32 {
        level.get_redstone_signal_from_block_entity(pos)
    }
}
