
use std::sync::Arc;

use super::{ChainModifiers, ChainStage, CommandQueueEntry, EntryAction, ExecutionContext, ExecutionControl, Frame, FrameControl};
use crate::commands::brigadier::{CommandSyntaxException, DynamicCommandExceptionType, tr};
use crate::commands::functions::InstantiatedFunction;
use crate::commands::source::{CommandResultCallback, CommandSourceStack};
use crate::commands::{CUSTOM_EXECUTORS, CUSTOM_MODIFIERS, EXECUTORS, MODIFIERS, not_implemented, unimplemented_modifier};
use crate::server::Server;

pub static ERROR_FORK_LIMIT_REACHED: DynamicCommandExceptionType = DynamicCommandExceptionType(|limit| tr("command.forkLimit", vec![limit]));

#[derive(Clone)]
pub struct BuildContexts(pub ChainStage);

impl BuildContexts {
    pub fn execute(&self, server: &mut Server, original_source: &CommandSourceStack, initial_sources: Vec<CommandSourceStack>, frame: &Frame, initial_modifiers: ChainModifiers) {
        let mut current_stage = self.0.clone();
        let mut modifiers = initial_modifiers;
        let mut current_sources = initial_sources;
        if !current_stage.is_execute() {
            let fork_limit = ExecutionContext::with(|c| c.fork_limit()) as usize;
            while !current_stage.is_execute() {
                let context = current_stage.top_context();
                if context.forks {
                    modifiers = modifiers.set_forked();
                }
                if let Some(node) = context.modifier {
                    if let Some(custom) = CUSTOM_MODIFIERS[node as usize] {
                        custom(server, original_source, current_sources, &current_stage, modifiers, &ExecutionControl::new(frame));
                        return;
                    }
                    ExecutionContext::with(ExecutionContext::increment_cost);
                    let forked_mode = modifiers.is_forked();
                    let modifier = MODIFIERS[node as usize].unwrap_or_else(|| unimplemented_modifier(context));
                    let mut next_sources = Vec::new();
                    for source in &current_sources {
                        match modifier(server, context, source) {
                            Ok(new_sources) => {
                                if next_sources.len() + new_sources.len() >= fork_limit {
                                    original_source.handle_error(server, ERROR_FORK_LIMIT_REACHED.create(fork_limit as i32), forked_mode);
                                    return;
                                }
                                next_sources.extend(new_sources);
                            }
                            Err(e) => {
                                source.handle_error(server, e, forked_mode);
                                if !forked_mode {
                                    return;
                                }
                            }
                        }
                    }
                    current_sources = next_sources;
                }
                current_stage = current_stage.next_stage();
            }
        }
        if current_sources.is_empty() {
            if modifiers.is_return() {
                ExecutionContext::queue(CommandQueueEntry::new(frame.clone(), EntryAction::Fallthrough));
            }
            return;
        }
        let execute_context = current_stage.top_context();
        if let Some(custom) = execute_context.command.and_then(|node| CUSTOM_EXECUTORS[node as usize]) {
            let control = ExecutionControl::new(frame);
            for source in &current_sources {
                custom(server, source, &current_stage, modifiers, &control);
            }
            return;
        }
        if modifiers.is_return() {
            let returning = &current_sources[0];
            let returning = returning.with_callback(CommandResultCallback::chain(&returning.callback, &frame.return_value_consumer));
            current_sources = vec![returning];
        }
        schedule(frame, current_sources, ExecuteCommand { modifiers, stage: current_stage });
    }
}

#[derive(Clone)]
pub struct ExecuteCommand {
    modifiers: ChainModifiers,
    stage: ChainStage,
}

impl ExecuteCommand {
    pub fn execute(&self, server: &mut Server, sender: &CommandSourceStack) {
        ExecutionContext::with(ExecutionContext::increment_cost);
        let forked = self.modifiers.is_forked();
        if let Err(e) = run_executable(server, &self.stage, sender, forked) {
            sender.handle_error(server, e, forked);
        }
    }
}

