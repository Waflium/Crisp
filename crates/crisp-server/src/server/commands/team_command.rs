use crisp_data::commands::CommandNode;

use super::command_response_tracker::CommandResponseTracker;
use super::list_players_command::format_list;
use crate::commands::CommandFn;
use crate::commands::brigadier::{CommandContext, CommandSyntaxException, SimpleCommandExceptionType};
use crate::commands::source::CommandSourceStack;
use crate::network::chat::{Arg, ChatFormatting, Component};
use crate::server::Server;
use crate::world::scores::score_holder::ScoreHolder;
use crate::world::scores::team::{CollisionRule, Visibility};

static ERROR_TEAM_ALREADY_EXISTS: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.team.add.duplicate");
static ERROR_TEAM_ALREADY_EMPTY: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.team.empty.unchanged");
static ERROR_TEAM_ALREADY_NAME: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.team.option.name.unchanged");
static ERROR_TEAM_ALREADY_COLOR: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.team.option.color.unchanged");
static ERROR_TEAM_ALREADY_FRIENDLYFIRE_ENABLED: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.team.option.friendlyfire.alreadyEnabled");
static ERROR_TEAM_ALREADY_FRIENDLYFIRE_DISABLED: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.team.option.friendlyfire.alreadyDisabled");
static ERROR_TEAM_ALREADY_FRIENDLYINVISIBLES_ENABLED: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.team.option.seeFriendlyInvisibles.alreadyEnabled");
static ERROR_TEAM_ALREADY_FRIENDLYINVISIBLES_DISABLED: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.team.option.seeFriendlyInvisibles.alreadyDisabled");
static ERROR_TEAM_NAMETAG_VISIBLITY_UNCHANGED: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.team.option.nametagVisibility.unchanged");
static ERROR_TEAM_DEATH_MESSAGE_VISIBLITY_UNCHANGED: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.team.option.deathMessageVisibility.unchanged");
static ERROR_TEAM_COLLISION_UNCHANGED: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.team.option.collisionRule.unchanged");

type Result = std::result::Result<i32, CommandSyntaxException>;

pub fn executors() -> Vec<(String, CommandFn)> {
    let mut out: Vec<(String, CommandFn)> = vec![
        ("team list".into(), list_teams),
        ("team list team".into(), list_members),
        ("team add team".into(), create_team),
        ("team add team displayName".into(), create_team),
        ("team remove team".into(), delete_team),
        ("team empty team".into(), empty_team),
        ("team join team".into(), join_team),
        ("team join team members".into(), join_team),
        ("team leave members".into(), leave_team),
        ("team modify team displayName displayName".into(), set_display_name),
        ("team modify team color reset".into(), clear_color),
        ("team modify team color value".into(), set_color),
        ("team modify team friendlyFire allowed".into(), set_friendly_fire),
        ("team modify team seeFriendlyInvisibles allowed".into(), set_friendly_sight),
        ("team modify team prefix prefix".into(), set_prefix),
        ("team modify team suffix suffix".into(), set_suffix),
    ];
    for visibility in Visibility::VALUES {
        out.push((format!("team modify team nametagVisibility {}", visibility.name()), set_name_tag_visibility));
        out.push((format!("team modify team deathMessageVisibility {}", visibility.name()), set_death_message_visibility));
    }
    for rule in CollisionRule::VALUES {
        out.push((format!("team modify team collisionRule {}", rule.name()), set_collision));
    }
    out
}

fn last_node(context: &CommandContext) -> &'static str {
    CommandNode::get(context.nodes.last().unwrap().node).name
}

fn formatted(server: &Server, team: &str) -> Component {
    server.scoreboard.borrow().get_player_team(team).expect("a team").formatted_display_name()
}

fn feedback_name(server: &Server, holder: &ScoreHolder) -> Component {
    holder.feedback_display_name(&server.levels[server.score_holder_level(holder)])
}

