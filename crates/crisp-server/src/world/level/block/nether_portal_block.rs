use crisp_core::random::RandomSource;
use crisp_core::{Axis, BlockPos, Direction, Vec3};
use crisp_data::properties::HORIZONTAL_AXIS;
use crisp_data::{BlockState, DimensionType, blocks, game_rule};

use super::BlockBehaviour;
use super::rotation::Rotation;
use crate::server::Server;
use crate::server::level::ServerLevel;
use crate::util::block_util::{FoundRectangle, get_largest_rectangle_around};
use crate::world::attribute::environment_attributes::NETHER_PORTAL_SPAWNS_PIGLINS;
use crate::world::difficulty::Difficulty;
use crate::world::entity::EntityId;
use crate::world::entity::inside_block::InsideBlockEffects;
use crate::world::entity::relative::relative;
use crate::world::level::block::portal::{Portal, PortalEntity};
use crate::world::level::portal::portal_forcer;
use crate::world::level::portal::portal_shape::{self, PortalShape};
use crate::world::level::portal::teleport_transition::{PostTeleportTransition, TeleportTransition};
use crate::server::{OVERWORLD, THE_NETHER};
use crate::world::level::{BlockGetter, LevelAccessor};

pub struct NetherPortalBlock;

impl BlockBehaviour for NetherPortalBlock {
    fn random_tick(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        if level.is_spawning_monsters()
            && level.difficulty != Difficulty::Peaceful
            && level.environment_attributes().get_value_at(NETHER_PORTAL_SPAWNS_PIGLINS, pos)
            && level.random.next_int_bounded(2000) < level.difficulty.id()
            && level.any_player_close_enough_for_spawning(pos.chunk())
        {
            let mut pos = pos;
            while level.block_state(pos).is_block(blocks::NETHER_PORTAL) {
                pos = pos.below();
            }
        }
    }

    fn update_shape(
        &self,
        state: BlockState,
        level: &mut dyn LevelAccessor,
        pos: BlockPos,
        direction_to_neighbour: Direction,
        _neighbour_pos: BlockPos,
        neighbour_state: BlockState,
    ) -> BlockState {
        let update_axis = direction_to_neighbour.axis();
        let axis = state.get(HORIZONTAL_AXIS);
        let wrong_axis = axis != update_axis && update_axis.is_horizontal();
        if !wrong_axis && !neighbour_state.is_block(blocks::NETHER_PORTAL) && !PortalShape::find_any_shape(level, pos, axis).is_complete() {
            blocks::AIR.default_state()
        } else {
            state
        }
    }

    fn entity_inside(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos, entity: EntityId, _effects: &mut InsideBlockEffects, _is_precise: bool) {
        if level.can_use_portal(entity, false) {
            level.set_as_inside_portal(entity, Portal::NetherPortal, pos);
        }
    }

    fn rotate(&self, state: BlockState, rotation: Rotation) -> BlockState {
        match rotation {
            Rotation::Counterclockwise90 | Rotation::Clockwise90 => match state.get(HORIZONTAL_AXIS) {
                Axis::X => state.set(HORIZONTAL_AXIS, Axis::Z),
                Axis::Z => state.set(HORIZONTAL_AXIS, Axis::X),
                Axis::Y => state,
            },
            _ => state,
        }
    }
}

pub fn get_portal_transition_time(level: &ServerLevel, entity: EntityId) -> i32 {
    match level.players.get(&entity) {
        Some(player) => {
            let rule = if player.abilities.invulnerable { game_rule::PLAYERS_NETHER_PORTAL_CREATIVE_DELAY } else { game_rule::PLAYERS_NETHER_PORTAL_DEFAULT_DELAY };
            level.game_rules.get_int(rule).max(0)
        }
        None => 0,
    }
}

