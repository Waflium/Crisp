use std::sync::Arc;

use crisp_core::{Axis, BlockPos, Direction, Vec3};
use crisp_data::properties::{BED_PART, BedPart, HORIZONTAL_FACING, OCCUPIED};
use crisp_data::{Block, BlockState, blocks, damage_types, registry, registry_id};

use super::rotation::Rotation;
use super::{BlockBehaviour, BlockHitResult};
use crate::network::chat::Component;
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::InteractionResult;
use crate::world::attribute::bed_rule::BedRule;
use crate::world::attribute::environment_attributes;
use crate::world::damagesource::DamageSource;
use crate::world::entity::EntityId;
use crate::world::entity::vehicle::dismount_helper::{self, DismountType};
use crate::world::item::ItemStack;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::server_explosion::ExplosionInteraction;
use crate::world::level::update_flags::UPDATE_ALL;
use crate::world::level::{BlockGetter, LevelAccessor};

pub const FACING: crisp_data::properties::EnumProperty<Direction> = HORIZONTAL_FACING;
pub const PART: crisp_data::properties::EnumProperty<BedPart> = BED_PART;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum BedKind {
    Bed,
    StrawBed,
}

pub struct AbstractBedBlock {
    kind: BedKind,
}

impl AbstractBedBlock {
    pub const fn new(kind: BedKind) -> AbstractBedBlock {
        AbstractBedBlock { kind }
    }
}

pub fn kind(block: Block) -> Option<BedKind> {
    if block.is_instance_of("StrawBedBlock") {
        Some(BedKind::StrawBed)
    } else if block.is_instance_of("AbstractBedBlock") {
        Some(BedKind::Bed)
    } else {
        None
    }
}

pub fn get_slept_in_bed_stat_type(kind: BedKind) -> crate::stats::stat::Stat {
    match kind {
        BedKind::Bed => crate::stats::stats::SLEEP_IN_BED,
        BedKind::StrawBed => crate::stats::stats::SLEEP_IN_STRAW_BED,
    }
}

pub fn get_bed_orientation(level: &mut ServerLevel, pos: BlockPos) -> Option<Direction> {
    let block_state = level.block_state(pos);
    kind(block_state.block()).map(|_| block_state.get(FACING))
}

pub fn get_bed_rule(kind: BedKind, level: &ServerLevel, pos: BlockPos) -> Arc<BedRule> {
    let attribute = match kind {
        BedKind::Bed => environment_attributes::BED_RULE,
        BedKind::StrawBed => environment_attributes::STRAW_BED_RULE,
    };
    level.environment_attributes().get_value_at(attribute, pos)
}

pub fn error_message(rule: &BedRule) -> Option<Component> {
    rule.error_message.as_ref().and_then(|message| crate::nbt::from_json(message)).map(|tag| Component::from_nbt(&tag))
}

pub fn get_sleep_height(state: BlockState, level: &mut ServerLevel, pos: BlockPos) -> Option<f64> {
    let kind = kind(state.block())?;
    let laying_on_state = if kind == BedKind::StrawBed && state.get(PART) == BedPart::Head {
        let laying_on_pos = pos.relative(get_connected_direction(state));
        let laying_on_state = level.block_state(laying_on_pos);
        if !laying_on_state.is_block(state.block()) || laying_on_state.get(PART) != BedPart::Foot {
            return None;
        }
        laying_on_state
    } else {
        state
    };
    let shape = laying_on_state.voxel_shape();
    if shape.is_empty() { None } else { Some(shape.max(Axis::Y)) }
}

fn destroy_straw_bed(level: &mut ServerLevel, pos: BlockPos) {
    level.play_sound(None, pos, registry_id(registry::SOUND_EVENT, "minecraft:block.straw_bed.break_leave"), SoundSource::Blocks, 1.0, 1.0);
    level.set_block(pos, blocks::AIR.default_state(), UPDATE_ALL);
}

