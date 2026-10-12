use crate::commands::brigadier::{CommandContext, CommandSyntaxException, SimpleCommandExceptionType};
use crate::commands::source::CommandSourceStack;
use crate::network::chat::Component;
use crate::server::Server;
use crate::util::java_format_fixed;

static ERROR_TICK_STEP_NOT_FROZEN: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.tick.step.fail");
static ERROR_STEP_STOP_NOT_STEPPING: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.tick.step.stop.fail");
static ERROR_SPRINT_STOP_NOT_SPRINTING: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.tick.sprint.stop.fail");

type Result<T> = std::result::Result<T, CommandSyntaxException>;

fn nanos_to_millis_string(nanos: i64) -> String {
    java_format_fixed((nanos as f32 / 1_000_000.0) as f64, 1)
}

pub fn set_ticking_rate(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    let rate = context.get_float("rate");
    server.set_tick_rate(rate);
    let rate_string = java_format_fixed(rate as f64, 1);
    source.send_success(server, || Component::translatable_with("commands.tick.rate.success", vec![rate_string.into()]), true);
    Ok(rate as i32)
}

pub fn tick_query(server: &mut Server, _context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    let manager = &server.tick_rate_manager;
    let busy_time = nanos_to_millis_string(server.average_tick_time_nanos());
    let tick_rate = manager.tickrate();
    let rate_string = java_format_fixed(tick_rate as f64, 1);
    let mut messages = Vec::new();
    if server.tick_rate_manager.is_sprinting() {
        messages.push(Component::translatable("commands.tick.status.sprinting"));
        messages.push(Component::translatable_with("commands.tick.query.rate.sprinting", vec![rate_string.into(), busy_time.into()]));
    } else {
        messages.push(Component::translatable(if manager.is_frozen() {
            "commands.tick.status.frozen"
        } else if manager.nanoseconds_per_tick() < server.average_tick_time_nanos() {
            "commands.tick.status.lagging"
        } else {
            "commands.tick.status.running"
        }));
        let target = nanos_to_millis_string(manager.nanoseconds_per_tick());
        messages.push(Component::translatable_with("commands.tick.query.rate.running", vec![rate_string.into(), busy_time.into(), target.into()]));
    }
    let mut samples = *server.tick_times_nanos();
    samples.sort();
    let len = samples.len();
    let p50 = nanos_to_millis_string(samples[len / 2]);
    let p95 = nanos_to_millis_string(samples[(len as f64 * 0.95) as usize]);
    let p99 = nanos_to_millis_string(samples[(len as f64 * 0.99) as usize]);
    messages.push(Component::translatable_with("commands.tick.query.percentiles", vec![p50.into(), p95.into(), p99.into(), (len as i32).into()]));
    for message in messages {
        source.send_success(server, || message, false);
    }
    Ok(tick_rate as i32)
}

pub fn sprint(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    let interrupted = server.request_game_to_sprint(context.get_int("time"));
    if interrupted {
        source.send_success(server, || Component::translatable("commands.tick.sprint.stop.success"), true);
    }
    source.send_success(server, || Component::translatable("commands.tick.status.sprinting"), true);
    Ok(1)
}

pub fn freeze(server: &mut Server, _context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    set_freeze(server, source, true)
}

pub fn unfreeze(server: &mut Server, _context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    set_freeze(server, source, false)
}

fn set_freeze(server: &mut Server, source: &CommandSourceStack, freeze: bool) -> Result<i32> {
    if freeze {
        if server.tick_rate_manager.is_sprinting() {
            server.stop_sprinting();
        }
        if server.tick_rate_manager.is_stepping_forward() {
            server.stop_stepping();
        }
    }
    server.set_frozen(freeze);
    let key = if freeze { "commands.tick.status.frozen" } else { "commands.tick.status.running" };
    source.send_success(server, || Component::translatable(key), true);
    Ok(freeze as i32)
}

pub fn step(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    let advance = if context.has_argument("time") { context.get_int("time") } else { 1 };
    if !server.step_game_if_paused(advance) {
        return Err(ERROR_TICK_STEP_NOT_FROZEN.create());
    }
    source.send_success(server, || Component::translatable_with("commands.tick.step.success", vec![advance.into()]), true);
    Ok(1)
}

pub fn stop_stepping(server: &mut Server, _context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    if !server.stop_stepping() {
        return Err(ERROR_STEP_STOP_NOT_STEPPING.create());
    }
    source.send_success(server, || Component::translatable("commands.tick.step.stop.success"), true);
    Ok(1)
}

pub fn stop_sprinting(server: &mut Server, _context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    if !server.stop_sprinting() {
        return Err(ERROR_SPRINT_STOP_NOT_SPRINTING.create());
    }
    source.send_success(server, || Component::translatable("commands.tick.sprint.stop.success"), true);
    Ok(1)
}

fn suggest_fixed(values: &[&str], mut builder: crate::commands::suggestion::SuggestionsBuilder) -> std::result::Result<crate::commands::suggestion::Suggestions, CommandSyntaxException> {
    crate::commands::shared_suggestion_provider::suggest(values, &mut builder);
    Ok(builder.build())
}

pub fn suggest_rate(
    _server: &Server,
    _context: &CommandContext,
    _source: &CommandSourceStack,
    builder: crate::commands::suggestion::SuggestionsBuilder,
) -> std::result::Result<crate::commands::suggestion::Suggestions, CommandSyntaxException> {
    suggest_fixed(&["20.0"], builder)
}

pub fn suggest_step(
    _server: &Server,
    _context: &CommandContext,
    _source: &CommandSourceStack,
    builder: crate::commands::suggestion::SuggestionsBuilder,
) -> std::result::Result<crate::commands::suggestion::Suggestions, CommandSyntaxException> {
    suggest_fixed(&["1t", "1s"], builder)
}

pub fn suggest_sprint(
    _server: &Server,
    _context: &CommandContext,
    _source: &CommandSourceStack,
    builder: crate::commands::suggestion::SuggestionsBuilder,
) -> std::result::Result<crate::commands::suggestion::Suggestions, CommandSyntaxException> {
    suggest_fixed(&["60s", "1d", "3d"], builder)
}
