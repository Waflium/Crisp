use crate::commands::brigadier::{CommandContext, CommandSyntaxException, SimpleCommandExceptionType};
use crate::commands::source::CommandSourceStack;
use crate::network::chat::{Arg, Component};
use crate::server::Server;
use crate::server::players::WhiteListEntry;
use crate::world::level::storage::format_uuid;

static ERROR_ALREADY_ENABLED: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.whitelist.alreadyOn");
static ERROR_ALREADY_DISABLED: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.whitelist.alreadyOff");
static ERROR_ALREADY_WHITELISTED: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.whitelist.add.failed");
static ERROR_NOT_WHITELISTED: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.whitelist.remove.failed");

pub fn reload(server: &mut Server, _context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    if server.whitelist.load().is_err() {
        warn!("Failed to load white-list: ");
    }
    source.send_success(server, || Component::translatable("commands.whitelist.reloaded"), true);
    server.kick_unlisted_players();
    Ok(1)
}

pub fn add_players(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let targets = context.get_game_profiles(server, source, "targets")?;
    let mut success = 0;
    for target in targets {
        if !server.whitelist.contains_key(&format_uuid(target.id)) {
            server.whitelist.add(WhiteListEntry { user: target.clone() });
            source.send_success(server, || Component::translatable_with("commands.whitelist.add.success", vec![Arg::Component(Component::literal(target.name.as_str()))]), true);
            success += 1;
        }
    }
    if success == 0 {
        return Err(ERROR_ALREADY_WHITELISTED.create());
    }
    Ok(success)
}

pub fn remove_players(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let targets = context.get_game_profiles(server, source, "targets")?;
    let mut success = 0;
    for target in targets {
        if server.whitelist.contains_key(&format_uuid(target.id)) {
            server.whitelist.remove_key(&format_uuid(target.id));
            source.send_success(
                server,
                || Component::translatable_with("commands.whitelist.remove.success", vec![Arg::Component(Component::literal(target.name.as_str()))]),
                true,
            );
            success += 1;
        }
    }
    if success == 0 {
        return Err(ERROR_NOT_WHITELISTED.create());
    }
    server.kick_unlisted_players();
    Ok(success)
}

pub fn enable(server: &mut Server, _context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    if server.config.white_list {
        return Err(ERROR_ALREADY_ENABLED.create());
    }
    server.set_using_whitelist(true);
    source.send_success(server, || Component::translatable("commands.whitelist.enabled"), true);
    server.kick_unlisted_players();
    Ok(1)
}

pub fn disable(server: &mut Server, _context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    if !server.config.white_list {
        return Err(ERROR_ALREADY_DISABLED.create());
    }
    server.set_using_whitelist(false);
    source.send_success(server, || Component::translatable("commands.whitelist.disabled"), true);
    Ok(1)
}

pub fn show_list(server: &mut Server, _context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let names: Vec<String> = server.whitelist.entries().into_iter().map(|e| e.user.name.clone()).collect();
    if names.is_empty() {
        source.send_success(server, || Component::translatable("commands.whitelist.none"), false);
    } else {
        source.send_success(server, || Component::translatable_with("commands.whitelist.list", vec![Arg::Int(names.len() as i32), names.join(", ").into()]), false);
    }
    Ok(names.len() as i32)
}

pub fn suggest_unlisted_players(
    server: &Server,
    _context: &CommandContext,
    _source: &CommandSourceStack,
    mut builder: crate::commands::suggestion::SuggestionsBuilder,
) -> Result<crate::commands::suggestion::Suggestions, CommandSyntaxException> {
    let names = server.level.players.values().filter(|p| !server.is_white_listed(p.profile.uuid)).map(|p| p.profile.name.as_str());
    crate::commands::shared_suggestion_provider::suggest(names, &mut builder);
    Ok(builder.build())
}

pub fn suggest_listed_players(
    server: &Server,
    _context: &CommandContext,
    _source: &CommandSourceStack,
    mut builder: crate::commands::suggestion::SuggestionsBuilder,
) -> Result<crate::commands::suggestion::Suggestions, CommandSyntaxException> {
    crate::commands::shared_suggestion_provider::suggest(server.whitelist.entries().into_iter().map(|e| e.user.name.as_str()), &mut builder);
    Ok(builder.build())
}