fn destroy_on_use(kind: BedKind, state: BlockState, level: &mut ServerLevel, pos: BlockPos) -> InteractionResult {
    match kind {
        BedKind::Bed => {
            level.remove_block(pos, false);
            let block_pos = pos.relative(state.get(FACING).opposite());
            if level.block_state(block_pos).is_block(state.block()) {
                level.remove_block(block_pos, false);
            }
            let boom_pos = Vec3::at_center_of(pos);
            let source = DamageSource { damage_source_position: Some(boom_pos), ..DamageSource::new(damage_types::BAD_RESPAWN_POINT) };
            level.explode(None, None, Some(source), boom_pos, 5.0, true, ExplosionInteraction::Block);
        }
        BedKind::StrawBed => destroy_straw_bed(level, pos),
    }
    InteractionResult::SUCCESS_SERVER
}

pub fn on_stop_sleeping(kind: BedKind, level: &mut ServerLevel, pos: BlockPos) {
    let bed_rule = get_bed_rule(kind, level, pos);
    if bed_rule.destroy_on_leave && kind == BedKind::StrawBed {
        destroy_straw_bed(level, pos);
    }
}

fn get_neighbour_direction(part: BedPart, facing: Direction) -> Direction {
    if part == BedPart::Foot { facing } else { facing.opposite() }
}

pub fn get_connected_direction(state: BlockState) -> Direction {
    let facing = state.get(FACING);
    if state.get(PART) == BedPart::Head { facing.opposite() } else { facing }
}

fn is_bunk_bed(level: &mut ServerLevel, pos: BlockPos) -> bool {
    kind(level.block_state(pos.below()).block()).is_some()
}

pub fn find_stand_up_position(entity_type: DismountType, level: &mut ServerLevel, pos: BlockPos, forward: Direction, yaw: f32) -> Option<Vec3> {
    let right = forward.clockwise();
    let side = if right.is_facing_angle(yaw) { right.opposite() } else { right };
    if is_bunk_bed(level, pos) {
        return find_bunk_bed_stand_up_position(entity_type, level, pos, forward, side);
    }
    let offsets = bed_stand_up_offsets(forward, side);
    find_stand_up_position_at_offset(entity_type, level, pos, &offsets, true).or_else(|| find_stand_up_position_at_offset(entity_type, level, pos, &offsets, false))
}

fn find_bunk_bed_stand_up_position(entity_type: DismountType, level: &mut ServerLevel, pos: BlockPos, forward: Direction, side: Direction) -> Option<Vec3> {
    let offsets = bed_surround_stand_up_offsets(forward, side);
    if let Some(safe_position) = find_stand_up_position_at_offset(entity_type, level, pos, &offsets, true) {
        return Some(safe_position);
    }
    let below = pos.below();
    if let Some(below_safe_position) = find_stand_up_position_at_offset(entity_type, level, below, &offsets, true) {
        return Some(below_safe_position);
    }
    let above_offsets = bed_above_stand_up_offsets(forward);
    if let Some(above_safe_position) = find_stand_up_position_at_offset(entity_type, level, pos, &above_offsets, true) {
        return Some(above_safe_position);
    }
    if let Some(unsafe_position) = find_stand_up_position_at_offset(entity_type, level, pos, &offsets, false) {
        return Some(unsafe_position);
    }
    if let Some(below_unsafe_position) = find_stand_up_position_at_offset(entity_type, level, below, &offsets, false) {
        return Some(below_unsafe_position);
    }
    find_stand_up_position_at_offset(entity_type, level, pos, &above_offsets, false)
}

fn find_stand_up_position_at_offset(entity_type: DismountType, level: &mut ServerLevel, pos: BlockPos, offsets: &[[i32; 2]], check_dangerous: bool) -> Option<Vec3> {
    offsets
        .iter()
        .find_map(|offset| dismount_helper::find_safe_dismount_location(entity_type, level, BlockPos::new(pos.x + offset[0], pos.y, pos.z + offset[1]), check_dangerous))
}

fn bed_stand_up_offsets(forward: Direction, side: Direction) -> Vec<[i32; 2]> {
    let mut offsets = bed_surround_stand_up_offsets(forward, side).to_vec();
    offsets.extend(bed_above_stand_up_offsets(forward));
    offsets
}

fn bed_surround_stand_up_offsets(forward: Direction, side: Direction) -> [[i32; 2]; 10] {
    let (fx, fz, sx, sz) = (forward.step_x(), forward.step_z(), side.step_x(), side.step_z());
    [
        [sx, sz],
        [sx - fx, sz - fz],
        [sx - fx * 2, sz - fz * 2],
        [-fx * 2, -fz * 2],
        [-sx - fx * 2, -sz - fz * 2],
        [-sx - fx, -sz - fz],
        [-sx, -sz],
        [-sx + fx, -sz + fz],
        [fx, fz],
        [sx + fx, sz + fz],
    ]
}

