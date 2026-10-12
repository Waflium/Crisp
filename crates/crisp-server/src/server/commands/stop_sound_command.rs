use crisp_data::packets::play::clientbound;

use crate::commands::CommandFn;
use crisp_data::commands::CommandNode;
use crate::commands::brigadier::{CommandContext, CommandSyntaxException};
use crate::commands::source::CommandSourceStack;
use crate::network::chat::Component;
use crate::server::Server;
use crate::server::level::SoundSource;

pub fn executors() -> Vec<(String, CommandFn)> {
    let mut out: Vec<(String, CommandFn)> = vec![("stopsound targets".into(), stop_sound), ("stopsound targets * sound".into(), stop_sound)];
    for source in SoundSource::VALUES {
        out.push((format!("stopsound targets {}", source.name()), stop_sound));
        out.push((format!("stopsound targets {} sound", source.name()), stop_sound));
    }
    out
}

fn stop_sound(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let targets = context.get_players(server, source, "targets")?;
    let sound_source = context.nodes.get(2).and_then(|n| SoundSource::by_name(CommandNode::get(n.node).name));
    let sound = context.has_argument("sound").then(|| context.get_identifier("sound").to_string());
    for &player in &targets {
        server.player(player).connection.send(clientbound::STOP_SOUND, |w| match (sound_source, &sound) {
            (Some(s), Some(name)) => {
                w.u8(3);
                w.varint(s as i32);
                w.string(name);
            }
            (Some(s), None) => {
                w.u8(1);
                w.varint(s as i32);
            }
            (None, Some(name)) => {
                w.u8(2);
                w.string(name);
            }
            (None, None) => w.u8(0),
        });
    }
    let message = match (sound_source, &sound) {
        (Some(s), Some(name)) => Component::translatable_with("commands.stopsound.success.source.sound", vec![Component::literal(name.clone()).into(), s.name().into()]),
        (Some(s), None) => Component::translatable_with("commands.stopsound.success.source.any", vec![s.name().into()]),
        (None, Some(name)) => Component::translatable_with("commands.stopsound.success.sourceless.sound", vec![Component::literal(name.clone()).into()]),
        (None, None) => Component::translatable("commands.stopsound.success.sourceless.any"),
    };
    source.send_success(server, || message.clone(), true);
    Ok(targets.len() as i32)
}
