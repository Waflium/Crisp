use super::command_response_tracker::CommandResponseTracker;
use crate::commands::brigadier::{CommandContext, CommandSyntaxException};
use crate::commands::source::CommandSourceStack;
use crate::network::chat::Component;
use crate::server::Server;
use crate::world::entity::{EntityId, RemovalReason};

pub fn kill_self(server: &mut Server, _context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let entity = source.entity_or_exception()?;
    kill(server, source, &[entity])
}

pub fn kill_targets(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let targets = context.get_entities(server, source, "targets")?;
    kill(server, source, &targets)
}

fn kill(server: &mut Server, source: &CommandSourceStack, victims: &[EntityId]) -> Result<i32, CommandSyntaxException> {
    let mut tracker = CommandResponseTracker::new();
    for &entity in victims {
        tracker.track(server.entity_display_name(entity));
        let level = server.entity_level_mut(entity);
        if level.is_living(entity) {
            level.kill(entity);
        } else {
            level.entity_mut(entity).removal = Some(RemovalReason::Killed);
            level.remove_entity(entity);
        }
    }
    tracker.send_feedback(
        server,
        source,
        true,
        |name, _| Component::translatable_with("commands.kill.success.single", vec![name.into()]),
        |count, _| Component::translatable_with("commands.kill.success.multiple", vec![count.into()]),
    )
}
