use crisp_core::{Axis, BlockPos, Direction};
use crisp_data::properties::{BELL_ATTACHMENT, BellAttachType, HORIZONTAL_FACING, POWERED};
use crisp_data::{Block, BlockState, SupportType, blocks, registry, registry_id};

use super::face_attached_horizontal_directional_block;
use super::mirror::Mirror;
use super::rotation::Rotation;
use super::{BlockBehaviour, BlockHitResult};
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::InteractionResult;
use crate::world::entity::EntityId;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::update_flags::UPDATE_ALL;
use crate::world::level::{BlockGetter, LevelAccessor, LevelReader};

pub const EVENT_BELL_RING: i32 = 1;

fn get_connected_direction(state: BlockState) -> Direction {
    match state.get(BELL_ATTACHMENT) {
        BellAttachType::Floor => Direction::Up,
        BellAttachType::Ceiling => Direction::Down,
        _ => state.get(HORIZONTAL_FACING).opposite(),
    }
}

pub struct BellBlock;

impl BellBlock {
    pub fn on_hit(&self, level: &mut ServerLevel, state: BlockState, hit_result: &BlockHitResult, player: Option<EntityId>, require_hit_from_correct_side: bool) -> bool {
        let direction = hit_result.direction;
        let block_pos = hit_result.block_pos;
        let proper_hit = !require_hit_from_correct_side || self.is_proper_hit(state, direction, hit_result.location.y - block_pos.y as f64);
        if proper_hit {
            self.attempt_to_ring(player, level, block_pos, Some(direction));
            true
        } else {
            false
        }
    }

    fn is_proper_hit(&self, state: BlockState, clicked_direction: Direction, click_y: f64) -> bool {
        if clicked_direction.axis() == Axis::Y || click_y > 0.8124f32 as f64 {
            return false;
        }
        let facing = state.get(HORIZONTAL_FACING);
        match state.get(BELL_ATTACHMENT) {
            BellAttachType::Floor => facing.axis() == clicked_direction.axis(),
            BellAttachType::SingleWall | BellAttachType::DoubleWall => facing.axis() != clicked_direction.axis(),
            BellAttachType::Ceiling => true,
        }
    }

    pub fn attempt_to_ring(&self, _ringing_entity: Option<EntityId>, level: &mut ServerLevel, pos: BlockPos, direction: Option<Direction>) -> bool {
        if !matches!(level.get_block_entity(pos).map(|e| &e.kind), Some(crate::world::level::block::entity::BlockEntityKind::Bell(_))) {
            return false;
        }
        let direction = direction.unwrap_or_else(|| level.block_state(pos).get(HORIZONTAL_FACING));
        level.bell_on_hit(pos, direction);
        level.play_sound(None, pos, registry_id(registry::SOUND_EVENT, "minecraft:block.bell.use"), SoundSource::Blocks, 2.0, 1.0);
        true
    }
}

impl BlockBehaviour for BellBlock {
    fn neighbor_changed(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, _block: Block, _moved_by_piston: bool) {
        let signal = level.has_neighbor_signal(pos);
        if signal != state.get(POWERED) {
            if signal {
                self.attempt_to_ring(None, level, pos, None);
            }
            level.set_block(pos, state.set(POWERED, signal), UPDATE_ALL);
        }
    }

    fn use_without_item(&self, state: BlockState, level: &mut ServerLevel, _pos: BlockPos, player: EntityId, hit: &BlockHitResult) -> InteractionResult {
        if self.on_hit(level, state, hit, Some(player), true) { InteractionResult::SUCCESS } else { InteractionResult::Pass }
    }

    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        let clicked_face = ctx.clicked_face();
        let pos = ctx.clicked_pos();
        let axis = clicked_face.axis();
        if axis == Axis::Y {
            let state = block
                .default_state()
                .set(BELL_ATTACHMENT, if clicked_face == Direction::Down { BellAttachType::Ceiling } else { BellAttachType::Floor })
                .set(HORIZONTAL_FACING, ctx.horizontal_direction());
            if self.can_survive(state, level, pos) {
                return Some(state);
            }
        } else {
            let sturdy = |level: &mut ServerLevel, at: BlockPos, face: Direction| level.block_state(at).is_face_sturdy(face, SupportType::Full);
            let double_attached = axis == Axis::X && sturdy(level, pos.west(), Direction::East) && sturdy(level, pos.east(), Direction::West)
                || axis == Axis::Z && sturdy(level, pos.north(), Direction::South) && sturdy(level, pos.south(), Direction::North);
            let mut state = block
                .default_state()
                .set(HORIZONTAL_FACING, clicked_face.opposite())
                .set(BELL_ATTACHMENT, if double_attached { BellAttachType::DoubleWall } else { BellAttachType::SingleWall });
            if self.can_survive(state, level, pos) {
                return Some(state);
            }
            let can_attach_below = sturdy(level, pos.below(), Direction::Up);
            state = state.set(BELL_ATTACHMENT, if can_attach_below { BellAttachType::Floor } else { BellAttachType::Ceiling });
            if self.can_survive(state, level, pos) {
                return Some(state);
            }
        }
        None
    }

    fn update_shape(&self, state: BlockState, level: &mut dyn LevelAccessor, pos: BlockPos, direction: Direction, _: BlockPos, neighbour_state: BlockState) -> BlockState {
        let attachment = state.get(BELL_ATTACHMENT);
        let connected_direction = get_connected_direction(state).opposite();
        if connected_direction == direction && !self.can_survive(state, level, pos) && attachment != BellAttachType::DoubleWall {
            return blocks::AIR.default_state();
        }
        if direction.axis() == state.get(HORIZONTAL_FACING).axis() {
            if attachment == BellAttachType::DoubleWall && !neighbour_state.is_face_sturdy(direction, SupportType::Full) {
                return state.set(BELL_ATTACHMENT, BellAttachType::SingleWall).set(HORIZONTAL_FACING, direction.opposite());
            }
            if attachment == BellAttachType::SingleWall
                && connected_direction.opposite() == direction
                && neighbour_state.is_face_sturdy(state.get(HORIZONTAL_FACING), SupportType::Full)
            {
                return state.set(BELL_ATTACHMENT, BellAttachType::DoubleWall);
            }
        }
        state
    }

    fn can_survive(&self, state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> bool {
        let connection_dir = get_connected_direction(state).opposite();
        if connection_dir == Direction::Up {
            super::can_support_center(level, pos.above(), Direction::Down)
        } else {
            face_attached_horizontal_directional_block::can_attach(level, pos, connection_dir)
        }
    }

    fn trigger_event(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos, b0: i32, b1: i32) -> bool {
        level.bell_trigger_event(pos, b0, b1)
    }

    fn rotate(&self, state: BlockState, rotation: Rotation) -> BlockState {
        state.set(HORIZONTAL_FACING, rotation.rotate(state.get(HORIZONTAL_FACING)))
    }

    fn mirror(&self, state: BlockState, mirror: Mirror) -> BlockState {
        super::rotate(state, mirror.get_rotation(state.get(HORIZONTAL_FACING)))
    }
}
