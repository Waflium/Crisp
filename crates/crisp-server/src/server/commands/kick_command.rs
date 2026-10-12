use crate::commands::brigadier::{CommandContext, CommandSyntaxException, SimpleCommandExceptionType};
use crate::commands::source::CommandSourceStack;
use crate::network::chat::Component;
use crate::server::Server;
use crate::world::entity::EntityId;

static ERROR_KICKING_OWNER: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.kick.owner.failed");

pub fn kick(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let players = context.get_players(server, source, "targets")?;
    kick_players(server, source, &players, Component::translatable("multiplayer.disconnect.kicked"))
}

pub fn kick_with_reason(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let players = context.get_players(server, source, "targets")?;
    let reason = context.get_message(server, source, "reason")?;
    kick_players(server, source, &players, reason)
}

fn kick_players(server: &mut Server, source: &CommandSourceStack, players: &[EntityId], reason: Component) -> Result<i32, CommandSyntaxException> {
    let mut count = 0;
    for &player in players {
        server.player_mut(player).disconnect_with(reason.clone());
        source.send_success(server, || Component::translatable_with("commands.kick.success", vec![server.entity_display_name(player).into(), reason.clone().into()]), true);
        count += 1;
    }
    if count == 0 {
        return Err(ERROR_KICKING_OWNER.create());
    }
    Ok(count)
}
