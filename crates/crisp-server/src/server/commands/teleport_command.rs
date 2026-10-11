use crisp_core::{BlockPos, Vec3, mth};
use crisp_data::packets::play::clientbound;

use super::command_response_tracker::CommandResponseTracker;
use super::look_at::LookAt;
use crate::commands::arguments::coordinates::Coordinates;
use crate::commands::brigadier::{CommandContext, CommandSyntaxException, SimpleCommandExceptionType};
use crate::commands::source::{Anchor, CommandSourceStack};
use crate::network::chat::Component;
use crate::network::connection::encode;
use crate::server::Server;
use crate::server::level::ServerLevel;
use crate::util::java_format_fixed;
use crate::world::entity::EntityId;
use crate::world::entity::relative::{PositionMoveRotation, relative};
use crate::world::level::is_in_spawnable_bounds;

static INVALID_POSITION: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.teleport.invalidPosition");

type Result<T> = std::result::Result<T, CommandSyntaxException>;

pub fn teleport_to_location(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    let entity = source.entity_or_exception()?;
    teleport_to_pos(server, source, &[entity], context.get_coordinates("location"), None, None)
}

pub fn teleport_to_destination(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    let entity = source.entity_or_exception()?;
    let destination = context.get_entity(server, source, "destination")?;
    teleport_to_entity(server, source, &[entity], destination)
}

pub fn teleport_targets_to_location(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    let targets = context.get_entities(server, source, "targets")?;
    teleport_to_pos(server, source, &targets, context.get_coordinates("location"), None, None)
}

pub fn teleport_targets_to_location_rotated(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    let targets = context.get_entities(server, source, "targets")?;
    teleport_to_pos(server, source, &targets, context.get_coordinates("location"), Some(context.get_coordinates("rotation")), None)
}

pub fn teleport_targets_facing_entity(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    let targets = context.get_entities(server, source, "targets")?;
    let entity = context.get_entity(server, source, "facingEntity")?;
    let look_at = LookAt::Entity { entity, anchor: Anchor::Feet };
    teleport_to_pos(server, source, &targets, context.get_coordinates("location"), None, Some(look_at))
}

pub fn teleport_targets_facing_entity_anchor(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    let targets = context.get_entities(server, source, "targets")?;
    let entity = context.get_entity(server, source, "facingEntity")?;
    let look_at = LookAt::Entity { entity, anchor: context.get_anchor("facingAnchor") };
    teleport_to_pos(server, source, &targets, context.get_coordinates("location"), None, Some(look_at))
}

pub fn teleport_targets_facing_location(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    let targets = context.get_entities(server, source, "targets")?;
    let look_at = LookAt::Position(context.get_coordinates("facingLocation").position(server, source));
    teleport_to_pos(server, source, &targets, context.get_coordinates("location"), None, Some(look_at))
}

pub fn teleport_targets_to_destination(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    let targets = context.get_entities(server, source, "targets")?;
    let destination = context.get_entity(server, source, "destination")?;
    teleport_to_entity(server, source, &targets, destination)
}

fn teleport_to_entity(server: &mut Server, source: &CommandSourceStack, entities: &[EntityId], destination: EntityId) -> Result<i32> {
    let mut tracker = CommandResponseTracker::new();
    for &entity in entities {
        let d = server.level.entity(destination);
        let (pos, y_rot, x_rot) = (d.position(), d.y_rot, d.x_rot);
        perform_teleport(&mut server.level, source, entity, pos, 0, y_rot, x_rot, None)?;
        tracker.track(entity);
    }
    let server = &*server;
    tracker.send_feedback(
        server,
        source,
        true,
        |entity, _| Component::translatable_with("commands.teleport.success.entity.single", vec![server.entity_display_name(entity).into(), server.entity_display_name(destination).into()]),
        |count, _| Component::translatable_with("commands.teleport.success.entity.multiple", vec![count.into(), server.entity_display_name(destination).into()]),
    )
}

fn teleport_to_pos(
    server: &mut Server,
    source: &CommandSourceStack,
    entities: &[EntityId],
    destination: &Coordinates,
    rotation: Option<&Coordinates>,
    look_at: Option<LookAt>,
) -> Result<i32> {
    let mut tracker = CommandResponseTracker::new();
    let pos = destination.position(server, source);
    let rot = rotation.map(|r| r.rotation(source));
    for &entity in entities {
        let relatives = relatives(destination, rotation);
        let (y_rot, x_rot) = match rot {
            None => {
                let e = server.level.entity(entity);
                (e.y_rot, e.x_rot)
            }
            Some((x, y)) => (y, x),
        };
        perform_teleport(&mut server.level, source, entity, pos, relatives, y_rot, x_rot, look_at)?;
        tracker.track(entity);
    }
    let server = &*server;
    let (x, y, z) = (format_double(pos.x), format_double(pos.y), format_double(pos.z));
    tracker.send_feedback(
        server,
        source,
        true,
        |entity, _| {
            Component::translatable_with(
                "commands.teleport.success.location.single",
                vec![server.entity_display_name(entity).into(), x.as_str().into(), y.as_str().into(), z.as_str().into()],
            )
        },
        |count, _| Component::translatable_with("commands.teleport.success.location.multiple", vec![count.into(), x.as_str().into(), y.as_str().into(), z.as_str().into()]),
    )
}

