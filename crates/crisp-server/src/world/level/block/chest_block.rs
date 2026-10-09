use std::sync::LazyLock;

use crisp_core::{BlockPos, Direction};
use crisp_data::properties::{CHEST_TYPE, ChestType, HORIZONTAL_FACING, WATERLOGGED};
use crisp_data::{Block, BlockState, block_tags, registry, registry_id};

use super::simple_waterlogged_block::{is_water_source, schedule_water_tick};
use super::weathering_copper;
use super::{BlockBehaviour, BlockHitResult};
use crate::server::level::ServerLevel;
use crate::world::InteractionResult;
use crate::world::container::ContainerRef;
use crate::world::entity::EntityId;
use crate::world::inventory::menu_provider::MenuProvider;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::BlockGetter;
use crate::world::level::block::entity::BlockEntityKind;

static SOUNDS: LazyLock<Vec<(i32, i32)>> = LazyLock::new(|| {
    (0..Block::count())
        .map(|id| {
            let fields: serde_json::Value = serde_json::from_str(Block(id as u16).info().fields).unwrap_or_default();
            let sound = |key: &str| fields[key].as_str().map_or(-1, |s| registry_id(registry::SOUND_EVENT, s));
            (sound("openSound"), sound("closeSound"))
        })
        .collect()
});

pub fn open_sound(block: Block) -> i32 {
    SOUNDS[block.0 as usize].0
}

pub fn close_sound(block: Block) -> i32 {
    SOUNDS[block.0 as usize].1
}