pub fn get_portal_destination(server: &mut Server, current_level: usize, entity: &PortalEntity, portal_entry_pos: BlockPos) -> Option<TeleportTransition> {
    let new_dimension = if server.levels[current_level].dimension == THE_NETHER { OVERWORLD } else { THE_NETHER };
    let new_level = server.get_level(new_dimension)?;
    let to_nether = server.levels[new_level].dimension == THE_NETHER;
    let teleportation_scale = get_teleportation_scale(server.levels[current_level].dimension_type(), server.levels[new_level].dimension_type());
    let new_world_border = &server.levels[new_level].world_border;
    let approximate_exit_pos =
        new_world_border.clamp_to_bounds(entity.position.x * teleportation_scale, entity.position.y, entity.position.z * teleportation_scale);
    let exit_portal_pos = portal_forcer::find_closest_portal_position(&mut server.levels[new_level], approximate_exit_pos, to_nether);
    let (exit_portal, post) = match exit_portal_pos {
        Some(pos) => {
            let level = &mut server.levels[new_level];
            let portal_state = level.block_state(pos);
            let exit_portal = get_largest_rectangle_around(pos, portal_state.get(HORIZONTAL_AXIS), 21, Axis::Y, 21, |block_pos| level.block_state(block_pos) == portal_state);
            (exit_portal, PostTeleportTransition::PLAY_PORTAL_SOUND.then(PostTeleportTransition::place_portal_ticket_at(pos)))
        }
        None => {
            if entity.is_spectator {
                return None;
            }
            let entry_state = server.levels[current_level].block_state(portal_entry_pos);
            let source_portal_axis = if entry_state.has(HORIZONTAL_AXIS) { entry_state.get(HORIZONTAL_AXIS) } else { Axis::X };
            let Some(created_exit) = portal_forcer::create_portal(&mut server.levels[new_level], approximate_exit_pos, source_portal_axis) else {
                error!("Unable to create a portal, likely target out of worldborder");
                return None;
            };
            (created_exit, PostTeleportTransition::PLAY_PORTAL_SOUND.then(PostTeleportTransition::PLACE_PORTAL_TICKET))
        }
    };
    Some(get_dimension_transition_from_exit(server, current_level, entity, portal_entry_pos, exit_portal, new_level, post))
}

fn get_teleportation_scale(source: &DimensionType, destination: &DimensionType) -> f64 {
    source.coordinate_scale / destination.coordinate_scale
}

fn get_dimension_transition_from_exit(
    server: &mut Server,
    current_level: usize,
    entity: &PortalEntity,
    portal_entry_pos: BlockPos,
    exit_portal: FoundRectangle,
    new_level: usize,
    post_teleport_transition: PostTeleportTransition,
) -> TeleportTransition {
    let level = &mut server.levels[current_level];
    let block_state = level.block_state(portal_entry_pos);
    let (axis, offset) = if block_state.has(HORIZONTAL_AXIS) {
        let axis = block_state.get(HORIZONTAL_AXIS);
        let portal_area = get_largest_rectangle_around(portal_entry_pos, axis, 21, Axis::Y, 21, |pos| level.block_state(pos) == block_state);
        (axis, portal_shape::get_relative_position(portal_area, axis, entity.position, entity.dimensions))
    } else {
        (Axis::X, Vec3::new(0.5, 0.0, 0.0))
    };
    create_dimension_transition(&mut server.levels[new_level], new_level, exit_portal, axis, offset, entity, post_teleport_transition)
}

fn create_dimension_transition(
    level: &mut ServerLevel,
    new_level: usize,
    found_rectangle: FoundRectangle,
    portal_axis: Axis,
    offset: Vec3,
    entity: &PortalEntity,
    post_teleport_transition: PostTeleportTransition,
) -> TeleportTransition {
    let bottom_left = found_rectangle.min_corner;
    let block_state = level.block_state(bottom_left);
    let axis = if block_state.has(HORIZONTAL_AXIS) { block_state.get(HORIZONTAL_AXIS) } else { Axis::X };
    let width = found_rectangle.axis1_size as f64;
    let height = found_rectangle.axis2_size as f64;
    let dimensions = entity.dimensions;
    let output_rotation = if portal_axis == axis { 0 } else { 90 };
    let offset_right = dimensions.width as f64 / 2.0 + (width - dimensions.width as f64) * offset.x;
    let offset_up = (height - dimensions.height as f64) * offset.y;
    let offset_forward = 0.5 + offset.z;
    let x_aligned = axis == Axis::X;
    let target_pos = Vec3::new(
        bottom_left.x as f64 + if x_aligned { offset_right } else { offset_forward },
        bottom_left.y as f64 + offset_up,
        bottom_left.z as f64 + if x_aligned { offset_forward } else { offset_right },
    );
    let collision_free_pos = portal_shape::find_collision_free_position(target_pos, level, &entity.collision_context, dimensions);
    TeleportTransition::with_relatives(
        new_level,
        collision_free_pos,
        Vec3::ZERO,
        output_rotation as f32,
        0.0,
        relative::DELTA | relative::ROTATION,
        post_teleport_transition,
    )
}
