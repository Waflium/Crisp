use super::command_response_tracker::{CommandResponseTracker, ElementType};
use crate::commands::brigadier::{CommandContext, CommandSyntaxException, SimpleCommandExceptionType};
use crate::commands::source::CommandSourceStack;
use crate::network::chat::Component;
use crate::server::Server;
use crate::world::entity::EntityId;

static ERROR_ADD_FAILED: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.posteffect.add.failed");
static ERROR_CLEAR_FAILED: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.posteffect.clear.failed");
static ERROR_REMOVE_FAILED: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.posteffect.remove.failed");

type Result<T> = std::result::Result<T, CommandSyntaxException>;

pub fn add_post_effect(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    let players = context.get_players(server, source, "targets")?;
    let posteffect = context.get_identifier("posteffect").to_string();
    let mut tracker = CommandResponseTracker::new();
    for player in players {
        let added = server.player_mut(player).add_post_effect(&posteffect);
        tracker.track_value(player, added as i32);
    }
    send_feedback_with_arg(server, source, tracker, || ERROR_ADD_FAILED.create(), "commands.posteffect.add", &posteffect)
}

pub fn clear_post_effect(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    let players = context.get_players(server, source, "targets")?;
    let mut tracker = CommandResponseTracker::new();
    for player in players {
        let cleared = server.player_mut(player).clear_post_effects();
        tracker.track_value(player, cleared as i32);
    }
    tracker.send_feedback_of(
        server,
        source,
        true,
        ElementType::NonZero,
        Some(|| ERROR_CLEAR_FAILED.create()),
        |player, _| Component::translatable_with("commands.posteffect.clear.success.single", vec![server.entity_display_name(player).into()]),
        |count, _| Component::translatable_with("commands.posteffect.clear.success.multiple", vec![count.into()]),
    )
}

pub fn list_post_effects(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    let player = context.get_player(server, source, "target")?;
    let post_effects = server.player(player).get_post_effects();
    let count = post_effects.len() as i32;
    if post_effects.is_empty() {
        source.send_success(server, || Component::translatable_with("commands.posteffect.list.empty", vec![server.entity_display_name(player).into()]), false);
    } else {
        let names = post_effects.join(", ");
        source.send_success(
            server,
            || Component::translatable_with("commands.posteffect.list.success", vec![server.entity_display_name(player).into(), count.into(), names.into()]),
            false,
        );
    }
    Ok(count)
}

pub fn remove_post_effect(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    let players = context.get_players(server, source, "targets")?;
    let posteffect = context.get_identifier("posteffect").to_string();
    let mut tracker = CommandResponseTracker::new();
    for player in players {
        let removed = server.player_mut(player).remove_post_effect(&posteffect);
        tracker.track_value(player, removed as i32);
    }
    send_feedback_with_arg(server, source, tracker, || ERROR_REMOVE_FAILED.create(), "commands.posteffect.remove", &posteffect)
}

fn send_feedback_with_arg(
    server: &Server,
    source: &CommandSourceStack,
    tracker: CommandResponseTracker<EntityId>,
    on_zero: fn() -> CommandSyntaxException,
    key: &str,
    posteffect: &str,
) -> Result<i32> {
    tracker.send_feedback_of(
        server,
        source,
        true,
        ElementType::NonZero,
        Some(on_zero),
        |player, _| {
            Component::translatable_with(
                &format!("{key}.success.single"),
                vec![Component::literal(posteffect).into(), server.entity_display_name(player).into()],
            )
        },
        |count, _| Component::translatable_with(&format!("{key}.success.multiple"), vec![Component::literal(posteffect).into(), count.into()]),
    )
}
