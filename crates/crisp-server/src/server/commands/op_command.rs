use crate::commands::brigadier::{CommandContext, CommandSyntaxException, SimpleCommandExceptionType};
use crate::commands::source::CommandSourceStack;
use crate::network::chat::Component;
use crate::server::Server;

static ERROR_ALREADY_OP: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.op.failed");

pub fn op_players(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let players = context.get_game_profiles(server, source, "targets")?;
    let mut count = 0;
    for player in players {
        if !server.is_op(player.id) {
            server.op(player.clone(), None, None);
            count += 1;
            source.send_success(server, || Component::translatable_with("commands.op.success", vec![player.name.as_str().into()]), true);
        }
    }
    if count == 0 {
        return Err(ERROR_ALREADY_OP.create());
    }
    Ok(count)
}

pub fn suggest_non_operators(
    server: &Server,
    _context: &CommandContext,
    _source: &CommandSourceStack,
    mut builder: crate::commands::suggestion::SuggestionsBuilder,
) -> Result<crate::commands::suggestion::Suggestions, CommandSyntaxException> {
    let names = server.level.players.values().filter(|p| !server.is_op(p.profile.uuid)).map(|p| p.profile.name.as_str());
    crate::commands::shared_suggestion_provider::suggest(names, &mut builder);
    Ok(builder.build())
}
