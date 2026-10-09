use crisp_core::BlockPos;

use super::set_spawn_command::{ZERO_ROTATION, spawnable_pos};
use crate::commands::arguments::coordinates::Coordinates;
use crate::commands::brigadier::{CommandContext, CommandSyntaxException};
use crate::commands::source::CommandSourceStack;
use crate::network::chat::Component;
use crate::server::Server;
use crate::world::level::storage::RespawnData;

pub fn set_spawn_here(server: &mut Server, _context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    Ok(set_spawn(server, source, BlockPos::containing_vec(source.position), &ZERO_ROTATION))
}

pub fn set_spawn_at(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let pos = spawnable_pos(server, context, source, "pos")?;
    Ok(set_spawn(server, source, pos, &ZERO_ROTATION))
}

pub fn set_spawn_at_rotated(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let pos = spawnable_pos(server, context, source, "pos")?;
    Ok(set_spawn(server, source, pos, context.get_coordinates("rotation")))
}

fn set_spawn(server: &mut Server, source: &CommandSourceStack, pos: BlockPos, rotation: &Coordinates) -> i32 {
    let (pitch, yaw) = rotation.rotation(source);
    let dimension = server.level.dimension;
    let respawn_data = RespawnData::of(dimension, pos, yaw, pitch);
    let (yaw, pitch) = (respawn_data.yaw, respawn_data.pitch);
    server.set_respawn_data(respawn_data);
    source.send_success(
        server,
        || {
            Component::translatable_with(
                "commands.setworldspawn.success",
                vec![pos.x.into(), pos.y.into(), pos.z.into(), yaw.into(), pitch.into(), dimension.into()],
            )
        },
        true,
    );
    1
}