fn bed_above_stand_up_offsets(forward: Direction) -> [[i32; 2]; 2] {
    [[0, 0], [-forward.step_x(), -forward.step_z()]]
}

impl BlockBehaviour for AbstractBedBlock {
    fn use_without_item(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, player: EntityId, _hit: &BlockHitResult) -> InteractionResult {
        let (mut state, mut pos) = (state, pos);
        let part = state.get(PART);
        let other_state = level.block_state(pos.relative(get_neighbour_direction(part, state.get(FACING))));
        if !(other_state.is_block(state.block()) && other_state.get(PART) != part) {
            return InteractionResult::CONSUME;
        }
        if part != BedPart::Head {
            pos = pos.relative(state.get(FACING));
            state = level.block_state(pos);
        }
        let bed_rule = get_bed_rule(self.kind, level, pos);
        if bed_rule.destroy_on_use {
            if let Some(message) = error_message(&bed_rule) {
                level.send_overlay_message(player, &message);
            }
            return destroy_on_use(self.kind, state, level, pos);
        }
        if state.get(OCCUPIED) {
            level.send_overlay_message(player, &Component::translatable("block.minecraft.bed.occupied"));
            return InteractionResult::SUCCESS_SERVER;
        }
        if let Err(problem) = level.start_sleep_in_bed(player, self.kind, state, &bed_rule, pos)
            && let Some(message) = problem
        {
            level.send_overlay_message(player, &message);
        }
        InteractionResult::SUCCESS_SERVER
    }

    fn update_shape(&self, state: BlockState, _level: &mut dyn LevelAccessor, _pos: BlockPos, direction: Direction, _: BlockPos, neighbour_state: BlockState) -> BlockState {
        if direction == get_neighbour_direction(state.get(PART), state.get(FACING)) {
            if neighbour_state.is_block(state.block()) && neighbour_state.get(PART) != state.get(PART) {
                state.set(OCCUPIED, neighbour_state.get(OCCUPIED))
            } else {
                blocks::AIR.default_state()
            }
        } else {
            state
        }
    }

    fn player_will_destroy(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState, player: EntityId) -> BlockState {
        if level.player(player).abilities.instabuild {
            let part = state.get(PART);
            if part == BedPart::Foot {
                let head_pos = pos.relative(get_neighbour_direction(part, state.get(FACING)));
                let head_state = level.block_state(head_pos);
                if head_state.is_block(state.block()) && head_state.get(PART) == BedPart::Head {
                    level.set_block(head_pos, blocks::AIR.default_state(), 35);
                    level.level_event(Some(player), crate::server::level::events::PARTICLES_AND_SOUND_DESTROY_BLOCK, head_pos, head_state.id() as i32);
                }
            }
        }
        if self.kind == BedKind::StrawBed {
            level.level_event(Some(player), crate::server::level::events::PARTICLES_DESTROY_BLOCK, pos, state.id() as i32);
            state
        } else {
            super::block_player_will_destroy(level, pos, state, player)
        }
    }

    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        let facing = ctx.horizontal_direction();
        let pos = ctx.clicked_pos();
        let relative = pos.relative(facing);
        let relative_state = level.block_state(relative);
        if super::of(relative_state).can_be_replaced(relative_state, ctx) && level.world_border.is_within_bounds_pos(relative) {
            Some(block.default_state().set(FACING, facing))
        } else {
            None
        }
    }

    fn set_placed_by(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState, _placer: Option<EntityId>, _item: ItemStack) {
        let other_pos = pos.relative(state.get(FACING));
        level.set_block(other_pos, state.set(PART, BedPart::Head), UPDATE_ALL);
    }

    fn rotate(&self, state: BlockState, rotation: Rotation) -> BlockState {
        state.set(FACING, rotation.rotate(state.get(FACING)))
    }

    fn mirror(&self, state: BlockState, mirror: super::mirror::Mirror) -> BlockState {
        super::rotate(state, mirror.get_rotation(state.get(FACING)))
    }
}
