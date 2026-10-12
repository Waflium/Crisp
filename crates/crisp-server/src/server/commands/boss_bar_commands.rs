use crisp_data::commands::CommandNode;

use super::list_players_command::format_list;
use crate::commands::CommandFn;
use crate::commands::brigadier::{CommandContext, CommandSyntaxException, DynamicCommandExceptionType, SimpleCommandExceptionType, tr};
use crate::commands::source::CommandSourceStack;
use crate::network::chat::{Arg, Component, component_utils};
use crate::server::Server;
use crate::server::bossevents::custom_boss_event::CustomBossEvent;
use crate::server::bossevents::custom_boss_events::CustomBossEvents;
use crate::server::level::server_boss_event::{BossBarColor, BossBarOverlay};

static ERROR_ALREADY_EXISTS: DynamicCommandExceptionType = DynamicCommandExceptionType(|id| tr("commands.bossbar.create.failed", vec![id]));
static ERROR_DOESNT_EXIST: DynamicCommandExceptionType = DynamicCommandExceptionType(|id| tr("commands.bossbar.unknown", vec![id]));
static ERROR_NO_PLAYER_CHANGE: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.bossbar.set.players.unchanged");
static ERROR_NO_NAME_CHANGE: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.bossbar.set.name.unchanged");
static ERROR_NO_COLOR_CHANGE: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.bossbar.set.color.unchanged");
static ERROR_NO_STYLE_CHANGE: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.bossbar.set.style.unchanged");
static ERROR_NO_VALUE_CHANGE: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.bossbar.set.value.unchanged");
static ERROR_NO_MAX_CHANGE: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.bossbar.set.max.unchanged");
static ERROR_ALREADY_HIDDEN: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.bossbar.set.visibility.unchanged.hidden");
static ERROR_ALREADY_VISIBLE: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.bossbar.set.visibility.unchanged.visible");

type Result<T> = std::result::Result<T, CommandSyntaxException>;

pub fn executors() -> Vec<(String, CommandFn)> {
    let mut out: Vec<(String, CommandFn)> = Vec::new();
    for color in BossBarColor::VALUES {
        out.push((format!("bossbar set id color {}", color.name()), set_color));
    }
    for style in BossBarOverlay::VALUES {
        out.push((format!("bossbar set id style {}", style.name()), set_style));
    }
    out
}

fn get_boss_bar<'e>(events: &'e mut CustomBossEvents, context: &CommandContext) -> Result<&'e mut CustomBossEvent> {
    let id = context.get_identifier("id");
    events.get_mut(id).ok_or_else(|| ERROR_DOESNT_EXIST.create(id))
}

fn success(server: &Server, source: &CommandSourceStack, context: &CommandContext, key: &str, mut args: Vec<Arg>) {
    let name = server.custom_boss_events.get(context.get_identifier("id")).map_or_else(Component::empty, CustomBossEvent::display_name);
    args.insert(0, name.into());
    source.send_success(server, || Component::translatable_with(key, args), true);
}

pub fn get_value(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    let value = get_boss_bar(&mut server.custom_boss_events, context)?.value;
    success(server, source, context, "commands.bossbar.get.value", vec![value.into()]);
    Ok(value)
}

pub fn get_max(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    let max = get_boss_bar(&mut server.custom_boss_events, context)?.max;
    success(server, source, context, "commands.bossbar.get.max", vec![max.into()]);
    Ok(max)
}

pub fn get_visible(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    let visible = get_boss_bar(&mut server.custom_boss_events, context)?.event.visible;
    let key = if visible { "commands.bossbar.get.visible.visible" } else { "commands.bossbar.get.visible.hidden" };
    success(server, source, context, key, Vec::new());
    Ok(visible as i32)
}

pub fn get_players(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    let players = get_boss_bar(&mut server.custom_boss_events, context)?.event.players.clone();
    if players.is_empty() {
        success(server, source, context, "commands.bossbar.get.players.none", Vec::new());
    } else {
        let names = format_list(players.iter().map(|&p| server.entity_display_name(p)).collect());
        success(server, source, context, "commands.bossbar.get.players.some", vec![(players.len() as i32).into(), names.into()]);
    }
    Ok(players.len() as i32)
}

pub fn set_visible(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    let visible = context.get_bool("visible");
    let bar = get_boss_bar(&mut server.custom_boss_events, context)?;
    if bar.event.visible == visible {
        return Err(if visible { ERROR_ALREADY_VISIBLE.create() } else { ERROR_ALREADY_HIDDEN.create() });
    }
    bar.event.set_visible(&server.levels, visible);
    bar.dirty = true;
    let key = if visible { "commands.bossbar.set.visible.success.visible" } else { "commands.bossbar.set.visible.success.hidden" };
    success(server, source, context, key, Vec::new());
    Ok(0)
}

pub fn set_value(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    let value = context.get_int("value");
    let bar = get_boss_bar(&mut server.custom_boss_events, context)?;
    if bar.value == value {
        return Err(ERROR_NO_VALUE_CHANGE.create());
    }
    bar.set_value(&server.levels, value);
    success(server, source, context, "commands.bossbar.set.value.success", vec![value.into()]);
    Ok(value)
}

