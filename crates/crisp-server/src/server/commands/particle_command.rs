use crisp_core::Vec3;

use crate::commands::brigadier::{CommandContext, CommandSyntaxException, SimpleCommandExceptionType};
use crate::commands::source::CommandSourceStack;
use crate::core::particles::particle_options::ParticleOptions;
use crate::network::chat::Component;
use crate::server::Server;
use crate::world::entity::EntityId;

static ERROR_FAILED: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.particle.failed");

type Result<T> = std::result::Result<T, CommandSyntaxException>;

pub fn particle_here(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    let players = server.player_list.clone();
    send_particles(server, source, context.get_particle("name"), source.position, Vec3::ZERO, 0.0, 0, false, &players)
}

pub fn particle_at(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    let pos = context.get_coordinates("pos").position(server, source);
    let players = server.player_list.clone();
    send_particles(server, source, context.get_particle("name"), pos, Vec3::ZERO, 0.0, 0, false, &players)
}

pub fn particle_with_delta(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    let force = context.nodes.iter().any(|n| crisp_data::commands::CommandNode::get(n.node).name == "force");
    let pos = context.get_coordinates("pos").position(server, source);
    let delta = context.get_coordinates("delta").position(server, source);
    let players = if context.has_argument("viewers") { context.get_players(server, source, "viewers")? } else { server.player_list.clone() };
    send_particles(server, source, context.get_particle("name"), pos, delta, context.get_float("speed"), context.get_int("count"), force, &players)
}

#[allow(clippy::too_many_arguments)]
fn send_particles(
    server: &mut Server,
    source: &CommandSourceStack,
    particle: &ParticleOptions,
    pos: Vec3,
    delta: Vec3,
    speed: f32,
    count: i32,
    force: bool,
    players: &[EntityId],
) -> Result<i32> {
    let mut result = 0;
    let level = &server.levels[source.level];
    for &player in players {
        if level.send_particles_to_player(player, particle, force, false, pos, count, delta, speed as f64) {
            result += 1;
        }
    }
    if result == 0 {
        return Err(ERROR_FAILED.create());
    }
    source.send_success(server, || Component::translatable_with("commands.particle.success", vec![particle.get_type().into()]), true);
    Ok(result)
}
