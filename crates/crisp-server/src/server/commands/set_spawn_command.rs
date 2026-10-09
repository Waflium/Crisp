use crisp_core::{BlockPos, mth};

use super::command_response_tracker::CommandResponseTracker;
use super::teleport_command::is_in_spawnable_bounds;
use crate::commands::arguments::coordinates::{Coordinates, ERROR_OUT_OF_BOUNDS, WorldCoordinate};
use crate::commands::brigadier::{CommandContext, CommandSyntaxException};
use crate::commands::source::CommandSourceStack;
use crate::network::chat::{Arg, Component};
use crate::server::Server;
use crate::server::level::player::RespawnConfig;
use crate::world::entity::EntityId;
use crate::world::level::storage::RespawnData;

type Result<T> = std::result::Result<T, CommandSyntaxException>;

pub const ZERO_ROTATION: Coordinates = Coordinates::World {
    x: WorldCoordinate { relative: true, value: 0.0 },
    y: WorldCoordinate { relative: true, value: 0.0 },
    z: WorldCoordinate { relative: true, value: 0.0 },
};

pub fn spawnable_pos(server: &Server, context: &CommandContext, source: &CommandSourceStack, name: &str) -> Result<BlockPos> {
    let pos = context.get_coordinates(name).block_pos(server, source);
    if !is_in_spawnable_bounds(pos) {
        return Err(ERROR_OUT_OF_BOUNDS.create());
    }
    Ok(pos)
}

pub fn set_spawn_self(server: &mut Server, _context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    let player = source.player_or_exception(server)?;
    set_spawn(server, source, &[player], BlockPos::containing_vec(source.position), &ZERO_ROTATION)
}

pub fn set_spawn_targets(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    let targets = context.get_players(server, source, "targets")?;
    set_spawn(server, source, &targets, BlockPos::containing_vec(source.position), &ZERO_ROTATION)
}

pub fn set_spawn_targets_at(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    let targets = context.get_players(server, source, "targets")?;
    let pos = spawnable_pos(server, context, source, "pos")?;
    set_spawn(server, source, &targets, pos, &ZERO_ROTATION)
}

pub fn set_spawn_targets_at_rotated(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    let targets = context.get_players(server, source, "targets")?;
    let pos = spawnable_pos(server, context, source, "pos")?;
    set_spawn(server, source, &targets, pos, context.get_coordinates("rotation"))
}

fn set_spawn(server: &mut Server, source: &CommandSourceStack, targets: &[EntityId], pos: BlockPos, rotation: &Coordinates) -> Result<i32> {
    let mut tracker = CommandResponseTracker::new();
    let (x_rot, y_rot) = rotation.rotation(source);
    let yaw = mth::wrap_degrees(y_rot);
    let pitch = mth::clamp_f(x_rot, -90.0, 90.0);
    let respawn_data = RespawnData::of(server.level.dimension, pos, yaw, pitch);
    for &target in targets {
        server.level.player_mut(target).respawn_config = Some(RespawnConfig { respawn_data: respawn_data.clone(), forced: true });
        tracker.track(target);
    }
    let server = &*server;
    let args = |last: Arg| -> Vec<Arg> {
        let d = &respawn_data;
        let dimension = Component::literal(d.dimension.as_str());
        vec![d.pos.x.into(), d.pos.y.into(), d.pos.z.into(), d.yaw.into(), d.pitch.into(), dimension.into(), last]
    };
    tracker.send_feedback(
        server,
        source,
        true,
        |player, _| Component::translatable_with("commands.spawnpoint.success.single", args(server.entity_display_name(player).into())),
        |count, _| Component::translatable_with("commands.spawnpoint.success.multiple", args(count.into())),
    )
}