fn relatives(destination: &Coordinates, rotation: Option<&Coordinates>) -> u16 {
    let (x, y, z) = (destination.is_x_relative(), destination.is_y_relative(), destination.is_z_relative());
    let rot = match rotation {
        None => relative::ROTATION,
        Some(r) => relative::rotation(r.is_y_relative(), r.is_x_relative()),
    };
    relative::direction(x, y, z) | relative::position(x, y, z) | rot
}

fn format_double(value: f64) -> String {
    java_format_fixed(value, 6)
}

#[allow(clippy::too_many_arguments)]
fn perform_teleport(
    level: &mut ServerLevel,
    source: &CommandSourceStack,
    victim: EntityId,
    pos: Vec3,
    relatives: u16,
    y_rot: f32,
    x_rot: f32,
    look_at: Option<LookAt>,
) -> Result<()> {
    if !is_in_spawnable_bounds(BlockPos::containing_vec(pos)) {
        return Err(INVALID_POSITION.create());
    }
    let e = level.entity(victim);
    let has = |r: u16| relatives & r != 0;
    let x = if has(relative::X) { pos.x - e.x() } else { pos.x };
    let y = if has(relative::Y) { pos.y - e.y() } else { pos.y };
    let z = if has(relative::Z) { pos.z - e.z() } else { pos.z };
    let new_y_rot = mth::wrap_degrees(if has(relative::Y_ROT) { y_rot - e.y_rot } else { y_rot });
    let new_x_rot = mth::wrap_degrees(if has(relative::X_ROT) { x_rot - e.x_rot } else { x_rot });
    if teleport_to(level, victim, Vec3::new(x, y, z), relatives, new_y_rot, new_x_rot) {
        if let Some(look_at) = look_at {
            look_at.perform(level, source, victim);
        }
        let fall_flying = level.is_living(victim) && level.entity(victim).is_fall_flying();
        if !fall_flying {
            let e = level.entity_mut(victim);
            e.set_delta_movement(e.delta_movement.multiply(1.0, 0.0, 1.0));
            level.set_on_ground(victim, true);
        }
    }
    Ok(())
}

fn teleport_to(level: &mut ServerLevel, id: EntityId, pos: Vec3, relatives: u16, y_rot: f32, x_rot: f32) -> bool {
    if level.entity(id).is_removed() {
        return false;
    }
    let destination = PositionMoveRotation { position: pos, delta_movement: Vec3::ZERO, y_rot, x_rot };
    if let Some(player) = level.players.get_mut(&id) {
        player.teleport_relative(destination, relatives);
        player.reset_position();
        let e = &mut player.entity;
        e.y_head_rot = if relatives & relative::Y_ROT != 0 { e.y_head_rot + y_rot } else { y_rot };
        player.movement.above_ground_tick_count = 0;
        return true;
    }
    let e = level.entity_mut(id);
    let current = PositionMoveRotation { position: e.position(), delta_movement: e.delta_movement, y_rot: e.y_rot, x_rot: e.x_rot };
    e.teleport_set_position(current, destination, relatives);
    level.entity_moved(id);
    let e = level.entity(id);
    let packet = encode(clientbound::TELEPORT_ENTITY, |w| {
        w.varint(id);
        PositionMoveRotation { position: e.position(), delta_movement: e.delta_movement, y_rot: e.y_rot, x_rot: e.x_rot }.write(w);
        w.i32(0);
        w.bool(e.on_ground);
    });
    level.send_to_tracking_players(id, &packet);
    true
}

#[cfg(test)]
mod tests {
    #[test]
    fn formats_like_java() {
        let cases = [
            (1.5, "1.500000"),
            (-0.0, "-0.000000"),
            (0.0000005, "0.000001"),
            (99.9999996, "100.000000"),
            (12345678.25, "12345678.250000"),
            (1e-9, "0.000000"),
            (2.0000005, "2.000001"),
            (-3.4999995, "-3.500000"),
            (1e20, "100000000000000000000.000000"),
            (0.1234565, "0.123457"),
            (30000000.0000005, "30000000.000001"),
        ];
        for (value, expected) in cases {
            assert_eq!(super::format_double(value), expected, "{value}");
        }
    }
}
