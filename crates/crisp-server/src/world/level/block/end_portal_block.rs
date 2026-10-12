use crisp_core::{AABB, BlockPos, Direction, Vec3};
use crisp_data::{BlockState, Fluid};

use super::BlockBehaviour;
use crate::server::level::ServerLevel;
use crate::server::{Server, THE_END};
use crate::world::entity::EntityId;
use crate::world::entity::inside_block::InsideBlockEffects;
use crate::world::entity::relative::relative;
use crate::world::level::block::portal::{Portal, PortalEntity};
use crate::world::level::chunk::HeightmapType;
use crate::world::level::levelgen::feature::end_platform_feature;
use crate::world::level::portal::teleport_transition::{PostTeleportTransition, TeleportTransition};

pub const END_SPAWN_POINT: BlockPos = BlockPos::new(100, 50, 0);

pub struct EndPortalBlock;

impl BlockBehaviour for EndPortalBlock {
    fn entity_inside_collision_shape(&self, _state: BlockState, _level: &mut ServerLevel, _pos: BlockPos, _entity: EntityId) -> Option<Vec<AABB>> {
        Some(vec![AABB::new(0.0, 6.0 / 16.0, 0.0, 1.0, 12.0 / 16.0, 1.0)])
    }

    fn entity_inside(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos, entity: EntityId, _effects: &mut InsideBlockEffects, _is_precise: bool) {
        if level.can_use_portal(entity, false) {
            if level.dimension == THE_END && level.is_player(entity) && !level.player(entity).seen_credits {
                level.show_end_credits(entity);
            } else {
                level.set_as_inside_portal(entity, Portal::EndPortal, pos);
            }
        }
    }

    fn can_be_replaced_by_fluid(&self, _state: BlockState, _fluid: Fluid) -> bool {
        false
    }
}

pub fn get_portal_destination(server: &mut Server, current_level: usize, entity: &PortalEntity, _portal_entry_pos: BlockPos) -> Option<TeleportTransition> {
    let respawn_data = server.respawn_data().clone();
    let from_end = server.levels[current_level].dimension == THE_END;
    let new_dimension = if from_end { respawn_data.dimension.as_str() } else { THE_END };
    let spawn_block_pos = if from_end { respawn_data.pos } else { END_SPAWN_POINT };
    let new_level = server.get_level(new_dimension)?;
    let mut spawn_pos = Vec3::at_bottom_center_of(spawn_block_pos);
    let is_player = server.has_player(entity.id);
    let (y_rot, x_rot, relatives) = if !from_end {
        end_platform_feature::create_end_platform(&mut server.levels[new_level], BlockPos::containing_vec(spawn_pos).below(), true);
        if is_player {
            spawn_pos = spawn_pos.add(0.0, -1.0, 0.0);
        }
        (Direction::West.to_y_rot(), 0.0, relative::DELTA | relative::X_ROT)
    } else {
        if is_player {
            return Some(server.find_respawn_teleport_transition(entity.id, false, PostTeleportTransition::DO_NOTHING));
        }
        let adjusted = adjust_spawn_location(&mut server.levels[new_level], respawn_data.pos);
        spawn_pos = Vec3::at_bottom_center_of(adjusted);
        (respawn_data.yaw, respawn_data.pitch, relative::DELTA | relative::ROTATION)
    };
    Some(TeleportTransition::with_relatives(
        new_level,
        spawn_pos,
        Vec3::ZERO,
        y_rot,
        x_rot,
        relatives,
        PostTeleportTransition::PLAY_PORTAL_SOUND.then(PostTeleportTransition::PLACE_PORTAL_TICKET),
    ))
}

fn adjust_spawn_location(level: &mut ServerLevel, spawn_block_pos: BlockPos) -> BlockPos {
    let spawn_pos = Vec3::at_center_of(spawn_block_pos);
    let spawn_height = level.chunks.chunk(spawn_block_pos.chunk()).height_at(HeightmapType::MotionBlockingNoLeaves, spawn_block_pos.x, spawn_block_pos.z) + 1;
    BlockPos::containing(spawn_pos.x, spawn_height as f64, spawn_pos.z)
}
