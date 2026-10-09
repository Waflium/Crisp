use super::command_response_tracker::{CommandResponseTracker, ElementType};
use super::list_players_command::format_list;
use crate::commands::brigadier::{CommandContext, CommandSyntaxException, SimpleCommandExceptionType};
use crate::commands::source::CommandSourceStack;
use crate::network::chat::{Arg, ChatFormatting, Component};
use crate::server::Server;
use crate::world::entity::EntityId;

static ERROR_ADD_FAILED: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.tag.add.failed");
static ERROR_REMOVE_FAILED: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.tag.remove.failed");

pub fn add_tag(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let name = context.get_string("name");
    let mut response = CommandResponseTracker::new();
    for entity in context.get_entities(server, source, "targets")? {
        let added = server.level.entity_mut(entity).add_tag(name);
        response.track_value(entity, added as i32);
    }
    send_with_name(server, source, response, || ERROR_ADD_FAILED.create(), "add", name)
}

pub fn remove_tag(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let name = context.get_string("name");
    let mut response = CommandResponseTracker::new();
    for entity in context.get_entities(server, source, "targets")? {
        let removed = server.level.entity_mut(entity).remove_tag(name);
        response.track_value(entity, removed as i32);
    }
    send_with_name(server, source, response, || ERROR_REMOVE_FAILED.create(), "remove", name)
}

fn send_with_name(
    server: &Server,
    source: &CommandSourceStack,
    response: CommandResponseTracker<EntityId>,
    on_zero: fn() -> CommandSyntaxException,
    key: &str,
    name: &str,
) -> Result<i32, CommandSyntaxException> {
    response.send_feedback_of(
        server,
        source,
        true,
        ElementType::NonZero,
        Some(on_zero),
        |entity, _| Component::translatable_with(&format!("commands.tag.{key}.success.single"), vec![name.into(), server.entity_display_name(entity).into()]),
        |count, _| Component::translatable_with(&format!("commands.tag.{key}.success.multiple"), vec![name.into(), count.into()]),
    )
}

pub fn list_tags(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let mut response = CommandResponseTracker::new();
    let mut tags: Vec<String> = Vec::new();
    for entity in context.get_entities(server, source, "targets")? {
        let entity_tags = &server.level.entity(entity).tags;
        for tag in entity_tags {
            if !tags.contains(tag) {
                tags.push(tag.clone());
            }
        }
        response.track_value(entity, entity_tags.len() as i32);
    }
    let count = tags.len() as i32;
    if tags.is_empty() {
        response.send_feedback(
            server,
            source,
            false,
            |entity, _| Component::translatable_with("commands.tag.list.single.empty", vec![server.entity_display_name(entity).into()]),
            |count, _| Component::translatable_with("commands.tag.list.multiple.empty", vec![count.into()]),
        )?;
    } else {
        tags.sort();
        let list = format_list(tags.into_iter().map(|t| Component::literal(t).with_color(ChatFormatting::Green)).collect());
        response.send_feedback(
            server,
            source,
            false,
            |entity, _| {
                Component::translatable_with(
                    "commands.tag.list.single.success",
                    vec![server.entity_display_name(entity).into(), Arg::Int(count), list.clone().into()],
                )
            },
            |entities, _| Component::translatable_with("commands.tag.list.multiple.success", vec![Arg::Int(entities), Arg::Int(count), list.clone().into()]),
        )?;
    }
    Ok(count)
}

pub fn suggest_tags(
    server: &Server,
    context: &CommandContext,
    source: &CommandSourceStack,
    mut builder: crate::commands::suggestion::SuggestionsBuilder,
) -> Result<crate::commands::suggestion::Suggestions, CommandSyntaxException> {
    let mut tags: Vec<&str> = Vec::new();
    for entity in context.get_entities(server, source, "targets")? {
        for tag in &server.level.entity(entity).tags {
            if !tags.contains(&tag.as_str()) {
                tags.push(tag);
            }
        }
    }
    crate::commands::shared_suggestion_provider::suggest(tags, &mut builder);
    Ok(builder.build())
}
