use crate::commands::brigadier::{CommandContext, CommandSyntaxException, SimpleCommandExceptionType};
use crate::commands::source::CommandSourceStack;
use crate::network::chat::Component;
use crate::server::Server;

static ERROR_NOT_OP: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.deop.failed");

pub fn deop_players(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let players = context.get_game_profiles(server, source, "targets")?;
    let mut count = 0;
    for player in players {
        if server.is_op(player.id) {
            server.deop(&player);
            count += 1;
            source.send_success(server, || Component::translatable_with("commands.deop.success", vec![player.name.as_str().into()]), true);
        }
    }
    if count == 0 {
        return Err(ERROR_NOT_OP.create());
    }
    server.kick_unlisted_players();
    Ok(count)
}

pub fn suggest_operators(
    server: &Server,
    _context: &CommandContext,
    _source: &CommandSourceStack,
    mut builder: crate::commands::suggestion::SuggestionsBuilder,
) -> Result<crate::commands::suggestion::Suggestions, CommandSyntaxException> {
    crate::commands::shared_suggestion_provider::suggest(server.ops.entries().into_iter().map(|e| e.user.name.as_str()), &mut builder);
    Ok(builder.build())
}
