use std::net::IpAddr;

use crate::commands::brigadier::{CommandContext, CommandSyntaxException, SimpleCommandExceptionType};
use crate::commands::source::CommandSourceStack;
use crate::network::chat::{Arg, Component};
use crate::server::Server;
use crate::server::players::{Ban, IpBanEntry, UserBanEntry};
use crate::world::entity::EntityId;
use crate::world::level::storage::format_uuid;

static ERROR_ALREADY_BANNED: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.ban.failed");
static ERROR_NOT_BANNED: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.pardon.failed");
static ERROR_INVALID_IP: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.banip.invalid");
static ERROR_IP_ALREADY_BANNED: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.banip.failed");
static ERROR_PARDON_IP_INVALID: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.pardonip.invalid");
static ERROR_IP_NOT_BANNED: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.pardonip.failed");

fn reason(server: &Server, context: &CommandContext, source: &CommandSourceStack) -> Result<Option<String>, CommandSyntaxException> {
    if !context.has_argument("reason") {
        return Ok(None);
    }
    Ok(Some(context.get_message(server, source, "reason")?.get_string()))
}

fn is_inet_address(text: &str) -> bool {
    text.parse::<IpAddr>().is_ok()
}

pub fn ban_players(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let players = context.get_game_profiles(server, source, "targets")?;
    let reason = reason(server, context, source)?;
    let mut count = 0;
    for player in players {
        let key = format_uuid(player.id);
        if server.bans.contains_key(&key) {
            continue;
        }
        let entry = UserBanEntry { user: player.clone(), ban: Ban::new(Some(source.text_name(server)), None, reason.clone()) };
        let message = entry.ban.reason_message();
        server.bans.add(entry);
        count += 1;
        source.send_success(
            server,
            || Component::translatable_with("commands.ban.success", vec![Arg::Component(Component::literal(player.name.as_str())), Arg::Component(message)]),
            true,
        );
        if let Some(online) = server.level.players.values().find(|p| p.profile.uuid == player.id).map(|p| p.entity.id) {
            server.level.player_mut(online).disconnect("multiplayer.disconnect.banned");
        }
    }
    if count == 0 {
        return Err(ERROR_ALREADY_BANNED.create());
    }
    Ok(count)
}

pub fn pardon_players(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let players = context.get_game_profiles(server, source, "targets")?;
    let mut count = 0;
    for player in players {
        if server.bans.contains_key(&format_uuid(player.id)) {
            server.bans.remove_key(&format_uuid(player.id));
            count += 1;
            source.send_success(server, || Component::translatable_with("commands.pardon.success", vec![Arg::Component(Component::literal(player.name.as_str()))]), true);
        }
    }
    if count == 0 {
        return Err(ERROR_NOT_BANNED.create());
    }
    Ok(count)
}

pub fn ban_ip(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let target = context.get_string("target").to_string();
    let reason = reason(server, context, source)?;
    let ip = if is_inet_address(&target) {
        target
    } else {
        match server.level.players.values().find(|p| p.profile.name.eq_ignore_ascii_case(&target)) {
            Some(player) => player.ip_address(),
            None => return Err(ERROR_INVALID_IP.create()),
        }
    };
    if server.ip_bans.contains_key(&ip) {
        return Err(ERROR_IP_ALREADY_BANNED.create());
    }
    let players: Vec<EntityId> = server.level.players.values().filter(|p| p.ip_address() == ip).map(|p| p.entity.id).collect();
    let entry = IpBanEntry { ip: ip.clone(), ban: Ban::new(Some(source.text_name(server)), None, reason) };
    let message = entry.ban.reason_message();
    server.ip_bans.add(entry);
    source.send_success(server, || Component::translatable_with("commands.banip.success", vec![ip.as_str().into(), Arg::Component(message)]), true);
    if !players.is_empty() {
        let names = super::list_players_command::format_list(players.iter().map(|&id| server.entity_display_name(id)).collect());
        source.send_success(server, || Component::translatable_with("commands.banip.info", vec![Arg::Int(players.len() as i32), Arg::Component(names)]), true);
    }
    for &id in &players {
        server.level.player_mut(id).disconnect("multiplayer.disconnect.ip_banned");
    }
    Ok(players.len() as i32)
}

pub fn pardon_ip(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let ip = context.get_string("target").to_string();
    if !is_inet_address(&ip) {
        return Err(ERROR_PARDON_IP_INVALID.create());
    }
    if !server.ip_bans.contains_key(&ip) {
        return Err(ERROR_IP_NOT_BANNED.create());
    }
    server.ip_bans.remove_key(&ip);
    source.send_success(server, || Component::translatable_with("commands.pardonip.success", vec![ip.as_str().into()]), true);
    Ok(1)
}

fn show_list(server: &Server, source: &CommandSourceStack, entries: Vec<(Component, &Ban)>) -> i32 {
    if entries.is_empty() {
        source.send_success(server, || Component::translatable("commands.banlist.none"), false);
    } else {
        source.send_success(server, || Component::translatable_with("commands.banlist.list", vec![Arg::Int(entries.len() as i32)]), false);
        for (name, ban) in &entries {
            source.send_success(
                server,
                || {
                    Component::translatable_with(
                        "commands.banlist.entry",
                        vec![Arg::Component(name.clone()), ban.source.as_str().into(), Arg::Component(ban.reason_message())],
                    )
                },
                false,
            );
        }
    }
    entries.len() as i32
}

fn player_entries(server: &Server) -> Vec<(Component, &Ban)> {
    server.bans.entries().into_iter().map(|e| (Component::literal(e.user.name.as_str()), &e.ban)).collect()
}

fn ip_entries(server: &Server) -> Vec<(Component, &Ban)> {
    server.ip_bans.entries().into_iter().map(|e| (Component::literal(e.ip.as_str()), &e.ban)).collect()
}

pub fn ban_list(server: &mut Server, _context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let mut entries = player_entries(server);
    entries.extend(ip_entries(server));
    Ok(show_list(server, source, entries))
}

pub fn ban_list_ips(server: &mut Server, _context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    Ok(show_list(server, source, ip_entries(server)))
}

pub fn ban_list_players(server: &mut Server, _context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    Ok(show_list(server, source, player_entries(server)))
}

pub fn suggest_banned_players(
    server: &Server,
    _context: &CommandContext,
    _source: &CommandSourceStack,
    mut builder: crate::commands::suggestion::SuggestionsBuilder,
) -> Result<crate::commands::suggestion::Suggestions, CommandSyntaxException> {
    crate::commands::shared_suggestion_provider::suggest(server.bans.entries().into_iter().map(|e| e.user.name.as_str()), &mut builder);
    Ok(builder.build())
}

pub fn suggest_banned_ips(
    server: &Server,
    _context: &CommandContext,
    _source: &CommandSourceStack,
    mut builder: crate::commands::suggestion::SuggestionsBuilder,
) -> Result<crate::commands::suggestion::Suggestions, CommandSyntaxException> {
    crate::commands::shared_suggestion_provider::suggest(server.ip_bans.entries().into_iter().map(|e| e.ip.as_str()), &mut builder);
    Ok(builder.build())
}
