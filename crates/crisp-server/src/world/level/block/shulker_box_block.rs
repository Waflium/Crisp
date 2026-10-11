use crisp_core::{AABB, BlockPos, Direction, Vec3};
use crisp_data::properties::FACING;
use crisp_data::{Block, BlockState};

use super::mirror::Mirror;
use super::rotation::Rotation;
use super::{BlockBehaviour, BlockHitResult};
use crate::server::level::ServerLevel;
use crate::world::InteractionResult;
use crate::world::entity::{EntityId, ItemEntity, LevelEntity};
use crate::world::inventory::menu_provider::MenuProvider;
use crate::world::item::ItemStack;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::block::entity::{AnimationStatus, BlockEntityKind};

pub struct ShulkerBoxBlock;

pub fn progress_delta_aabb(direction: Direction, progress_from: f32, progress_to: f32, position: Vec3) -> AABB {
    let bounds = AABB::new(-0.5, 0.0, -0.5, 0.5, 1.0, 0.5);
    let max = progress_from.max(progress_to) as f64;
    let min = progress_from.min(progress_to) as f64;
    let (x, y, z) = (direction.step_x() as f64, direction.step_y() as f64, direction.step_z() as f64);
    bounds.expand_towards(x * max, y * max, z * max).contract(-x * (1.0 + min), -y * (1.0 + min), -z * (1.0 + min)).move_by(position.x, position.y, position.z)
}

fn can_open(state: BlockState, level: &mut ServerLevel, pos: BlockPos, status: AnimationStatus) -> bool {
    if status != AnimationStatus::Closed {
        return true;
    }
    let bottom_center = Vec3::new(pos.x as f64 + 0.5, pos.y as f64, pos.z as f64 + 0.5);
    let lid = progress_delta_aabb(state.get(FACING), 0.0, 0.5, bottom_center).deflate_all(1.0e-6);
    level.no_collision(&lid)
}

impl BlockBehaviour for ShulkerBoxBlock {
    fn rotate(&self, state: BlockState, rotation: Rotation) -> BlockState {
        state.set(FACING, rotation.rotate(state.get(FACING)))
    }

    fn mirror(&self, state: BlockState, mirror: Mirror) -> BlockState {
        super::rotate(state, mirror.get_rotation(state.get(FACING)))
    }

    fn use_without_item(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, player: EntityId, _hit: &BlockHitResult) -> InteractionResult {
        let status = match level.get_block_entity(pos).map(|e| &e.kind) {
            Some(BlockEntityKind::ShulkerBox(shulker)) => Some(shulker.animation_status),
            _ => None,
        };
        if let Some(status) = status
            && can_open(state, level, pos, status)
        {
            level.open_menu(player, MenuProvider::BlockEntity(pos));
        }
        InteractionResult::SUCCESS
    }

    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, _level: &mut ServerLevel) -> Option<BlockState> {
        Some(block.default_state().set(FACING, ctx.clicked_face()))
    }

    fn player_will_destroy(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState, player: EntityId) -> BlockState {
        let creative = level.player(player).abilities.instabuild;
        if let Some(entity) = level.get_block_entity(pos)
            && matches!(entity.kind, BlockEntityKind::ShulkerBox(_))
        {
            let empty = entity.container_base().is_some_and(|b| b.items.iter().all(ItemStack::is_empty));
            if creative && !empty {
                let components = entity.collect_components();
                let mut stack = ItemStack::new(state.block().item(), 1);
                for (ty, value) in components {
                    if let Some(value) = value {
                        stack.set(ty, Some(value));
                    }
                }
                let mut item = ItemEntity::new(Vec3::new(pos.x as f64 + 0.5, pos.y as f64 + 0.5, pos.z as f64 + 0.5), stack);
                item.set_default_pickup_delay();
                level.add_fresh_entity(LevelEntity::Item(item));
            } else {
                level.unpack_loot_table(pos, Some(player));
            }
        }
        super::block_player_will_destroy(level, pos, state, player)
    }

    fn affect_neighbors_after_removal(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, _moved_by_piston: bool) {
        crate::world::containers::update_neighbours_after_destroy(level, state, pos);
    }

    fn get_analog_output_signal(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos, _direction: Direction) -> i32 {
        level.get_redstone_signal_from_block_entity(pos)
    }

    fn trigger_event(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos, b0: i32, b1: i32) -> bool {
        level.get_block_entity(pos).is_some_and(|e| e.trigger_event(b0, b1))
    }
}
