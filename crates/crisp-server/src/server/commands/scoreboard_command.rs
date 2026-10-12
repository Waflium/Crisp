use crisp_data::commands::CommandNode;

use super::command_response_tracker::{CommandResponseTracker, ElementType};
use super::list_players_command::format_list;
use crate::commands::CommandFn;
use crate::commands::brigadier::{CommandContext, CommandSyntaxException, Dynamic2CommandExceptionType, SimpleCommandExceptionType, tr};
use crate::commands::source::CommandSourceStack;
use crate::network::chat::numbers::NumberFormat;
use crate::network::chat::{Arg, Component};
use crate::server::Server;
use crate::world::scores::criteria::{ObjectiveCriteria, RenderType};
use crate::world::scores::objective::ObjectiveId;
use crate::world::scores::score_holder::ScoreHolder;

static ERROR_OBJECTIVE_ALREADY_EXISTS: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.scoreboard.objectives.add.duplicate");
static ERROR_DISPLAY_SLOT_ALREADY_EMPTY: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.scoreboard.objectives.display.alreadyEmpty");
static ERROR_DISPLAY_SLOT_ALREADY_SET: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.scoreboard.objectives.display.alreadySet");
static ERROR_TRIGGER_ALREADY_ENABLED: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.scoreboard.players.enable.failed");
static ERROR_NOT_TRIGGER: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.scoreboard.players.enable.invalid");
static ERROR_NO_VALUE: Dynamic2CommandExceptionType = Dynamic2CommandExceptionType(|objective, target| tr("commands.scoreboard.players.get.null", vec![objective, target]));

type Result = std::result::Result<i32, CommandSyntaxException>;

pub fn executors() -> Vec<(String, CommandFn)> {
    let mut out: Vec<(String, CommandFn)> = vec![
        ("scoreboard objectives list".into(), list_objectives),
        ("scoreboard objectives add objective criteria".into(), add_objective),
        ("scoreboard objectives add objective criteria displayName".into(), add_objective),
        ("scoreboard objectives modify objective displayname displayName".into(), set_display_name),
        ("scoreboard objectives modify objective displayautoupdate value".into(), set_display_auto_update),
        ("scoreboard objectives remove objective".into(), remove_objective),
        ("scoreboard objectives setdisplay slot".into(), clear_display_slot),
        ("scoreboard objectives setdisplay slot objective".into(), set_display_slot),
        ("scoreboard players list".into(), list_tracked_players),
        ("scoreboard players list target".into(), list_tracked_player_scores),
        ("scoreboard players set targets objective score".into(), set_score),
        ("scoreboard players get target objective".into(), get_score),
        ("scoreboard players add targets objective score".into(), add_score),
        ("scoreboard players remove targets objective score".into(), remove_score),
        ("scoreboard players reset targets".into(), reset_scores),
        ("scoreboard players reset targets objective".into(), reset_score),
        ("scoreboard players enable targets objective".into(), enable_trigger),
        ("scoreboard players display name targets objective".into(), set_score_display),
        ("scoreboard players display name targets objective name".into(), set_score_display),
        ("scoreboard players operation targets targetObjective operation source sourceObjective".into(), perform_operation),
    ];
    for render_type in RenderType::VALUES {
        out.push((format!("scoreboard objectives modify objective rendertype {}", render_type.id()), set_render_type));
    }
    for (top, f) in [("scoreboard objectives modify objective numberformat", set_objective_format as CommandFn), ("scoreboard players display numberformat targets objective", set_score_number_format)] {
        out.push((top.into(), f));
        out.push((format!("{top} blank"), f));
        out.push((format!("{top} fixed contents"), f));
        out.push((format!("{top} styled style"), f));
    }
    out
}

fn last_node(context: &CommandContext) -> &'static str {
    CommandNode::get(context.nodes.last().unwrap().node).name
}

