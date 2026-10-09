use crisp_data::packets::play::clientbound;

use super::command_response_tracker::{CommandResponseTracker, ElementType};
use crate::commands::brigadier::{CommandContext, CommandSyntaxException, SimpleCommandExceptionType};
use crate::commands::source::CommandSourceStack;
use crate::network::chat::Component;
use crate::server::Server;
use crate::world::entity::EntityId;

static ERROR_NO_PLAYERS: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.transfer.error.no_players");

pub fn transfer(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let hostname = context.get_string("hostname").to_string();
    let port = if context.has_argument("port") { context.get_int("port") } else { 25565 };
    let players = if context.has_argument("players") { context.get_players(server, source, "players")? } else { vec![source.player_or_exception(server)?] };
    let mut tracker: CommandResponseTracker<EntityId> = CommandResponseTracker::new();
    for player in players {
        server.level.player(player).connection.send(clientbound::TRANSFER, |w| {
            w.string(&hostname);
            w.varint(port);
        });
        tracker.track(player);
    }
    tracker.send_feedback_of(
        server,
        source,
        true,
        ElementType::NonZero,
        Some(|| ERROR_NO_PLAYERS.create()),
        |player, _| Component::translatable_with("commands.transfer.success.single", vec![server.entity_display_name(player).into(), hostname.as_str().into(), port.into()]),
        |count, _| Component::translatable_with("commands.transfer.success.multiple", vec![count.into(), hostname.as_str().into(), port.into()]),
    )
}
