use std::sync::{Arc, Mutex};

use crisp_protocol::nbt::Nbt;

use super::data::data_accessor::{DataAccessor, KINDS};
use super::data::get_single_tag;
use crate::commands::arguments::function_argument;
use crate::commands::brigadier::{CommandContext, CommandSyntaxException, Dynamic2CommandExceptionType, DynamicCommandExceptionType, tr};
use crate::commands::execution::tasks::CallFunction;
use crate::commands::execution::{ChainModifiers, ChainStage, EntryAction, ExecutionControl};
use crate::commands::functions::CommandFunction;
use crate::commands::source::{CommandResultCallback, CommandSourceStack};
use crate::commands::suggestion::{Suggestions, SuggestionsBuilder};
use crate::commands::{CustomCommandFn, shared_suggestion_provider};
use crate::network::chat::{Arg, Component};
use crate::server::Server;

static ERROR_ARGUMENT_NOT_COMPOUND: DynamicCommandExceptionType =
    DynamicCommandExceptionType(|kind| tr("commands.function.error.argument_not_compound", vec![kind]));
static ERROR_NO_FUNCTIONS: DynamicCommandExceptionType = DynamicCommandExceptionType(|name| tr("commands.function.scheduled.no_functions", vec![name]));
pub static ERROR_FUNCTION_INSTANTATION_FAILURE: Dynamic2CommandExceptionType =
    Dynamic2CommandExceptionType(|id, reason| tr("commands.function.instantiationFailure", vec![id, reason]));

const WITH_KIND: usize = 3;

pub fn custom_executors() -> Vec<(String, CustomCommandFn)> {
    let mut all: Vec<(String, CustomCommandFn)> = vec![("function name".into(), run), ("function name arguments".into(), run)];
    for kind in KINDS {
        all.push((format!("function name with {kind} source"), run));
        all.push((format!("function name with {kind} source path"), run));
    }
    all
}

pub fn suggest_function(server: &Server, _context: &CommandContext, _source: &CommandSourceStack, mut builder: SuggestionsBuilder) -> Result<Suggestions, CommandSyntaxException> {
    shared_suggestion_provider::suggest_resource_with_prefix(server.functions.tag_names(), &mut builder, "#");
    shared_suggestion_provider::suggest_resource(server.functions.function_names(), &mut builder);
    Ok(builder.build())
}

fn type_name(tag: &Nbt) -> &'static str {
    match tag {
        Nbt::Byte(_) => "BYTE",
        Nbt::Short(_) => "SHORT",
        Nbt::Int(_) => "INT",
        Nbt::Long(_) => "LONG",
        Nbt::Float(_) => "FLOAT",
        Nbt::Double(_) => "DOUBLE",
        Nbt::ByteArray(_) => "BYTE[]",
        Nbt::String(_) => "STRING",
        Nbt::List(_) => "LIST",
        Nbt::Compound(_) => "COMPOUND",
        Nbt::IntArray(_) => "INT[]",
        Nbt::LongArray(_) => "LONG[]",
    }
}

