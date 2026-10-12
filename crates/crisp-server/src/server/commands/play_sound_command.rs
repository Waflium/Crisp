use crisp_core::Vec3;
use crisp_core::random::RandomSource;
use crisp_data::packets::play::clientbound;

use super::command_response_tracker::{CommandResponseTracker, ElementType};
use crate::commands::CommandFn;
use crisp_data::commands::CommandNode;
use crate::commands::brigadier::{CommandContext, CommandSyntaxException, SimpleCommandExceptionType};
use crate::commands::source::CommandSourceStack;
use crate::network::chat::Component;
use crate::server::Server;
use crate::server::level::SoundSource;
use crate::world::entity::EntityId;

static ERROR_TOO_FAR: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.playsound.failed");

pub fn executors() -> Vec<(String, CommandFn)> {
    let mut out: Vec<(String, CommandFn)> = vec![("playsound sound".into(), play_sound)];
    for source in SoundSource::VALUES {
        let mut path = format!("playsound sound {}", source.name());
        out.push((path.clone(), play_sound));
        for argument in ["targets", "pos", "volume", "pitch", "minVolume"] {
            path = format!("{path} {argument}");
            out.push((path.clone(), play_sound));
        }
    }
    out
}

fn play_sound(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let sound = context.get_identifier("sound").to_string();
    let sound_source = if context.nodes.len() > 2 {
        SoundSource::by_name(CommandNode::get(context.nodes[2].node).name).expect("a sound source literal")
    } else {
        SoundSource::Master
    };
    let players = if context.has_argument("targets") {
        context.get_players(server, source, "targets")?
    } else {
        source.player_id(server).into_iter().collect()
    };
    let position = if context.has_argument("pos") { context.get_coordinates("pos").position(server, source) } else { source.position };
    let float = |name: &str, default: f32| if context.has_argument(name) { context.get_float(name) } else { default };
    let (volume, pitch, min_volume) = (float("volume", 1.0), float("pitch", 1.0), float("minVolume", 0.0));
    let range = if volume > 1.0 { 16.0 * volume } else { 16.0 };
    let max_dist_sqr = (range * range) as f64;
    let seed = server.levels[source.level].random.next_long();
    let mut tracker: CommandResponseTracker<EntityId> = CommandResponseTracker::new();
    for player in players {
        if server.level_of(player) != Some(source.level) {
            continue;
        }
        let p = server.levels[source.level].entity(player).position();
        let (dx, dy, dz) = (position.x - p.x, position.y - p.y, position.z - p.z);
        let dist_sqr = dx * dx + dy * dy + dz * dz;
        let (mut local_position, mut local_volume) = (position, volume);
        if dist_sqr > max_dist_sqr {
            if min_volume <= 0.0 {
                continue;
            }
            let distance = dist_sqr.sqrt();
            local_position = Vec3::new(p.x + dx / distance * 2.0, p.y + dy / distance * 2.0, p.z + dz / distance * 2.0);
            local_volume = min_volume;
        }
        server.player(player).connection.send(clientbound::SOUND, |w| {
            w.varint(0);
            w.string(&sound);
            w.bool(false);
            w.varint(sound_source as i32);
            w.i32((local_position.x * 8.0) as i32);
            w.i32((local_position.y * 8.0) as i32);
            w.i32((local_position.z * 8.0) as i32);
            w.f32(local_volume);
            w.f32(pitch);
            w.i64(seed);
        });
        tracker.track(player);
    }
    tracker.send_feedback_of(
        server,
        source,
        true,
        ElementType::NonZero,
        Some(|| ERROR_TOO_FAR.create()),
        |player, _| Component::translatable_with("commands.playsound.success.single", vec![Component::literal(sound.clone()).into(), server.entity_display_name(player).into()]),
        |count, _| Component::translatable_with("commands.playsound.success.multiple", vec![Component::literal(sound.clone()).into(), count.into()]),
    )
}
