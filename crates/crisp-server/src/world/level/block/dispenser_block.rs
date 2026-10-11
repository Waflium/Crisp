use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, Direction, Vec3};
use crisp_data::properties::{FACING, TRIGGERED};
use crisp_data::{Block, BlockState, block_entity_types};

use super::mirror::Mirror;
use super::rotation::Rotation;
use super::{BlockBehaviour, BlockHitResult};
use crate::server::level::ServerLevel;
use crate::world::InteractionResult;
use crate::world::container::ContainerRef;
use crate::world::entity::{EntityId, ItemEntity, LevelEntity};
use crate::world::inventory::menu_provider::MenuProvider;
use crate::world::item::ItemStack;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::block::entity::BlockEntityKind;
use crate::world::level::update_flags::UPDATE_CLIENTS;

const TRIGGER_DURATION: i32 = 4;
const SOUND_DISPENSER_DISPENSE: i32 = 1000;
const SOUND_DISPENSER_FAIL: i32 = 1001;
const PARTICLES_SHOOT_SMOKE: i32 = 2000;

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
        let container = ContainerRef::block(level, pos);
        let slot = slot as usize;
        let stack = level.get_item(container, slot);
        let remaining = if self.dropper {
            if stack.is_empty() {
                return;
            }
            let direction = crate::world::level::BlockGetter::block_state(level, pos).get(FACING);
            match level.get_container_at(pos.relative(direction)) {
                None => dispense_default(level, state, pos, stack),
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
            dispense_default(level, state, pos, stack)
        };
        level.set_item(container, slot, remaining);
    }
}

fn dispense_position(pos: BlockPos, direction: Direction) -> Vec3 {
    Vec3::at_center_of(pos).add(0.7 * direction.step_x() as f64, 0.7 * direction.step_y() as f64, 0.7 * direction.step_z() as f64)
}

fn dispense_default(level: &mut ServerLevel, state: BlockState, pos: BlockPos, mut dispensed: ItemStack) -> ItemStack {
    let direction = state.get(FACING);
    let position = dispense_position(pos, direction);
    let one = dispensed.split(1);
    spawn_item(level, one, 6, direction, position);
    level.level_event(None, SOUND_DISPENSER_DISPENSE, pos, 0);
    level.level_event(None, PARTICLES_SHOOT_SMOKE, pos, direction.ordinal() as i32);
    dispensed
}

pub fn spawn_item(level: &mut ServerLevel, stack: ItemStack, accuracy: i32, direction: Direction, position: Vec3) {
    let y = if direction.axis() == crisp_core::Axis::Y { position.y - 0.125 } else { position.y - 0.15625 };
    let random = &mut level.random;
    random.next_double();
    random.next_double();
    let pow = random.next_double() * 0.1 + 0.2;
    let spread = 0.0172275 * accuracy as f64;
    let delta = Vec3::new(
        random.triangle(direction.step_x() as f64 * pow, spread),
        random.triangle(0.2, spread),
        random.triangle(direction.step_z() as f64 * pow, spread),
    );
    let item = ItemEntity::with_movement(Vec3::new(position.x, y, position.z), stack, delta);
    level.add_fresh_entity(LevelEntity::Item(item));
}

impl BlockBehaviour for DispenserBlock {
    fn rotate(&self, state: BlockState, rotation: Rotation) -> BlockState {
        state.set(FACING, rotation.rotate(state.get(FACING)))
    }

    fn mirror(&self, state: BlockState, mirror: Mirror) -> BlockState {
        super::rotate(state, mirror.get_rotation(state.get(FACING)))
    }

    fn use_without_item(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos, player: EntityId, _hit: &BlockHitResult) -> InteractionResult {
        if matches!(level.get_block_entity(pos).map(|e| &e.kind), Some(BlockEntityKind::Dispenser(_))) {
            level.open_menu(player, MenuProvider::BlockEntity(pos));
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
