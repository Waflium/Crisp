use std::borrow::Cow;

use crate::commands::brigadier::{CommandContext, CommandSyntaxException};
use crate::commands::source::CommandSourceStack;
use crate::network::chat::{Arg, ClickEvent, Component, HoverEvent, TextColor};
use crate::server::Server;
use crate::server::commands::list_players_command::format_list;
use crate::world::entity::EntityId;
use crate::world::scores::score_holder::ScoreHolder;
use crate::world::scores::team_color::TeamColor;
use crate::world::waypoints::waypoint::Icon;
use crate::world::waypoints::waypoint_style_assets;

type Result<T> = std::result::Result<T, CommandSyntaxException>;

pub fn style_reset(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    let waypoint = context.get_waypoint(server, source, "waypoint")?;
    Ok(set_waypoint_style(server, source, waypoint, Cow::Borrowed(waypoint_style_assets::DEFAULT)))
}

pub fn style_set(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    let waypoint = context.get_waypoint(server, source, "waypoint")?;
    let style = context.get_identifier("style").to_string();
    Ok(set_waypoint_style(server, source, waypoint, Cow::Owned(style)))
}

pub fn color(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    let waypoint = context.get_waypoint(server, source, "waypoint")?;
    let color = context.get_team_color("color");
    Ok(set_waypoint_color(server, source, waypoint, color))
}

pub fn color_hex(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    let waypoint = context.get_waypoint(server, source, "waypoint")?;
    let color = context.get_hex_color("color");
    Ok(set_waypoint_hex_color(server, source, waypoint, color))
}

pub fn color_reset(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    let waypoint = context.get_waypoint(server, source, "waypoint")?;
    Ok(reset_waypoint_color(server, source, waypoint))
}

fn set_waypoint_style(server: &mut Server, source: &CommandSourceStack, waypoint: EntityId, style: Cow<'static, str>) -> i32 {
    mutate_icon(server, source, waypoint, |icon| icon.style = style);
    source.send_success(server, || Component::translatable("commands.waypoint.modify.style"), false);
    0
}

fn set_waypoint_color(server: &mut Server, source: &CommandSourceStack, waypoint: EntityId, color: TeamColor) -> i32 {
    mutate_icon(server, source, waypoint, |icon| icon.color = Some(color.rgb()));
    source.send_success(
        server,
        || Component::translatable_with("commands.waypoint.modify.color", vec![Component::literal(color.serialized_name()).with_text_color(color.text_color()).into()]),
        false,
    );
    0
}

fn set_waypoint_hex_color(server: &mut Server, source: &CommandSourceStack, waypoint: EntityId, color: i32) -> i32 {
    mutate_icon(server, source, waypoint, |icon| icon.color = Some(color));
    source.send_success(
        server,
        || {
            let hex = format!("{:06X}", color & 0xFFFFFF);
            Component::translatable_with("commands.waypoint.modify.color", vec![Component::literal(hex).with_text_color(TextColor::from_rgb(color as u32)).into()])
        },
        false,
    );
    0
}

fn reset_waypoint_color(server: &mut Server, source: &CommandSourceStack, waypoint: EntityId) -> i32 {
    mutate_icon(server, source, waypoint, |icon| icon.color = None);
    source.send_success(server, || Component::translatable("commands.waypoint.modify.color.reset"), false);
    0
}

pub fn list_waypoints(server: &mut Server, _context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    let level = &server.levels[source.level];
    let waypoints: Vec<EntityId> = level.get_waypoint_manager().transmitters().iter().collect();
    let dimension = level.dimension;
    if waypoints.is_empty() {
        source.send_success(server, || Component::translatable_with("commands.waypoint.list.empty", vec![Arg::String(dimension.to_string())]), false);
        return Ok(0);
    }
    let waypoint_names = format_list(
        waypoints
            .iter()
            .map(|&transmitter| {
                let pos = level.entity(transmitter).block_position();
                ScoreHolder::for_entity(level, transmitter)
                    .feedback_display_name(level)
                    .with_click(ClickEvent::SuggestCommand(format!("/execute in {dimension} run tp @s {} {} {}", pos.x, pos.y, pos.z)))
                    .with_hover(HoverEvent::ShowText(Box::new(Component::translatable("chat.coordinates.tooltip"))))
                    .with_text_color(TextColor::from_rgb(level.living(transmitter).locator_bar_icon.color.unwrap_or(-1) as u32))
            })
            .collect(),
    );
    let count = waypoints.len() as i32;
    source.send_success(
        server,
        || Component::translatable_with("commands.waypoint.list.success", vec![Arg::Int(count), Arg::String(dimension.to_string()), waypoint_names.into()]),
        false,
    );
    Ok(count)
}

fn mutate_icon(server: &mut Server, source: &CommandSourceStack, waypoint: EntityId, icon_consumer: impl FnOnce(&mut Icon)) {
    let level = &mut server.levels[source.level];
    if level.entity_ref(waypoint).is_none() {
        let level = server.entity_level_mut(waypoint);
        icon_consumer(level.waypoint_icon(waypoint));
        return;
    }
    level.get_waypoint_manager().untrack_waypoint(level, waypoint);
    icon_consumer(level.waypoint_icon(waypoint));
    level.get_waypoint_manager().track_waypoint(level, waypoint);
}