pub fn get_connected_direction(state: BlockState) -> Direction {
    let facing = state.get(HORIZONTAL_FACING);
    if state.get(CHEST_TYPE) == ChestType::Left { facing.clockwise() } else { facing.counter_clockwise() }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum BlockType {
    Single,
    First,
    Second,
}

fn block_type(state: BlockState) -> BlockType {
    match state.get(CHEST_TYPE) {
        ChestType::Single => BlockType::Single,
        ChestType::Right => BlockType::First,
        ChestType::Left => BlockType::Second,
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Combined {
    None,
    Single(BlockPos),
    Double(BlockPos, BlockPos),
}

fn is_chest_blocked_at(level: &mut ServerLevel, pos: BlockPos) -> bool {
    level.block_state(pos.above()).is_redstone_conductor()
}

fn has_chest_block_entity(level: &mut ServerLevel, pos: BlockPos, state: BlockState) -> bool {
    let ty = state.block().block_entity_type();
    level.get_block_entity(pos).is_some_and(|e| Some(e.ty()) == ty && matches!(e.kind, BlockEntityKind::Chest(_)))
}

pub fn combine(state: BlockState, level: &mut ServerLevel, pos: BlockPos, ignore_being_blocked: bool) -> Combined {
    if !has_chest_block_entity(level, pos, state) {
        return Combined::None;
    }
    if !ignore_being_blocked && is_chest_blocked_at(level, pos) {
        return Combined::None;
    }
    let ty = block_type(state);
    if ty == BlockType::Single {
        return Combined::Single(pos);
    }
    let neighbour_pos = pos.relative(get_connected_direction(state));
    let neighbour = level.block_state(neighbour_pos);
    if neighbour.is_block(state.block()) {
        let neighbour_type = block_type(neighbour);
        if neighbour_type != BlockType::Single && ty != neighbour_type && neighbour.get(HORIZONTAL_FACING) == state.get(HORIZONTAL_FACING) {
            if !ignore_being_blocked && is_chest_blocked_at(level, neighbour_pos) {
                return Combined::None;
            }
            if has_chest_block_entity(level, neighbour_pos, neighbour) {
                return if ty == BlockType::First { Combined::Double(pos, neighbour_pos) } else { Combined::Double(neighbour_pos, pos) };
            }
        }
    }
    Combined::Single(pos)
}

pub fn get_container(state: BlockState, level: &mut ServerLevel, pos: BlockPos, ignore_being_blocked: bool) -> Option<ContainerRef> {
    match combine(state, level, pos, ignore_being_blocked) {
        Combined::None => None,
        Combined::Single(pos) => Some(ContainerRef::block(level, pos)),
        Combined::Double(first, second) => {
            let (ContainerRef::Block { pos: p1, id: i1 }, ContainerRef::Block { pos: p2, id: i2 }) = (ContainerRef::block(level, first), ContainerRef::block(level, second)) else {
                unreachable!()
            };
            Some(ContainerRef::Compound { first: (p1, i1), second: (p2, i2) })
        }
    }
}

fn open_count(level: &mut ServerLevel, pos: BlockPos) -> i32 {
    if !level.block_state(pos).has_block_entity() {
        return 0;
    }
    match level.get_block_entity(pos).map(|e| &e.kind) {
        Some(BlockEntityKind::Chest(chest)) => chest.openers_counter.open_count,
        _ => 0,
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Copper {
    No,
    Waxed,
    Weathering,
}

pub struct ChestBlock {
    block: Block,
    trapped: bool,
    copper: Copper,
}

impl ChestBlock {
    pub fn new(block: Block) -> ChestBlock {
        let copper = if block.is_instance_of("WeatheringCopperChestBlock") {
            Copper::Weathering
        } else if block.is_instance_of("CopperChestBlock") {
            Copper::Waxed
        } else {
            Copper::No
        };
        ChestBlock { block, trapped: block.is_instance_of("TrappedChestBlock"), copper }
    }

    fn chest_can_connect_to(&self, state: BlockState) -> bool {
        match self.copper {
            Copper::No => state.is_block(self.block),
            _ => state.is(&block_tags::COPPER_CHESTS) && state.has(CHEST_TYPE),
        }
    }

    fn candidate_partner_facing(&self, level: &mut ServerLevel, pos: BlockPos, neighbour_direction: Direction) -> Option<Direction> {
        let state = level.block_state(pos.relative(neighbour_direction));
        (self.chest_can_connect_to(state) && state.get(CHEST_TYPE) == ChestType::Single).then(|| state.get(HORIZONTAL_FACING))
    }

    fn get_chest_type(&self, level: &mut ServerLevel, pos: BlockPos, facing: Direction) -> ChestType {
        if Some(facing) == self.candidate_partner_facing(level, pos, facing.clockwise()) {
            ChestType::Left
        } else if Some(facing) == self.candidate_partner_facing(level, pos, facing.counter_clockwise()) {
            ChestType::Right
        } else {
            ChestType::Single
        }
    }

    fn chest_update_shape(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, direction: Direction, neighbour: BlockState) -> BlockState {
        schedule_water_tick(state, level, pos);
        if self.chest_can_connect_to(neighbour) && direction.axis().is_horizontal() {
            let neighbour_type = neighbour.get(CHEST_TYPE);
            if state.get(CHEST_TYPE) == ChestType::Single
                && neighbour_type != ChestType::Single
                && state.get(HORIZONTAL_FACING) == neighbour.get(HORIZONTAL_FACING)
                && get_connected_direction(neighbour) == direction.opposite()
            {
                return state.set(CHEST_TYPE, opposite(neighbour_type));
            }
        } else if get_connected_direction(state) == direction {
            return state.set(CHEST_TYPE, ChestType::Single);
        }
        state
    }
}

fn opposite(ty: ChestType) -> ChestType {
    match ty {
        ChestType::Single => ChestType::Single,
        ChestType::Left => ChestType::Right,
        ChestType::Right => ChestType::Left,
    }
}

fn unwax(block: Block, state: BlockState) -> BlockState {
    match crisp_data::WAXABLES.iter().find(|&&(_, waxed)| waxed == block) {
        Some(&(unwaxed, _)) => weathering_copper::with_properties_of(unwaxed, state),
        None => state,
    }
}

fn least_oxidized_chest_of_connected_blocks(state: BlockState, level: &mut ServerLevel, pos: BlockPos) -> BlockState {
    let connected = level.block_state(pos.relative(get_connected_direction(state)));
    let is_copper = |b: Block| b.is_instance_of("CopperChestBlock");
    if state.get(CHEST_TYPE) == ChestType::Single || !is_copper(state.block()) || !is_copper(connected.block()) {
        return state;
    }
    let waxed = |b: Block| !b.is_instance_of("WeatheringCopperChestBlock");
    let (mut updated, mut connected_predicted) = (state, connected);
    if waxed(state.block()) != waxed(connected.block()) {
        if waxed(state.block()) {
            updated = unwax(state.block(), state);
        }
        if waxed(connected.block()) {
            connected_predicted = unwax(connected.block(), connected);
        }
    }
    let age = |b: Block| weathering_copper::age(b);
    let least = if age(state.block()) <= age(connected.block()) { updated.block() } else { connected_predicted.block() };
    weathering_copper::with_properties_of(least, updated)
}

impl BlockBehaviour for ChestBlock {
    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        let mut ty = ChestType::Single;
        let mut facing = ctx.horizontal_direction().opposite();
        let pos = ctx.clicked_pos();
        let water = is_water_source(level, pos);
        let secondary_use = ctx.is_secondary_use_active();
        let clicked_face = ctx.clicked_face();
        if clicked_face.axis().is_horizontal() && secondary_use {
            if let Some(neighbour_facing) = self.candidate_partner_facing(level, pos, clicked_face.opposite())
                && neighbour_facing.axis() != clicked_face.axis()
            {
                facing = neighbour_facing;
                ty = if facing.counter_clockwise() == clicked_face.opposite() { ChestType::Right } else { ChestType::Left };
            }
        }
        if ty == ChestType::Single && !secondary_use {
            ty = self.get_chest_type(level, pos, facing);
        }
        let state = block.default_state().set(HORIZONTAL_FACING, facing).set(CHEST_TYPE, ty).set(WATERLOGGED, water);
        Some(if self.copper == Copper::No { state } else { least_oxidized_chest_of_connected_blocks(state, level, pos) })
    }

    fn update_shape(
        &self,
        state: BlockState,
        level: &mut ServerLevel,
        pos: BlockPos,
        direction: Direction,
        _neighbour_pos: BlockPos,
        neighbour: BlockState,
    ) -> BlockState {
        let state = self.chest_update_shape(state, level, pos, direction, neighbour);
        if self.copper != Copper::No
            && self.chest_can_connect_to(neighbour)
            && state.get(CHEST_TYPE) != ChestType::Single
            && get_connected_direction(state) == direction
        {
            return weathering_copper::with_properties_of(neighbour.block(), state);
        }
        state
    }

    fn affect_neighbors_after_removal(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, _moved_by_piston: bool) {
        crate::world::containers::update_neighbours_after_destroy(level, state, pos);
    }

    fn use_without_item(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, player: EntityId, _hit: &BlockHitResult) -> InteractionResult {
        let provider = match combine(state, level, pos, false) {
            Combined::None => None,
            Combined::Single(pos) => Some(MenuProvider::BlockEntity(pos)),
            Combined::Double(first, second) => Some(MenuProvider::DoubleChest { first, second }),
        };
        if let Some(provider) = provider {
            level.open_menu(player, provider);
        }
        InteractionResult::SUCCESS
    }

    fn tick(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        if matches!(level.get_block_entity(pos).map(|e| &e.kind), Some(BlockEntityKind::Chest(_))) {
            level.recheck_openers(pos);
        }
    }

    fn random_tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        if self.copper == Copper::Weathering
            && state.get(CHEST_TYPE) != ChestType::Right
            && matches!(level.get_block_entity(pos).map(|e| &e.kind), Some(BlockEntityKind::Chest(_)))
            && level.get_entities_with_container_open(pos).is_empty()
        {
            weathering_copper::change_over_time(state, level, pos);
        }
    }

    fn should_changed_state_keep_block_entity(&self, old_state: BlockState) -> bool {
        self.copper != Copper::No && old_state.is(&block_tags::COPPER_CHESTS)
    }

    fn trigger_event(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos, b0: i32, b1: i32) -> bool {
        level.get_block_entity(pos).is_some_and(|e| e.trigger_event(b0, b1))
    }

    fn get_analog_output_signal(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, _direction: Direction) -> i32 {
        let container = get_container(state, level, pos, false);
        level.get_redstone_signal_from_container(container)
    }

    fn own_signal(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos) -> i32 {
        if self.trapped { open_count(level, pos).clamp(0, 15) } else { 0 }
    }

    fn get_direct_signal(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, direction: Direction) -> i32 {
        if self.trapped && direction == Direction::Up { self.get_signal(state, level, pos, direction) } else { 0 }
    }
}