fn arguments(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<Option<Nbt>, CommandSyntaxException> {
    if context.has_argument("arguments") {
        return Ok(Some(context.get_nbt("arguments").clone()));
    }
    if !context.has_argument("source") {
        return Ok(None);
    }
    let accessor = DataAccessor::access(server, context, source, WITH_KIND, "source")?;
    if !context.has_argument("path") {
        return Ok(Some(accessor.get_data(server)?));
    }
    match get_single_tag(server, context.get_nbt_path("path"), &accessor)? {
        tag @ Nbt::Compound(_) => Ok(Some(tag)),
        tag => Err(ERROR_ARGUMENT_NOT_COMPOUND.create(type_name(&tag))),
    }
}

pub fn modify_sender_for_execution(sender: &CommandSourceStack) -> CommandSourceStack {
    sender.with_suppressed_output().with_maximum_permission(2)
}

fn run(server: &mut Server, sender: &CommandSourceStack, current_step: &ChainStage, modifiers: ChainModifiers, output: &ExecutionControl) {
    if let Err(e) = run_guarded(server, sender, current_step, modifiers, output) {
        sender.handle_error(server, e, modifiers.is_forked());
        sender.callback.on_failure(server);
    }
}

fn run_guarded(server: &mut Server, sender: &CommandSourceStack, current_step: &ChainStage, modifiers: ChainModifiers, output: &ExecutionControl) -> Result<(), CommandSyntaxException> {
    let context = current_step.top_context();
    let (name, functions) = function_argument::get_function_collection(server, context, "name")?;
    if functions.is_empty() {
        return Err(ERROR_NO_FUNCTIONS.create(Component::literal(&name)));
    }
    let arguments = arguments(server, context, sender)?;
    let common_function_context = modify_sender_for_execution(sender);
    if functions.len() == 1 {
        let id = functions[0].id().to_string();
        sender.send_success(server, || tr("commands.function.scheduled.single", vec![Arg::Component(Component::literal(&id))]), true);
    } else {
        let ids = functions.iter().map(|f| Component::literal(f.id())).collect();
        sender.send_success(server, || tr("commands.function.scheduled.multiple", vec![Arg::Component(super::list_players_command::format_list(ids))]), true);
    }
    queue_functions(&functions, arguments.as_ref(), sender, &common_function_context, output, modifiers)
}

fn signal_result(server: &mut Server, original_source: &CommandSourceStack, id: &str, new_value: i32) {
    original_source.send_success(server, || tr("commands.function.result", vec![Arg::Component(Component::literal(id)), Arg::Int(new_value)]), true);
}

pub fn queue_functions(
    functions: &[Arc<CommandFunction>],
    arguments: Option<&Nbt>,
    original_source: &CommandSourceStack,
    function_source: &CommandSourceStack,
    output: &ExecutionControl,
    modifiers: ChainModifiers,
) -> Result<(), CommandSyntaxException> {
    if modifiers.is_return() {
        queue_functions_as_return(functions, arguments, original_source, function_source, output)
    } else {
        queue_functions_no_return(functions, arguments, original_source, function_source, output)
    }
}

fn instantiate_and_queue_functions(
    arguments: Option<&Nbt>,
    output: &ExecutionControl,
    no_callback_source: &CommandSourceStack,
    function: &CommandFunction,
    function_result_collector: CommandResultCallback,
    return_parent_frame: bool,
) -> Result<(), CommandSyntaxException> {
    let instantiated = function.instantiate(arguments).map_err(|reason| ERROR_FUNCTION_INSTANTATION_FAILURE.create(function.id(), reason))?;
    let call = CallFunction { function: instantiated, result_callback: function_result_collector, return_parent_frame };
    output.queue_next(EntryAction::CallFunction(call, no_callback_source.clone()));
    Ok(())
}

fn decorate_output_if_needed(original_source: &CommandSourceStack, id: &str, callback: CommandResultCallback) -> CommandResultCallback {
    if original_source.silent {
        return callback;
    }
    let (original_source, id) = (original_source.clone(), id.to_string());
    CommandResultCallback::new(move |server, success, result| {
        signal_result(server, &original_source, &id, result);
        callback.on_result(server, success, result);
    })
}

fn queue_functions_as_return(
    functions: &[Arc<CommandFunction>],
    arguments: Option<&Nbt>,
    original_source: &CommandSourceStack,
    function_source: &CommandSourceStack,
    output: &ExecutionControl,
) -> Result<(), CommandSyntaxException> {
    let no_callback_source = function_source.clear_callbacks();
    let function_command_output_callback = CommandResultCallback::chain(&original_source.callback, &output.current_frame().return_value_consumer);
    for function in functions {
        let collector = decorate_output_if_needed(original_source, function.id(), function_command_output_callback.clone());
        instantiate_and_queue_functions(arguments, output, &no_callback_source, function, collector, true)?;
    }
    output.queue_next(EntryAction::Fallthrough);
    Ok(())
}

fn queue_functions_no_return(
    functions: &[Arc<CommandFunction>],
    arguments: Option<&Nbt>,
    original_source: &CommandSourceStack,
    function_source: &CommandSourceStack,
    output: &ExecutionControl,
) -> Result<(), CommandSyntaxException> {
    let no_callback_source = function_source.clear_callbacks();
    let original_callback = original_source.callback.clone();
    match functions {
        [] => {}
        [function] => {
            let collector = decorate_output_if_needed(original_source, function.id(), original_callback);
            instantiate_and_queue_functions(arguments, output, &no_callback_source, function, collector, false)?;
        }
        _ if original_callback.is_empty() => {
            for function in functions {
                let collector = decorate_output_if_needed(original_source, function.id(), original_callback.clone());
                instantiate_and_queue_functions(arguments, output, &no_callback_source, function, collector, false)?;
            }
        }
        _ => {
            let accumulator = Arc::new(Mutex::new((false, 0i32)));
            let partial = {
                let accumulator = accumulator.clone();
                CommandResultCallback::new(move |_, _, result| {
                    let mut accumulator = accumulator.lock().unwrap();
                    accumulator.0 = true;
                    accumulator.1 = accumulator.1.wrapping_add(result);
                })
            };
            for function in functions {
                let collector = decorate_output_if_needed(original_source, function.id(), partial.clone());
                instantiate_and_queue_functions(arguments, output, &no_callback_source, function, collector, false)?;
            }
            output.queue_next(EntryAction::Custom(Box::new(move |server, _| {
                let (any_result, sum) = *accumulator.lock().unwrap();
                if any_result {
                    original_callback.on_success(server, sum);
                }
            })));
        }
    }
    Ok(())
}