fn run_executable(server: &mut Server, stage: &ChainStage, source: &CommandSourceStack, forked_mode: bool) -> Result<i32, CommandSyntaxException> {
    let executable = stage.top_context();
    let result = match executable.command.and_then(|node| EXECUTORS[node as usize]) {
        Some(command) => command(server, executable, source),
        None => Err(not_implemented(executable)),
    };
    match result {
        Ok(result) => {
            source.callback.on_result(server, true, result);
            Ok(if forked_mode { 1 } else { result })
        }
        Err(e) => {
            source.callback.on_result(server, false, 0);
            if forked_mode { Ok(0) } else { Err(e) }
        }
    }
}

pub struct CallFunction {
    pub function: Arc<InstantiatedFunction>,
    pub result_callback: CommandResultCallback,
    pub return_parent_frame: bool,
}

impl CallFunction {
    pub fn execute(self, sender: CommandSourceStack, frame: &Frame) {
        let new_depth = frame.depth + 1;
        let frame_control = ExecutionContext::with(|context| {
            context.increment_cost();
            if self.return_parent_frame { frame.frame_control } else { FrameControl::DiscardAtDepthOrHigher(new_depth) }
        });
        let new_frame = Frame { depth: new_depth, return_value_consumer: self.result_callback, frame_control };
        let function = self.function;
        match function.entries.len() {
            0 => {}
            1 => ExecutionContext::queue(CommandQueueEntry::new(new_frame, EntryAction::BuildContexts(function.entries[0].clone(), sender))),
            2 => {
                ExecutionContext::queue(CommandQueueEntry::new(new_frame.clone(), EntryAction::BuildContexts(function.entries[0].clone(), sender.clone())));
                ExecutionContext::queue(CommandQueueEntry::new(new_frame, EntryAction::BuildContexts(function.entries[1].clone(), sender)));
            }
            _ => ExecutionContext::queue(CommandQueueEntry::new(new_frame, EntryAction::ContinuationTask(ContinuationTask::Entries { function, sender, index: 0 }))),
        }
    }
}

pub enum ContinuationTask {
    Sources { action: ExecuteCommand, sources: std::vec::IntoIter<CommandSourceStack> },
    Entries { function: Arc<InstantiatedFunction>, sender: CommandSourceStack, index: usize },
}

impl ContinuationTask {
    pub fn execute(mut self, frame: &Frame) {
        let (next, more) = match &mut self {
            ContinuationTask::Sources { action, sources } => {
                let source = sources.next().expect("a source left");
                (EntryAction::ExecuteCommand(action.clone(), source), sources.len() > 0)
            }
            ContinuationTask::Entries { function, sender, index } => {
                let entry = function.entries[*index].clone();
                *index += 1;
                (EntryAction::BuildContexts(entry, sender.clone()), *index < function.entries.len())
            }
        };
        ExecutionContext::queue(CommandQueueEntry::new(frame.clone(), next));
        if more {
            ExecutionContext::queue(CommandQueueEntry::new(frame.clone(), EntryAction::ContinuationTask(self)));
        }
    }
}

fn schedule(frame: &Frame, sources: Vec<CommandSourceStack>, action: ExecuteCommand) {
    match sources.len() {
        0 => {}
        1 | 2 => {
            for source in sources {
                ExecutionContext::queue(CommandQueueEntry::new(frame.clone(), EntryAction::ExecuteCommand(action.clone(), source)));
            }
        }
        _ => {
            let task = ContinuationTask::Sources { action, sources: sources.into_iter() };
            ExecutionContext::queue(CommandQueueEntry::new(frame.clone(), EntryAction::ContinuationTask(task)));
        }
    }
}

pub struct IsolatedCall {
    pub functions: Arc<Vec<Arc<InstantiatedFunction>>>,
    pub source: CommandSourceStack,
    pub output: CommandResultCallback,
}

impl IsolatedCall {
    pub fn execute(self, frame: &Frame) {
        let new_depth = frame.depth + 1;
        let new_frame = Frame { depth: new_depth, return_value_consumer: self.output, frame_control: FrameControl::DiscardAtDepthOrHigher(new_depth) };
        let control = ExecutionControl::new(&new_frame);
        for function in self.functions.iter() {
            let call = CallFunction { function: function.clone(), result_callback: new_frame.return_value_consumer.clone(), return_parent_frame: true };
            control.queue_next(EntryAction::CallFunction(call, self.source.clone()));
        }
        control.queue_next(EntryAction::Fallthrough);
    }
}