pub fn set_max(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    let max = context.get_int("max");
    let bar = get_boss_bar(&mut server.custom_boss_events, context)?;
    if bar.max == max {
        return Err(ERROR_NO_MAX_CHANGE.create());
    }
    bar.set_max(&server.levels, max);
    success(server, source, context, "commands.bossbar.set.max.success", vec![max.into()]);
    Ok(max)
}

fn literal(context: &CommandContext) -> &'static str {
    CommandNode::get(context.nodes[4].node).name
}

pub fn set_color(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    let color = BossBarColor::by_name(literal(context)).expect("a color literal");
    let bar = get_boss_bar(&mut server.custom_boss_events, context)?;
    if bar.event.color == color {
        return Err(ERROR_NO_COLOR_CHANGE.create());
    }
    bar.event.set_color(&server.levels, color);
    bar.dirty = true;
    success(server, source, context, "commands.bossbar.set.color.success", Vec::new());
    Ok(0)
}

pub fn set_style(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    let style = BossBarOverlay::by_name(literal(context)).expect("a style literal");
    let bar = get_boss_bar(&mut server.custom_boss_events, context)?;
    if bar.event.overlay == style {
        return Err(ERROR_NO_STYLE_CHANGE.create());
    }
    bar.event.set_overlay(&server.levels, style);
    bar.dirty = true;
    success(server, source, context, "commands.bossbar.set.style.success", Vec::new());
    Ok(0)
}

fn resolved_name(server: &Server, context: &CommandContext, source: &CommandSourceStack) -> Result<Component> {
    let name = context.get_resolved_component(server, source, "name")?;
    component_utils::update_for_entity(server, source, &name, None)
}

pub fn set_name(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    get_boss_bar(&mut server.custom_boss_events, context)?;
    let name = resolved_name(server, context, source)?;
    let bar = get_boss_bar(&mut server.custom_boss_events, context)?;
    if bar.event.name == name {
        return Err(ERROR_NO_NAME_CHANGE.create());
    }
    bar.event.set_name(&server.levels, name);
    bar.dirty = true;
    success(server, source, context, "commands.bossbar.set.name.success", Vec::new());
    Ok(0)
}

pub fn set_players(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    get_boss_bar(&mut server.custom_boss_events, context)?;
    let targets = if context.has_argument("targets") { context.get_optional_players(server, source, "targets")? } else { Vec::new() };
    let bar = get_boss_bar(&mut server.custom_boss_events, context)?;
    if !bar.set_players(&server.levels, &targets) {
        return Err(ERROR_NO_PLAYER_CHANGE.create());
    }
    let count = bar.event.players.len() as i32;
    if count == 0 {
        success(server, source, context, "commands.bossbar.set.players.success.none", Vec::new());
    } else {
        let names = format_list(targets.iter().map(|&p| server.entity_display_name(p)).collect());
        success(server, source, context, "commands.bossbar.set.players.success.some", vec![(targets.len() as i32).into(), names.into()]);
    }
    Ok(count)
}

pub fn list_bars(server: &mut Server, _context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    let events = &server.custom_boss_events;
    let count = events.len() as i32;
    if count == 0 {
        source.send_success(server, || Component::translatable("commands.bossbar.list.bars.none"), false);
    } else {
        let names = format_list(events.events().map(CustomBossEvent::display_name).collect());
        source.send_success(server, || Component::translatable_with("commands.bossbar.list.bars.some", vec![count.into(), names.clone().into()]), false);
    }
    Ok(count)
}

pub fn create_bar(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    let id = context.get_identifier("id");
    let name = context.get_resolved_component(server, source, "name")?;
    if server.custom_boss_events.get(id).is_some() {
        return Err(ERROR_ALREADY_EXISTS.create(id));
    }
    let name = component_utils::update_for_entity(server, source, &name, None)?;
    server.custom_boss_events.create(&mut server.levels[source.level].random, id, name);
    success(server, source, context, "commands.bossbar.create.success", Vec::new());
    Ok(server.custom_boss_events.len() as i32)
}

pub fn remove_bar(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    let bar = get_boss_bar(&mut server.custom_boss_events, context)?;
    bar.remove_all_players(&server.levels);
    let name = bar.display_name();
    server.custom_boss_events.remove(context.get_identifier("id"));
    source.send_success(server, || Component::translatable_with("commands.bossbar.remove.success", vec![name.into()]), true);
    Ok(server.custom_boss_events.len() as i32)
}

pub fn suggest_boss_bar(
    server: &Server,
    _context: &CommandContext,
    _source: &CommandSourceStack,
    mut builder: crate::commands::suggestion::SuggestionsBuilder,
) -> std::result::Result<crate::commands::suggestion::Suggestions, CommandSyntaxException> {
    crate::commands::shared_suggestion_provider::suggest_resource(server.custom_boss_events.events().map(|e| e.custom_id.as_str()), &mut builder);
    Ok(builder.build())
}
