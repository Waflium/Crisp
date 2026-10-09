use crisp_data::packets::play::clientbound;

use super::command_response_tracker::CommandResponseTracker;
use crate::commands::brigadier::{CommandContext, CommandSyntaxException};
use crate::commands::source::CommandSourceStack;
use crate::network::chat::{Component, component_utils};
use crate::server::Server;
use crate::world::entity::EntityId;

fn send_feedback(server: &Server, source: &CommandSourceStack, tracker: CommandResponseTracker<EntityId>, key: &str) -> Result<i32, CommandSyntaxException> {
    tracker.send_feedback(
        server,
        source,
        true,
        |player, _| Component::translatable_with(&format!("commands.title.{key}.single"), vec![server.entity_display_name(player).into()]),
        |count, _| Component::translatable_with(&format!("commands.title.{key}.multiple"), vec![count.into()]),
    )
}

fn send_packet_to_players(
    server: &mut Server,
    context: &CommandContext,
    source: &CommandSourceStack,
    id: i32,
    body: impl Fn(&mut crisp_protocol::Writer),
    key: &str,
) -> Result<i32, CommandSyntaxException> {
    let mut tracker = CommandResponseTracker::new();
    for player in context.get_players(server, source, "targets")? {
        server.level.player(player).connection.send(id, &body);
        tracker.track(player);
    }
    send_feedback(server, source, tracker, key)
}

pub fn clear_title(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    send_packet_to_players(server, context, source, clientbound::CLEAR_TITLES, |p| p.bool(false), "cleared")
}

pub fn reset_title(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    send_packet_to_players(server, context, source, clientbound::CLEAR_TITLES, |p| p.bool(true), "reset")
}

fn show_title(server: &mut Server, context: &CommandContext, source: &CommandSourceStack, id: i32, key: &str) -> Result<i32, CommandSyntaxException> {
    let title = context.get_raw_component("title");
    let mut tracker = CommandResponseTracker::new();
    for player in context.get_players(server, source, "targets")? {
        let resolved = component_utils::update_for_entity(server, source, title, Some(player))?;
        server.level.player(player).connection.send(id, |p| resolved.write(p));
        tracker.track(player);
    }
    send_feedback(server, source, tracker, key)
}

pub fn show_title_text(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    show_title(server, context, source, clientbound::SET_TITLE_TEXT, "show.title")
}

pub fn show_subtitle(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    show_title(server, context, source, clientbound::SET_SUBTITLE_TEXT, "show.subtitle")
}

pub fn show_action_bar(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    show_title(server, context, source, clientbound::SET_ACTION_BAR_TEXT, "show.actionbar")
}

pub fn set_times(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let (fade_in, stay, fade_out) = (context.get_int("fadeIn"), context.get_int("stay"), context.get_int("fadeOut"));
    let body = |p: &mut crisp_protocol::Writer| {
        p.i32(fade_in);
        p.i32(stay);
        p.i32(fade_out);
    };
    send_packet_to_players(server, context, source, clientbound::SET_TITLES_ANIMATION, body, "times")
}
