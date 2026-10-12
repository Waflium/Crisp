use crate::commands::brigadier::{CommandContext, CommandSyntaxException};
use crate::commands::source::CommandSourceStack;
use crate::network::chat::{Arg, ChatFormatting, Component};
use crate::server::Server;
use crate::world::level::storage::format_uuid;

pub fn format_list(values: Vec<Component>) -> Component {
    if values.len() == 1 {
        return values.into_iter().next().unwrap();
    }
    let mut result = Component::empty();
    for (i, value) in values.into_iter().enumerate() {
        if i > 0 {
            result = result.append(Component::literal(", ").with_color(ChatFormatting::Gray));
        }
        result = result.append(value);
    }
    result
}

fn format(server: &Server, source: &CommandSourceStack, names: Vec<Component>) -> i32 {
    let count = names.len() as i32;
    let max = server.config.max_players as i32;
    let list = format_list(names);
    source.send_success(server, || Component::translatable_with("commands.list.players", vec![Arg::Int(count), Arg::Int(max), Arg::Component(list)]), false);
    count
}

pub fn list_players(server: &mut Server, _context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let names = server.player_list.iter().map(|&id| server.entity_display_name(id)).collect();
    Ok(format(server, source, names))
}

pub fn list_players_with_uuids(server: &mut Server, _context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let names = server
        .players()
        .map(|p| {
            Component::translatable_with(
                "commands.list.nameAndId",
                vec![Arg::Component(Component::literal(p.profile.name.as_str())), Arg::Component(Component::literal(format_uuid(p.profile.uuid)))],
            )
        })
        .collect();
    Ok(format(server, source, names))
}