fn number_format(server: &Server, context: &CommandContext, source: &CommandSourceStack) -> std::result::Result<Option<NumberFormat>, CommandSyntaxException> {
    Ok(match last_node(context) {
        "blank" => Some(NumberFormat::Blank),
        "contents" => Some(NumberFormat::Fixed(context.get_resolved_component(server, source, "contents")?)),
        "style" => Some(NumberFormat::Styled(context.get_style("style").clone())),
        _ => None,
    })
}

fn formatted(server: &Server, objective: ObjectiveId) -> Component {
    server.scoreboard.borrow().objective(objective).formatted_display_name()
}

fn feedback_name(server: &Server, holder: &ScoreHolder) -> Component {
    holder.feedback_display_name(&server.levels[server.score_holder_level(holder)])
}

pub fn get_score(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result {
    let target = context.get_score_holder(server, source, "target")?;
    let objective = context.get_objective(server, "objective")?;
    let scoreboard = server.scoreboard.borrow();
    let Some(score) = scoreboard.get_player_score_info(target.scoreboard_name(), objective) else {
        return Err(ERROR_NO_VALUE.create(scoreboard.objective(objective).name(), feedback_name(server, &target)));
    };
    let value = score.value;
    source.send_success(
        server,
        || {
            Component::translatable_with(
                "commands.scoreboard.players.get.success",
                vec![feedback_name(server, &target).into(), Arg::Int(value), formatted(server, objective).into()],
            )
        },
        false,
    );
    Ok(value)
}

pub fn perform_operation(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result {
    let targets = context.get_score_holders(server, source, "targets")?;
    let target_objective = context.get_writable_objective(server, "targetObjective")?;
    let operation = context.get_operation("operation");
    let sources = context.get_score_holders(server, source, "source")?;
    let source_objective = context.get_objective(server, "sourceObjective")?;
    let mut tracker = CommandResponseTracker::new();
    for target in targets {
        let index = server.score_holder_level(&target);
        let level = &mut server.levels[index];
        let mut score = level.scoreboard.borrow_mut().get_or_create_player_score(&target, target_objective, false);
        for from in &sources {
            let mut source_score = level.scoreboard.borrow_mut().get_or_create_player_score(from, source_objective, false);
            apply_operation(level, operation, &mut score, &mut source_score)?;
        }
        let value = score.get(level);
        tracker.track_value(target, value);
    }
    tracker.send_feedback_of(
        server,
        source,
        true,
        ElementType::Any,
        None,
        |holder, total| {
            Component::translatable_with(
                "commands.scoreboard.players.operation.success.single",
                vec![formatted(server, target_objective).into(), feedback_name(server, &holder).into(), Arg::Int(total)],
            )
        },
        |count, _| Component::translatable_with("commands.scoreboard.players.operation.success.multiple", vec![formatted(server, target_objective).into(), Arg::Int(count)]),
    )
}

fn apply_operation(
    level: &mut crate::server::level::ServerLevel,
    operation: crate::commands::arguments::operation_argument::Operation,
    a: &mut crate::world::scores::scoreboard::ScoreAccess,
    b: &mut crate::world::scores::scoreboard::ScoreAccess,
) -> std::result::Result<(), CommandSyntaxException> {
    use crate::commands::arguments::operation_argument::Operation;
    if operation == Operation::Swap {
        let swap = a.get(level);
        let b_value = b.get(level);
        a.set(level, b_value).map_err(read_only)?;
        b.set(level, swap).map_err(read_only)?;
    } else {
        let value = operation.apply(a.get(level), b.get(level))?;
        a.set(level, value).map_err(read_only)?;
    }
    Ok(())
}

fn read_only(_: crate::world::scores::scoreboard::ReadOnlyScore) -> CommandSyntaxException {
    crate::commands::unexpected_error("Cannot modify read-only score")
}

pub fn enable_trigger(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result {
    let targets = context.get_score_holders(server, source, "targets")?;
    let objective = context.get_objective(server, "objective")?;
    if *server.scoreboard.borrow().objective(objective).criteria() != ObjectiveCriteria::TRIGGER {
        return Err(ERROR_NOT_TRIGGER.create());
    }
    let mut tracker = CommandResponseTracker::new();
    for target in targets {
        let index = server.score_holder_level(&target);
        let level = &mut server.levels[index];
        let mut score = level.scoreboard.borrow_mut().get_or_create_player_score(&target, objective, false);
        if score.locked(level) {
            score.unlock(level);
            tracker.track(target);
        }
    }
    tracker.send_feedback_of(
        server,
        source,
        true,
        ElementType::NonZero,
        Some(|| ERROR_TRIGGER_ALREADY_ENABLED.create()),
        |holder, _| Component::translatable_with("commands.scoreboard.players.enable.success.single", vec![formatted(server, objective).into(), feedback_name(server, &holder).into()]),
        |count, _| Component::translatable_with("commands.scoreboard.players.enable.success.multiple", vec![formatted(server, objective).into(), Arg::Int(count)]),
    )
}

pub fn reset_scores(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result {
    let targets = context.get_score_holders(server, source, "targets")?;
    let mut tracker = CommandResponseTracker::new();
    for target in targets {
        server.scoreboard.borrow_mut().reset_all_player_scores(target.scoreboard_name());
        tracker.track(target);
    }
    tracker.send_feedback(
        server,
        source,
        true,
        |holder, _| Component::translatable_with("commands.scoreboard.players.reset.all.single", vec![feedback_name(server, &holder).into()]),
        |count, _| Component::translatable_with("commands.scoreboard.players.reset.all.multiple", vec![Arg::Int(count)]),
    )
}

pub fn reset_score(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result {
    let targets = context.get_score_holders(server, source, "targets")?;
    let objective = context.get_objective(server, "objective")?;
    let mut tracker = CommandResponseTracker::new();
    for target in targets {
        server.scoreboard.borrow_mut().reset_single_player_score(target.scoreboard_name(), objective);
        tracker.track(target);
    }
    tracker.send_feedback(
        server,
        source,
        true,
        |holder, _| Component::translatable_with("commands.scoreboard.players.reset.specific.single", vec![formatted(server, objective).into(), feedback_name(server, &holder).into()]),
        |count, _| Component::translatable_with("commands.scoreboard.players.reset.specific.multiple", vec![formatted(server, objective).into(), Arg::Int(count)]),
    )
}

pub fn set_score(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result {
    let targets = context.get_score_holders(server, source, "targets")?;
    let objective = context.get_writable_objective(server, "objective")?;
    let value = context.get_int("score");
    let mut tracker = CommandResponseTracker::new();
    for target in targets {
        let index = server.score_holder_level(&target);
        let level = &mut server.levels[index];
        let mut score = level.scoreboard.borrow_mut().get_or_create_player_score(&target, objective, false);
        score.set(level, value).map_err(read_only)?;
        tracker.track_value(target, value);
    }
    tracker.send_feedback(
        server,
        source,
        true,
        |holder, _| {
            Component::translatable_with(
                "commands.scoreboard.players.set.success.single",
                vec![formatted(server, objective).into(), feedback_name(server, &holder).into(), Arg::Int(value)],
            )
        },
        |count, _| Component::translatable_with("commands.scoreboard.players.set.success.multiple", vec![formatted(server, objective).into(), Arg::Int(count), Arg::Int(value)]),
    )
}

pub fn set_score_display(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result {
    let targets = context.get_score_holders(server, source, "targets")?;
    let objective = context.get_objective(server, "objective")?;
    let display = if context.has_argument("name") { Some(context.get_resolved_component(server, source, "name")?) } else { None };
    let mut tracker = CommandResponseTracker::new();
    for target in targets {
        let index = server.score_holder_level(&target);
        let level = &mut server.levels[index];
        let mut score = level.scoreboard.borrow_mut().get_or_create_player_score(&target, objective, false);
        score.set_display(level, display.clone());
        tracker.track(target);
    }
    match display {
        None => tracker.send_feedback(
            server,
            source,
            true,
            |holder, _| {
                Component::translatable_with(
                    "commands.scoreboard.players.display.name.clear.success.single",
                    vec![feedback_name(server, &holder).into(), formatted(server, objective).into()],
                )
            },
            |count, _| Component::translatable_with("commands.scoreboard.players.display.name.clear.success.multiple", vec![Arg::Int(count), formatted(server, objective).into()]),
        ),
        Some(display) => {
            let multiple = display.clone();
            tracker.send_feedback(
                server,
                source,
                true,
                |holder, _| {
                    Component::translatable_with(
                        "commands.scoreboard.players.display.name.set.success.single",
                        vec![display.into(), feedback_name(server, &holder).into(), formatted(server, objective).into()],
                    )
                },
                |count, _| {
                    Component::translatable_with(
                        "commands.scoreboard.players.display.name.set.success.multiple",
                        vec![multiple.into(), Arg::Int(count), formatted(server, objective).into()],
                    )
                },
            )
        }
    }
}

pub fn set_score_number_format(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result {
    let targets = context.get_score_holders(server, source, "targets")?;
    let objective = context.get_objective(server, "objective")?;
    let format = number_format(server, context, source)?;
    let set = format.is_some();
    let mut tracker = CommandResponseTracker::new();
    for target in targets {
        let index = server.score_holder_level(&target);
        let level = &mut server.levels[index];
        let mut score = level.scoreboard.borrow_mut().get_or_create_player_score(&target, objective, false);
        score.number_format_override(level, format.clone());
        tracker.track(target);
    }
    let key = if set { "commands.scoreboard.players.display.numberFormat.set.success" } else { "commands.scoreboard.players.display.numberFormat.clear.success" };
    tracker.send_feedback(
        server,
        source,
        true,
        |holder, _| Component::translatable_with(&format!("{key}.single"), vec![feedback_name(server, &holder).into(), formatted(server, objective).into()]),
        |count, _| Component::translatable_with(&format!("{key}.multiple"), vec![Arg::Int(count), formatted(server, objective).into()]),
    )
}

fn change_score(server: &mut Server, context: &CommandContext, source: &CommandSourceStack, sign: i32, key: &str) -> Result {
    let targets = context.get_score_holders(server, source, "targets")?;
    let objective = context.get_writable_objective(server, "objective")?;
    let value = context.get_int("score");
    let mut tracker = CommandResponseTracker::new();
    for target in targets {
        let index = server.score_holder_level(&target);
        let level = &mut server.levels[index];
        let mut score = level.scoreboard.borrow_mut().get_or_create_player_score(&target, objective, false);
        let current = score.get(level);
        score.set(level, current.wrapping_add(sign.wrapping_mul(value))).map_err(read_only)?;
        let new = score.get(level);
        tracker.track_value(target, new);
    }
    tracker.send_feedback_of(
        server,
        source,
        true,
        ElementType::Any,
        None,
        |holder, total| {
            Component::translatable_with(
                &format!("{key}.single"),
                vec![Arg::Int(value), formatted(server, objective).into(), feedback_name(server, &holder).into(), Arg::Int(total)],
            )
        },
        |count, _| Component::translatable_with(&format!("{key}.multiple"), vec![Arg::Int(value), formatted(server, objective).into(), Arg::Int(count)]),
    )
}

pub fn add_score(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result {
    change_score(server, context, source, 1, "commands.scoreboard.players.add.success")
}

pub fn remove_score(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result {
    change_score(server, context, source, -1, "commands.scoreboard.players.remove.success")
}

pub fn list_tracked_players(server: &mut Server, _context: &CommandContext, source: &CommandSourceStack) -> Result {
    let entities = server.scoreboard.borrow().tracked_players();
    let count = entities.len() as i32;
    if entities.is_empty() {
        source.send_success(server, || Component::translatable("commands.scoreboard.players.list.empty"), false);
    } else {
        source.send_success(
            server,
            || {
                let names = format_list(entities.iter().map(|e| feedback_name(server, e)).collect());
                Component::translatable_with("commands.scoreboard.players.list.success", vec![Arg::Int(count), names.into()])
            },
            false,
        );
    }
    Ok(count)
}

pub fn list_tracked_player_scores(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result {
    let entity = context.get_score_holder(server, source, "target")?;
    let scores = server.scoreboard.borrow().list_player_scores_of(entity.scoreboard_name());
    let count = scores.len() as i32;
    if scores.is_empty() {
        source.send_success(server, || Component::translatable_with("commands.scoreboard.players.list.entity.empty", vec![feedback_name(server, &entity).into()]), false);
    } else {
        source.send_success(
            server,
            || Component::translatable_with("commands.scoreboard.players.list.entity.success", vec![feedback_name(server, &entity).into(), Arg::Int(count)]),
            false,
        );
        for (objective, value) in scores {
            source.send_success(server, || Component::translatable_with("commands.scoreboard.players.list.entity.entry", vec![formatted(server, objective).into(), Arg::Int(value)]), false);
        }
    }
    Ok(count)
}

pub fn clear_display_slot(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result {
    let slot = context.get_display_slot("slot");
    if server.scoreboard.borrow().get_display_objective(slot).is_none() {
        return Err(ERROR_DISPLAY_SLOT_ALREADY_EMPTY.create());
    }
    server.scoreboard.borrow_mut().set_display_objective(slot, None);
    source.send_success(server, || Component::translatable_with("commands.scoreboard.objectives.display.cleared", vec![slot.serialized_name().into()]), true);
    Ok(0)
}

pub fn set_display_slot(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result {
    let slot = context.get_display_slot("slot");
    let objective = context.get_objective(server, "objective")?;
    if server.scoreboard.borrow().get_display_objective(slot) == Some(objective) {
        return Err(ERROR_DISPLAY_SLOT_ALREADY_SET.create());
    }
    server.scoreboard.borrow_mut().set_display_objective(slot, Some(objective));
    source.send_success(
        server,
        || {
            let display_name = server.scoreboard.borrow().objective(objective).display_name().clone();
            Component::translatable_with("commands.scoreboard.objectives.display.set", vec![slot.serialized_name().into(), display_name.into()])
        },
        true,
    );
    Ok(0)
}

pub fn set_display_name(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result {
    let objective = context.get_objective(server, "objective")?;
    let display_name = context.get_resolved_component(server, source, "displayName")?;
    if *server.scoreboard.borrow().objective(objective).display_name() != display_name {
        server.scoreboard.borrow_mut().set_objective_display_name(objective, display_name);
        let name = server.scoreboard.borrow().objective(objective).name().to_string();
        source.send_success(server, || Component::translatable_with("commands.scoreboard.objectives.modify.displayname", vec![name.into(), formatted(server, objective).into()]), true);
    }
    Ok(0)
}

pub fn set_display_auto_update(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result {
    let objective = context.get_objective(server, "objective")?;
    let display_auto_update = context.get_bool("value");
    if server.scoreboard.borrow().objective(objective).display_auto_update() != display_auto_update {
        server.scoreboard.borrow_mut().set_display_auto_update(objective, display_auto_update);
        let name = server.scoreboard.borrow().objective(objective).name().to_string();
        let key = if display_auto_update { "commands.scoreboard.objectives.modify.displayAutoUpdate.enable" } else { "commands.scoreboard.objectives.modify.displayAutoUpdate.disable" };
        source.send_success(server, || Component::translatable_with(key, vec![name.into(), formatted(server, objective).into()]), true);
    }
    Ok(0)
}

pub fn set_objective_format(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result {
    let objective = context.get_objective(server, "objective")?;
    let format = number_format(server, context, source)?;
    let set = format.is_some();
    server.scoreboard.borrow_mut().set_number_format(objective, format);
    let name = server.scoreboard.borrow().objective(objective).name().to_string();
    let key = if set { "commands.scoreboard.objectives.modify.objectiveFormat.set" } else { "commands.scoreboard.objectives.modify.objectiveFormat.clear" };
    source.send_success(server, || Component::translatable_with(key, vec![name.into()]), true);
    Ok(0)
}

pub fn set_render_type(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result {
    let objective = context.get_objective(server, "objective")?;
    let render_type = RenderType::by_name(last_node(context)).expect("a render type literal");
    if server.scoreboard.borrow().objective(objective).render_type() != render_type {
        server.scoreboard.borrow_mut().set_render_type(objective, render_type);
        source.send_success(server, || Component::translatable_with("commands.scoreboard.objectives.modify.rendertype", vec![formatted(server, objective).into()]), true);
    }
    Ok(0)
}

pub fn remove_objective(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result {
    let objective = context.get_objective(server, "objective")?;
    let display = formatted(server, objective);
    server.scoreboard.borrow_mut().remove_objective(objective);
    source.send_success(server, || Component::translatable_with("commands.scoreboard.objectives.remove.success", vec![display.into()]), true);
    Ok(server.scoreboard.borrow().objective_count() as i32)
}

pub fn add_objective(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result {
    let name = context.get_string("objective").to_string();
    let criteria = context.get_criteria("criteria").clone();
    let display_name = if context.has_argument("displayName") { context.get_resolved_component(server, source, "displayName")? } else { Component::literal(name.clone()) };
    let objective = {
        let mut scoreboard = server.scoreboard.borrow_mut();
        if scoreboard.get_objective(&name).is_some() {
            return Err(ERROR_OBJECTIVE_ALREADY_EXISTS.create());
        }
        let render_type = criteria.default_render_type();
        scoreboard.add_objective(&name, criteria, display_name, render_type, false, None)
    };
    source.send_success(server, || Component::translatable_with("commands.scoreboard.objectives.add.success", vec![formatted(server, objective).into()]), true);
    Ok(server.scoreboard.borrow().objective_count() as i32)
}

pub fn list_objectives(server: &mut Server, _context: &CommandContext, source: &CommandSourceStack) -> Result {
    let scoreboard = server.scoreboard.borrow();
    let count = scoreboard.objective_count() as i32;
    if count == 0 {
        source.send_success(server, || Component::translatable("commands.scoreboard.objectives.list.empty"), false);
    } else {
        source.send_success(
            server,
            || {
                let names = format_list(scoreboard.objectives().map(|o| o.formatted_display_name()).collect());
                Component::translatable_with("commands.scoreboard.objectives.list.success", vec![Arg::Int(count), names.into()])
            },
            false,
        );
    }
    Ok(count)
}

pub fn suggest_triggers(
    server: &Server,
    context: &CommandContext,
    source: &CommandSourceStack,
    mut builder: crate::commands::suggestion::SuggestionsBuilder,
) -> std::result::Result<crate::commands::suggestion::Suggestions, CommandSyntaxException> {
    let targets = context.get_score_holders(server, source, "targets")?;
    let scoreboard = server.scoreboard.borrow();
    let mut result = Vec::new();
    for objective in scoreboard.objectives() {
        if *objective.criteria() == ObjectiveCriteria::TRIGGER
            && targets.iter().any(|name| scoreboard.get_player_score_info(name.scoreboard_name(), objective.id()).is_none_or(|score| score.locked))
        {
            result.push(objective.name().to_string());
        }
    }
    crate::commands::shared_suggestion_provider::suggest(result, &mut builder);
    Ok(builder.build())
}
