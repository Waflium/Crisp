use crate::commands::CommandFn;
use crate::commands::arguments::function_argument::{self, FunctionOrTag};
use crate::commands::brigadier::{CommandContext, CommandSyntaxException, DynamicCommandExceptionType, SimpleCommandExceptionType, tr};
use crate::commands::functions::CommandFunction;
use crate::commands::shared_suggestion_provider;
use crate::commands::source::CommandSourceStack;
use crate::commands::suggestion::{Suggestions, SuggestionsBuilder};
use crate::network::chat::{Arg, Component};
use crate::resources::identifier;
use crate::server::Server;
use crate::world::level::timers::TimerCallback;

static ERROR_SAME_TICK: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.schedule.same_tick");
static ERROR_CANT_REMOVE: DynamicCommandExceptionType = DynamicCommandExceptionType(|id| tr("commands.schedule.cleared.failure", vec![id]));
static ERROR_MACRO: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.schedule.macro");

pub fn executors() -> Vec<(String, CommandFn)> {
    vec![
        ("schedule function function time".into(), schedule_replace as CommandFn),
        ("schedule function function time append".into(), schedule_append),
        ("schedule function function time replace".into(), schedule_replace),
        ("schedule clear function".into(), remove),
    ]
}

pub fn suggest_schedule(server: &Server, _context: &CommandContext, _source: &CommandSourceStack, mut builder: SuggestionsBuilder) -> Result<Suggestions, CommandSyntaxException> {
    let ids: Result<Vec<String>, String> = server.scheduled_events.events_ids().map(identifier::parse).collect();
    if let Ok(ids) = ids {
        shared_suggestion_provider::suggest_resource(ids, &mut builder);
    }
    Ok(builder.build())
}

fn schedule_replace(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    schedule(server, context, source, true)
}

fn schedule_append(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    schedule(server, context, source, false)
}

fn schedule(server: &mut Server, context: &CommandContext, source: &CommandSourceStack, replace: bool) -> Result<i32, CommandSyntaxException> {
    let (callback_id, callback) = function_argument::get_function_or_tag(server, context, "function")?;
    let time = context.get_int("time");
    if time == 0 {
        return Err(ERROR_SAME_TICK.create());
    }
    let tick_time = server.level.game_time + time as i64;
    let queue = &mut server.scheduled_events;
    let key = match callback {
        FunctionOrTag::Function(function) => {
            if let CommandFunction::Macro(_) = *function {
                return Err(ERROR_MACRO.create());
            }
            if replace {
                queue.remove(&callback_id);
            }
            queue.schedule(&callback_id, tick_time, TimerCallback::Function(callback_id.clone()));
            "commands.schedule.created.function"
        }
        FunctionOrTag::Tag => {
            let schedule_id = format!("#{callback_id}");
            if replace {
                queue.remove(&schedule_id);
            }
            queue.schedule(&schedule_id, tick_time, TimerCallback::FunctionTag(callback_id.clone()));
            "commands.schedule.created.tag"
        }
    };
    source.send_success(server, || tr(key, vec![Arg::Component(Component::literal(&callback_id)), Arg::Int(time), Arg::Long(tick_time)]), true);
    Ok(tick_time.rem_euclid(i32::MAX as i64) as i32)
}

fn remove(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let string_id = context.get_identifier("function").to_string();
    let count = server.scheduled_events.remove(&string_id);
    if count == 0 {
        return Err(ERROR_CANT_REMOVE.create(string_id));
    }
    source.send_success(server, || tr("commands.schedule.cleared.success", vec![Arg::Int(count), Arg::String(string_id.clone())]), true);
    Ok(count)
}
