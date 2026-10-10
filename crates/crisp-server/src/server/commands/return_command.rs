use crate::commands::execution::tasks::BuildContexts;
use crate::commands::execution::{ChainModifiers, ChainStage, ContinuationSources, EntryAction, ExecutionControl};
use crate::commands::source::CommandSourceStack;
use crate::commands::{CustomCommandFn, CustomModifierFn};
use crate::server::Server;

pub fn custom_executors() -> Vec<(String, CustomCommandFn)> {
    vec![("return value".into(), return_value), ("return fail".into(), return_fail)]
}

pub fn custom_modifiers() -> Vec<(String, CustomModifierFn)> {
    vec![("return run".into(), return_from_command)]
}

fn return_fail(server: &mut Server, sender: &CommandSourceStack, _current_step: &ChainStage, _modifiers: ChainModifiers, output: &ExecutionControl) {
    sender.callback.on_failure(server);
    let frame = output.current_frame();
    frame.return_failure(server);
    frame.discard();
}

fn return_from_command(
    _server: &mut Server,
    original_source: &CommandSourceStack,
    current_sources: Vec<CommandSourceStack>,
    current_step: &ChainStage,
    modifiers: ChainModifiers,
    output: &ExecutionControl,
) {
    if current_sources.is_empty() {
        if modifiers.is_return() {
            output.queue_next(EntryAction::Fallthrough);
        }
    } else {
        output.current_frame().discard();
        let next_state = current_step.next_stage();
        output.queue_next(EntryAction::Continuation {
            build: BuildContexts(next_state),
            modifiers: modifiers.set_return(),
            original: original_source.clone(),
            sources: ContinuationSources::Owned(current_sources),
        });
    }
}

fn return_value(server: &mut Server, sender: &CommandSourceStack, current_step: &ChainStage, _modifiers: ChainModifiers, output: &ExecutionControl) {
    let return_value = current_step.top_context().get_int("value");
    sender.callback.on_success(server, return_value);
    let frame = output.current_frame();
    frame.return_success(server, return_value);
    frame.discard();
}
