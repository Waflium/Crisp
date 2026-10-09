use crate::commands::brigadier::{CommandContext, CommandSyntaxException, DynamicCommandExceptionType, tr};
use crate::commands::source::CommandSourceStack;
use crate::network::chat::{Arg, Component};
use crate::server::Server;
use crate::world::stopwatches::{Stopwatch, Stopwatches};

static ERROR_ALREADY_EXISTS: DynamicCommandExceptionType = DynamicCommandExceptionType(|id| tr("commands.stopwatch.already_exists", vec![id]));
static ERROR_DOES_NOT_EXIST: DynamicCommandExceptionType = DynamicCommandExceptionType(|id| tr("commands.stopwatch.does_not_exist", vec![id]));

type Result<T> = std::result::Result<T, CommandSyntaxException>;

fn success(server: &Server, source: &CommandSourceStack, key: &str, id: &str) {
    source.send_success(server, || Component::translatable_with(key, vec![Component::literal(id).into()]), true);
}

pub fn create_stopwatch(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    let id = context.get_identifier("id");
    if !server.stopwatches.add(id, Stopwatch::new(Stopwatches::current_time())) {
        return Err(ERROR_ALREADY_EXISTS.create(id));
    }
    success(server, source, "commands.stopwatch.create.success", id);
    Ok(1)
}

pub fn query_stopwatch(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    let id = context.get_identifier("id");
    let scale = if context.has_argument("scale") { context.get_double("scale") } else { 1.0 };
    let Some(stopwatch) = server.stopwatches.get(id) else { return Err(ERROR_DOES_NOT_EXIST.create(id)) };
    let elapsed_seconds = stopwatch.elapsed_seconds(Stopwatches::current_time());
    source.send_success(server, || Component::translatable_with("commands.stopwatch.query", vec![Component::literal(id).into(), Arg::Double(elapsed_seconds)]), true);
    Ok((elapsed_seconds * scale) as i32)
}

pub fn restart_stopwatch(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    let id = context.get_identifier("id");
    if !server.stopwatches.update(id, |_| Stopwatch::new(Stopwatches::current_time())) {
        return Err(ERROR_DOES_NOT_EXIST.create(id));
    }
    success(server, source, "commands.stopwatch.restart.success", id);
    Ok(1)
}

pub fn remove_stopwatch(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    let id = context.get_identifier("id");
    if !server.stopwatches.remove(id) {
        return Err(ERROR_DOES_NOT_EXIST.create(id));
    }
    success(server, source, "commands.stopwatch.remove.success", id);
    Ok(1)
}

pub fn suggest_stopwatches(
    server: &Server,
    _context: &CommandContext,
    _source: &CommandSourceStack,
    mut builder: crate::commands::suggestion::SuggestionsBuilder,
) -> std::result::Result<crate::commands::suggestion::Suggestions, CommandSyntaxException> {
    crate::commands::shared_suggestion_provider::suggest_resource(server.stopwatches.ids(), &mut builder);
    Ok(builder.build())
}
