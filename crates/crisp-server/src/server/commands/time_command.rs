use crate::commands::brigadier::{CommandContext, CommandSyntaxException, Dynamic2CommandExceptionType, DynamicCommandExceptionType, tr};
use crate::commands::source::CommandSourceStack;
use crate::network::chat::{Arg, Component};
use crate::server::Server;
use crate::world::clock::{MoveResult, ServerClockManager, Timeline};

static ERROR_NO_DEFAULT_CLOCK: DynamicCommandExceptionType = DynamicCommandExceptionType(|d| tr("commands.time.no_default_clock", vec![d]));
static ERROR_NO_TIME_MARKER_FOUND: Dynamic2CommandExceptionType =
    Dynamic2CommandExceptionType(|clock, marker| tr("commands.time.no_time_marker_found", vec![marker, clock]));
static ERROR_WRONG_TIMELINE_FOR_CLOCK: Dynamic2CommandExceptionType =
    Dynamic2CommandExceptionType(|clock, timeline| tr("commands.time.wrong_timeline_for_clock", vec![timeline, clock]));
static ERROR_ALREADY_AT_TIME_MARKER: Dynamic2CommandExceptionType =
    Dynamic2CommandExceptionType(|clock, marker| tr("commands.time.set.already_at_time_marker", vec![clock, marker]));
static ERROR_ALREADY_AT_TIME: Dynamic2CommandExceptionType = Dynamic2CommandExceptionType(|clock, time| tr("commands.time.set.already_at_time", vec![clock, time]));
static ERROR_ALREADY_PAUSED: DynamicCommandExceptionType = DynamicCommandExceptionType(|clock| tr("commands.time.pause.already_paused", vec![clock]));
static ERROR_ALREADY_RUNNING: DynamicCommandExceptionType = DynamicCommandExceptionType(|clock| tr("commands.time.pause.already_running", vec![clock]));
static ERROR_ALREADY_SAME_RATE: Dynamic2CommandExceptionType = Dynamic2CommandExceptionType(|clock, rate| tr("commands.time.rate.already_same", vec![clock, rate]));

fn clock(server: &Server, context: &CommandContext, source: &CommandSourceStack) -> Result<(usize, &'static str), CommandSyntaxException> {
    let name = if context.has_argument("clock") {
        let id = context.get_identifier("clock");
        crisp_data::registry_entries("minecraft:world_clock").and_then(|e| e.iter().find(|&&n| n == id)).copied().expect("a parsed clock")
    } else {
        let dimension_type = server.levels[source.level].dimension_type();
        dimension_type.default_clock.ok_or_else(|| ERROR_NO_DEFAULT_CLOCK.create(dimension_type.name))?
    };
    Ok((ServerClockManager::id_of(name).expect("a known clock"), name))
}

fn wrap_time(ticks: i64) -> i32 {
    (ticks % i32::MAX as i64) as i32
}

pub fn query_game_time(server: &mut Server, _context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let game_time = server.levels[source.level].game_time;
    source.send_success(server, || Component::translatable_with("commands.time.query.gametime", vec![Arg::Long(game_time)]), false);
    Ok(wrap_time(game_time))
}

pub fn query_time(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let (clock, name) = clock(server, context, source)?;
    let total_ticks = server.clocks.clocks[clock].total_ticks;
    source.send_success(server, || Component::translatable_with("commands.time.query.absolute", vec![name.into(), Arg::Long(total_ticks)]), false);
    Ok(wrap_time(total_ticks))
}

fn timeline(context: &CommandContext, clock: usize, clock_name: &str) -> Result<&'static Timeline, CommandSyntaxException> {
    let id = context.get_identifier("timeline");
    let timeline = Timeline::get(id).expect("a parsed timeline");
    if timeline.clock != clock {
        return Err(ERROR_WRONG_TIMELINE_FOR_CLOCK.create(clock_name, id));
    }
    Ok(timeline)
}

pub fn query_timeline_ticks(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let (clock, name) = clock(server, context, source)?;
    let timeline = timeline(context, clock, name)?;
    let ticks = timeline.current_ticks(&server.clocks);
    source.send_success(server, || Component::translatable_with("commands.time.query.timeline", vec![timeline.id.into(), Arg::Long(ticks)]), false);
    Ok(wrap_time(ticks))
}