pub fn leave_team(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result {
    let members = context.get_score_holders(server, source, "members")?;
    let mut tracker = CommandResponseTracker::new();
    for member in members {
        let left = server.scoreboard.borrow_mut().remove_player_from_team_any(member.scoreboard_name());
        server.update_scoreboard_waypoints();
        tracker.track_value(member, left as i32);
    }
    tracker.send_feedback(
        server,
        source,
        true,
        |holder, _| Component::translatable_with("commands.team.leave.success.single", vec![feedback_name(server, &holder).into()]),
        |count, _| Component::translatable_with("commands.team.leave.success.multiple", vec![Arg::Int(count)]),
    )
}

pub fn join_team(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result {
    let team = context.get_team(server, "team")?;
    let members = if context.has_argument("members") {
        context.get_score_holders(server, source, "members")?
    } else {
        let entity = source.entity_or_exception()?;
        vec![ScoreHolder::for_entity(server.entity_level(entity), entity)]
    };
    let mut tracker = CommandResponseTracker::new();
    for member in members {
        let joined = server.scoreboard.borrow_mut().add_player_to_team(member.scoreboard_name(), &team);
        server.update_scoreboard_waypoints();
        tracker.track_value(member, joined as i32);
    }
    tracker.send_feedback(
        server,
        source,
        true,
        |holder, _| Component::translatable_with("commands.team.join.success.single", vec![feedback_name(server, &holder).into(), formatted(server, &team).into()]),
        |count, _| Component::translatable_with("commands.team.join.success.multiple", vec![Arg::Int(count), formatted(server, &team).into()]),
    )
}

pub fn set_name_tag_visibility(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result {
    let team = context.get_team(server, "team")?;
    let visibility = Visibility::by_name(last_node(context)).expect("a visibility literal");
    if server.scoreboard.borrow().get_player_team(&team).unwrap().name_tag_visibility() == visibility {
        return Err(ERROR_TEAM_NAMETAG_VISIBLITY_UNCHANGED.create());
    }
    server.scoreboard.borrow_mut().set_name_tag_visibility(&team, visibility);
    server.update_scoreboard_waypoints();
    source.send_success(
        server,
        || Component::translatable_with("commands.team.option.nametagVisibility.success", vec![formatted(server, &team).into(), visibility.display_name().into()]),
        true,
    );
    Ok(0)
}

pub fn set_death_message_visibility(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result {
    let team = context.get_team(server, "team")?;
    let visibility = Visibility::by_name(last_node(context)).expect("a visibility literal");
    if server.scoreboard.borrow().get_player_team(&team).unwrap().death_message_visibility() == visibility {
        return Err(ERROR_TEAM_DEATH_MESSAGE_VISIBLITY_UNCHANGED.create());
    }
    server.scoreboard.borrow_mut().set_death_message_visibility(&team, visibility);
    server.update_scoreboard_waypoints();
    source.send_success(
        server,
        || Component::translatable_with("commands.team.option.deathMessageVisibility.success", vec![formatted(server, &team).into(), visibility.display_name().into()]),
        true,
    );
    Ok(0)
}

pub fn set_collision(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result {
    let team = context.get_team(server, "team")?;
    let rule = CollisionRule::by_name(last_node(context)).expect("a collision rule literal");
    if server.scoreboard.borrow().get_player_team(&team).unwrap().collision_rule() == rule {
        return Err(ERROR_TEAM_COLLISION_UNCHANGED.create());
    }
    server.scoreboard.borrow_mut().set_collision_rule(&team, rule);
    server.update_scoreboard_waypoints();
    source.send_success(server, || Component::translatable_with("commands.team.option.collisionRule.success", vec![formatted(server, &team).into(), rule.display_name().into()]), true);
    Ok(0)
}

pub fn set_friendly_sight(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result {
    let team = context.get_team(server, "team")?;
    let allowed = context.get_bool("allowed");
    if server.scoreboard.borrow().get_player_team(&team).unwrap().can_see_friendly_invisibles() == allowed {
        return Err(if allowed { ERROR_TEAM_ALREADY_FRIENDLYINVISIBLES_ENABLED.create() } else { ERROR_TEAM_ALREADY_FRIENDLYINVISIBLES_DISABLED.create() });
    }
    server.scoreboard.borrow_mut().set_see_friendly_invisibles(&team, allowed);
    server.update_scoreboard_waypoints();
    let key = if allowed { "commands.team.option.seeFriendlyInvisibles.enabled" } else { "commands.team.option.seeFriendlyInvisibles.disabled" };
    source.send_success(server, || Component::translatable_with(key, vec![formatted(server, &team).into()]), true);
    Ok(0)
}

pub fn set_friendly_fire(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result {
    let team = context.get_team(server, "team")?;
    let allowed = context.get_bool("allowed");
    if server.scoreboard.borrow().get_player_team(&team).unwrap().is_allow_friendly_fire() == allowed {
        return Err(if allowed { ERROR_TEAM_ALREADY_FRIENDLYFIRE_ENABLED.create() } else { ERROR_TEAM_ALREADY_FRIENDLYFIRE_DISABLED.create() });
    }
    server.scoreboard.borrow_mut().set_allow_friendly_fire(&team, allowed);
    server.update_scoreboard_waypoints();
    let key = if allowed { "commands.team.option.friendlyfire.enabled" } else { "commands.team.option.friendlyfire.disabled" };
    source.send_success(server, || Component::translatable_with(key, vec![formatted(server, &team).into()]), true);
    Ok(0)
}

pub fn set_display_name(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result {
    let team = context.get_team(server, "team")?;
    let display_name = context.get_resolved_component(server, source, "displayName")?;
    if *server.scoreboard.borrow().get_player_team(&team).unwrap().display_name() == display_name {
        return Err(ERROR_TEAM_ALREADY_NAME.create());
    }
    server.scoreboard.borrow_mut().set_team_display_name(&team, display_name);
    server.update_scoreboard_waypoints();
    source.send_success(server, || Component::translatable_with("commands.team.option.name.success", vec![formatted(server, &team).into()]), true);
    Ok(0)
}

pub fn set_color(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result {
    let team = context.get_team(server, "team")?;
    let color = context.get_team_color("value");
    if server.scoreboard.borrow().get_player_team(&team).unwrap().color() == Some(color) {
        return Err(ERROR_TEAM_ALREADY_COLOR.create());
    }
    server.scoreboard.borrow_mut().set_color(&team, Some(color));
    server.update_scoreboard_waypoints();
    source.send_success(server, || Component::translatable_with("commands.team.option.color.success", vec![formatted(server, &team).into(), color.serialized_name().into()]), true);
    Ok(0)
}

pub fn clear_color(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result {
    let team = context.get_team(server, "team")?;
    if server.scoreboard.borrow().get_player_team(&team).unwrap().color().is_none() {
        return Err(ERROR_TEAM_ALREADY_COLOR.create());
    }
    server.scoreboard.borrow_mut().set_color(&team, None);
    server.update_scoreboard_waypoints();
    source.send_success(server, || Component::translatable_with("commands.team.option.color.clear.success", vec![formatted(server, &team).into()]), true);
    Ok(0)
}

pub fn empty_team(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result {
    let team = context.get_team(server, "team")?;
    let members: Vec<String> = server.scoreboard.borrow().get_player_team(&team).unwrap().players().map(String::from).collect();
    if members.is_empty() {
        return Err(ERROR_TEAM_ALREADY_EMPTY.create());
    }
    for member in &members {
        server.scoreboard.borrow_mut().remove_player_from_team(member, &team);
        server.update_scoreboard_waypoints();
    }
    let count = members.len() as i32;
    source.send_success(server, || Component::translatable_with("commands.team.empty.success", vec![Arg::Int(count), formatted(server, &team).into()]), true);
    Ok(count)
}

pub fn delete_team(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result {
    let team = context.get_team(server, "team")?;
    let display = formatted(server, &team);
    server.scoreboard.borrow_mut().remove_player_team(&team);
    server.update_scoreboard_waypoints();
    source.send_success(server, || Component::translatable_with("commands.team.remove.success", vec![display.into()]), true);
    Ok(server.scoreboard.borrow().team_count() as i32)
}

pub fn create_team(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result {
    let name = context.get_string("team").to_string();
    let display_name = if context.has_argument("displayName") { context.get_resolved_component(server, source, "displayName")? } else { Component::literal(name.clone()) };
    {
        let mut scoreboard = server.scoreboard.borrow_mut();
        if scoreboard.get_player_team(&name).is_some() {
            return Err(ERROR_TEAM_ALREADY_EXISTS.create());
        }
        scoreboard.add_player_team(&name);
        scoreboard.set_team_display_name(&name, display_name);
    }
    server.update_scoreboard_waypoints();
    source.send_success(server, || Component::translatable_with("commands.team.add.success", vec![formatted(server, &name).into()]), true);
    Ok(server.scoreboard.borrow().team_count() as i32)
}

pub fn list_members(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result {
    let team = context.get_team(server, "team")?;
    let mut members: Vec<String> = server.scoreboard.borrow().get_player_team(&team).unwrap().players().map(String::from).collect();
    let count = members.len() as i32;
    if members.is_empty() {
        source.send_success(server, || Component::translatable_with("commands.team.list.members.empty", vec![formatted(server, &team).into()]), false);
    } else {
        source.send_success(
            server,
            || {
                if members.len() > 1 {
                    members.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
                }
                let names = format_list(members.into_iter().map(|m| Component::literal(m).with_color(ChatFormatting::Green)).collect());
                Component::translatable_with("commands.team.list.members.success", vec![formatted(server, &team).into(), Arg::Int(count), names.into()])
            },
            false,
        );
    }
    Ok(count)
}

pub fn list_teams(server: &mut Server, _context: &CommandContext, source: &CommandSourceStack) -> Result {
    let scoreboard = server.scoreboard.borrow();
    let count = scoreboard.team_count() as i32;
    if count == 0 {
        source.send_success(server, || Component::translatable("commands.team.list.teams.empty"), false);
    } else {
        source.send_success(
            server,
            || {
                let names = format_list(scoreboard.player_teams().map(|t| t.formatted_display_name()).collect());
                Component::translatable_with("commands.team.list.teams.success", vec![Arg::Int(count), names.into()])
            },
            false,
        );
    }
    Ok(count)
}

pub fn set_prefix(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result {
    let team = context.get_team(server, "team")?;
    let prefix = context.get_resolved_component(server, source, "prefix")?;
    server.scoreboard.borrow_mut().set_player_prefix(&team, prefix.clone());
    server.update_scoreboard_waypoints();
    source.send_success(server, || Component::translatable_with("commands.team.option.prefix.success", vec![prefix.into()]), false);
    Ok(1)
}

pub fn set_suffix(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result {
    let team = context.get_team(server, "team")?;
    let suffix = context.get_resolved_component(server, source, "suffix")?;
    server.scoreboard.borrow_mut().set_player_suffix(&team, suffix.clone());
    server.update_scoreboard_waypoints();
    source.send_success(server, || Component::translatable_with("commands.team.option.suffix.success", vec![suffix.into()]), false);
    Ok(1)
}
