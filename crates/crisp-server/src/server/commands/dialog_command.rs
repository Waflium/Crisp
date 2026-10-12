use crisp_data::packets::play::clientbound;

use super::command_response_tracker::CommandResponseTracker;
use crate::commands::brigadier::{CommandContext, CommandSyntaxException};
use crate::commands::source::CommandSourceStack;
use crate::network::chat::Component;
use crate::server::Server;

type Result<T> = std::result::Result<T, CommandSyntaxException>;

pub fn show_dialog(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    let targets = context.get_players(server, source, "targets")?;
    let dialog = context.get_resource_or_id("dialog");
    let mut tracker = CommandResponseTracker::new();
    for target in targets {
        server.player(target).connection.send(clientbound::SHOW_DIALOG, |w| crate::server::dialog::dialog::write_holder(dialog, w));
        tracker.track(target);
    }
    tracker.send_feedback(
        server,
        source,
        true,
        |player, _| Component::translatable_with("commands.dialog.show.single", vec![server.entity_display_name(player).into()]),
        |count, _| Component::translatable_with("commands.dialog.show.multiple", vec![count.into()]),
    )
}

pub fn clear_dialog(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    let targets = context.get_players(server, source, "targets")?;
    let mut tracker = CommandResponseTracker::new();
    for target in targets {
        server.player(target).connection.send(clientbound::CLEAR_DIALOG, |_| {});
        tracker.track(target);
    }
    tracker.send_feedback(
        server,
        source,
        true,
        |player, _| Component::translatable_with("commands.dialog.clear.single", vec![server.entity_display_name(player).into()]),
        |count, _| Component::translatable_with("commands.dialog.clear.multiple", vec![count.into()]),
    )
}