pub fn query_timeline_repetitions(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let (clock, name) = clock(server, context, source)?;
    let timeline = timeline(context, clock, name)?;
    let repetitions = timeline.period_count(&server.clocks) as i64;
    source.send_success(
        server,
        || Component::translatable_with("commands.time.query.timeline.repetitions", vec![timeline.id.into(), Arg::Long(repetitions)]),
        false,
    );
    Ok(wrap_time(repetitions))
}

pub fn set_total_ticks(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let (clock, name) = clock(server, context, source)?;
    let total_ticks = context.get_int("time");
    if server.clocks.clocks[clock].total_ticks == total_ticks as i64 {
        return Err(ERROR_ALREADY_AT_TIME.create(name, total_ticks));
    }
    server.clocks.set_total_ticks(clock, total_ticks as i64);
    server.modify_clock(clock);
    source.send_success(server, || Component::translatable_with("commands.time.set.absolute", vec![name.into(), Arg::Int(total_ticks)]), true);
    Ok(total_ticks)
}

pub fn add_time(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let (clock, name) = clock(server, context, source)?;
    server.clocks.add_ticks(clock, context.get_int("time"));
    server.modify_clock(clock);
    let total_ticks = server.clocks.clocks[clock].total_ticks;
    source.send_success(server, || Component::translatable_with("commands.time.set.absolute", vec![name.into(), Arg::Long(total_ticks)]), true);
    Ok(wrap_time(total_ticks))
}

pub fn set_time_to_time_marker(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let (clock, name) = clock(server, context, source)?;
    let marker = context.get_identifier("timemarker").to_string();
    let result = server.clocks.move_to_time_marker(clock, &marker);
    server.modify_clock(clock);
    match result {
        MoveResult::NoTimeMarkerFound => return Err(ERROR_NO_TIME_MARKER_FOUND.create(name, marker)),
        MoveResult::NotMoved => return Err(ERROR_ALREADY_AT_TIME_MARKER.create(name, marker)),
        MoveResult::Moved => {
            source.send_success(server, || Component::translatable_with("commands.time.set.time_marker", vec![name.into(), marker.as_str().into()]), true)
        }
    }
    Ok(wrap_time(server.clocks.clocks[clock].total_ticks))
}

fn set_paused(server: &mut Server, context: &CommandContext, source: &CommandSourceStack, paused: bool) -> Result<i32, CommandSyntaxException> {
    let (clock, name) = clock(server, context, source)?;
    if server.clocks.clocks[clock].paused == paused {
        return Err(if paused { ERROR_ALREADY_PAUSED.create(name) } else { ERROR_ALREADY_RUNNING.create(name) });
    }
    server.clocks.clocks[clock].paused = paused;
    server.modify_clock(clock);
    let key = if paused { "commands.time.pause" } else { "commands.time.resume" };
    source.send_success(server, || Component::translatable_with(key, vec![name.into()]), true);
    Ok(1)
}

pub fn pause(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    set_paused(server, context, source, true)
}

pub fn resume(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    set_paused(server, context, source, false)
}

pub fn set_rate(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let (clock, name) = clock(server, context, source)?;
    let rate = context.get_float("rate");
    if server.clocks.clocks[clock].rate == rate {
        return Err(ERROR_ALREADY_SAME_RATE.create(name, rate));
    }
    server.clocks.clocks[clock].rate = rate;
    server.modify_clock(clock);
    source.send_success(server, || Component::translatable_with("commands.time.rate", vec![name.into(), Arg::Float(rate)]), true);
    Ok(1)
}

pub fn suggest_time_markers(
    server: &Server,
    context: &CommandContext,
    source: &CommandSourceStack,
    mut builder: crate::commands::suggestion::SuggestionsBuilder,
) -> Result<crate::commands::suggestion::Suggestions, CommandSyntaxException> {
    let (clock, _) = clock(server, context, source)?;
    crate::commands::shared_suggestion_provider::suggest_resource(ServerClockManager::command_time_markers(clock), &mut builder);
    Ok(builder.build())
}

pub fn suggest_timelines(
    server: &Server,
    context: &CommandContext,
    source: &CommandSourceStack,
    mut builder: crate::commands::suggestion::SuggestionsBuilder,
) -> Result<crate::commands::suggestion::Suggestions, CommandSyntaxException> {
    let (clock, _) = clock(server, context, source)?;
    let timelines = crate::world::clock::TIMELINE_DATA.iter().filter(|t| t.clock == clock).map(|t| t.id);
    crate::commands::shared_suggestion_provider::suggest_resource(timelines, &mut builder);
    Ok(builder.build())
}
