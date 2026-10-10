
pub mod tasks;

use std::cell::RefCell;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use self::tasks::{BuildContexts, CallFunction, ContinuationTask, ExecuteCommand, IsolatedCall};
use super::brigadier::{CommandContext, ContextChain};
use super::functions::InstantiatedFunction;
use super::source::{CommandResultCallback, CommandSourceStack};
use crate::server::Server;

const MAX_QUEUE_DEPTH: usize = 10_000_000;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct ChainModifiers(u8);

impl ChainModifiers {
    pub const DEFAULT: ChainModifiers = ChainModifiers(0);
    const FLAG_FORKED: u8 = 1;
    const FLAG_IS_RETURN: u8 = 2;

    pub fn is_forked(self) -> bool {
        self.0 & Self::FLAG_FORKED != 0
    }

    pub fn set_forked(self) -> ChainModifiers {
        ChainModifiers(self.0 | Self::FLAG_FORKED)
    }

    pub fn is_return(self) -> bool {
        self.0 & Self::FLAG_IS_RETURN != 0
    }

    pub fn set_return(self) -> ChainModifiers {
        ChainModifiers(self.0 | Self::FLAG_IS_RETURN)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FrameControl {
    ClearQueue,
    DiscardAtDepthOrHigher(i32),
}

#[derive(Clone, Debug)]
pub struct Frame {
    pub depth: i32,
    pub return_value_consumer: CommandResultCallback,
    pub frame_control: FrameControl,
}

impl Frame {
    pub fn return_success(&self, server: &mut Server, value: i32) {
        self.return_value_consumer.on_success(server, value);
    }

    pub fn return_failure(&self, server: &mut Server) {
        self.return_value_consumer.on_failure(server);
    }

    pub fn discard(&self) {
        ExecutionContext::with(|context| match self.frame_control {
            FrameControl::ClearQueue => context.command_queue.clear(),
            FrameControl::DiscardAtDepthOrHigher(depth) => context.discard_at_depth_or_higher(depth),
        });
    }
}

#[derive(Clone)]
pub struct ChainStage {
    pub chain: Arc<ContextChain>,
    pub stage: usize,
}

impl ChainStage {
    pub fn new(chain: ContextChain) -> ChainStage {
        ChainStage { chain: Arc::new(chain), stage: 0 }
    }

    pub fn is_execute(&self) -> bool {
        self.stage >= self.chain.modifiers.len()
    }

    pub fn top_context(&self) -> &CommandContext {
        self.chain.modifiers.get(self.stage).unwrap_or(&self.chain.executable)
    }

    pub fn next_stage(&self) -> ChainStage {
        ChainStage { chain: self.chain.clone(), stage: self.stage + 1 }
    }
}

pub enum ContinuationSources {
    Owned(Vec<CommandSourceStack>),
    Shared(Arc<Mutex<Vec<CommandSourceStack>>>),
}

pub enum EntryAction {
    BuildContexts(BuildContexts, CommandSourceStack),
    Continuation { build: BuildContexts, modifiers: ChainModifiers, original: CommandSourceStack, sources: ContinuationSources },
    ExecuteCommand(ExecuteCommand, CommandSourceStack),
    CallFunction(CallFunction, CommandSourceStack),
    ContinuationTask(ContinuationTask),
    Fallthrough,
    IsolatedCall(IsolatedCall),
    Custom(Box<dyn FnOnce(&mut Server, &Frame)>),
}

pub struct CommandQueueEntry {
    pub frame: Frame,
    pub action: EntryAction,
}

impl CommandQueueEntry {
    pub fn new(frame: Frame, action: EntryAction) -> CommandQueueEntry {
        CommandQueueEntry { frame, action }
    }

    fn execute(self, server: &mut Server) {
        let frame = &self.frame;
        match self.action {
            EntryAction::BuildContexts(build, sender) => build.execute(server, &sender, vec![sender.clone()], frame, ChainModifiers::DEFAULT),
            EntryAction::Continuation { build, modifiers, original, sources } => {
                let sources = match sources {
                    ContinuationSources::Owned(sources) => sources,
                    ContinuationSources::Shared(sources) => std::mem::take(&mut *sources.lock().unwrap()),
                };
                build.execute(server, &original, sources, frame, modifiers);
            }
            EntryAction::ExecuteCommand(command, sender) => command.execute(server, &sender),
            EntryAction::CallFunction(call, sender) => call.execute(sender, frame),
            EntryAction::ContinuationTask(task) => task.execute(frame),
            EntryAction::Fallthrough => {
                frame.return_failure(server);
                frame.discard();
            }
            EntryAction::IsolatedCall(call) => call.execute(frame),
            EntryAction::Custom(action) => action(server, frame),
        }
    }
}

pub struct ExecutionControl<'a> {
    frame: &'a Frame,
}

impl<'a> ExecutionControl<'a> {
    pub fn new(frame: &'a Frame) -> ExecutionControl<'a> {
        ExecutionControl { frame }
    }

    pub fn queue_next(&self, action: EntryAction) {
        ExecutionContext::queue(CommandQueueEntry::new(self.frame.clone(), action));
    }

    pub fn current_frame(&self) -> &Frame {
        self.frame
    }
}

pub struct ExecutionContext {
    command_limit: i32,
    fork_limit: i32,
    command_quota: i32,
    queue_overflow: bool,
    command_queue: VecDeque<CommandQueueEntry>,
    new_top_commands: Vec<CommandQueueEntry>,
    current_frame_depth: i32,
}

thread_local! {
    static CURRENT_EXECUTION_CONTEXT: RefCell<Option<ExecutionContext>> = const { RefCell::new(None) };
}

struct ContextGuard;

impl Drop for ContextGuard {
    fn drop(&mut self) {
        CURRENT_EXECUTION_CONTEXT.with(|c| *c.borrow_mut() = None);
    }
}

impl ExecutionContext {
    pub fn new(command_limit: i32, fork_limit: i32) -> ExecutionContext {
        ExecutionContext {
            command_limit,
            fork_limit,
            command_quota: command_limit,
            queue_overflow: false,
            command_queue: VecDeque::new(),
            new_top_commands: Vec::new(),
            current_frame_depth: 0,
        }
    }

    pub fn is_running() -> bool {
        CURRENT_EXECUTION_CONTEXT.with(|c| c.borrow().is_some())
    }

    pub fn with<R>(f: impl FnOnce(&mut ExecutionContext) -> R) -> R {
        CURRENT_EXECUTION_CONTEXT.with(|c| f(c.borrow_mut().as_mut().expect("no command execution context")))
    }

    pub fn run(server: &mut Server, context: ExecutionContext, config: impl FnOnce(&mut ExecutionContext)) {
        CURRENT_EXECUTION_CONTEXT.with(|c| *c.borrow_mut() = Some(context));
        let _guard = ContextGuard;
        ExecutionContext::with(config);
        ExecutionContext::run_command_queue(server);
    }

    fn create_top_frame(&self, frame_result: CommandResultCallback) -> Frame {
        if self.current_frame_depth == 0 {
            return Frame { depth: 0, return_value_consumer: frame_result, frame_control: FrameControl::ClearQueue };
        }
        let depth = self.current_frame_depth + 1;
        Frame { depth, return_value_consumer: frame_result, frame_control: FrameControl::DiscardAtDepthOrHigher(depth) }
    }

    pub fn queue_initial_function_call(&mut self, function: Arc<InstantiatedFunction>, sender: &CommandSourceStack, function_return: CommandResultCallback) {
        let frame = self.create_top_frame(function_return);
        let call = CallFunction { function, result_callback: sender.callback.clone(), return_parent_frame: false };
        self.queue_next(CommandQueueEntry::new(frame, EntryAction::CallFunction(call, sender.clone())));
    }

    pub fn queue_initial_command_execution(&mut self, chain: ContextChain, sender: &CommandSourceStack, command_return: CommandResultCallback) {
        let frame = self.create_top_frame(command_return);
        self.queue_next(CommandQueueEntry::new(frame, EntryAction::BuildContexts(BuildContexts(ChainStage::new(chain)), sender.clone())));
    }

    fn handle_queue_overflow(&mut self) {
        self.queue_overflow = true;
        self.new_top_commands.clear();
        self.command_queue.clear();
    }

    pub fn queue_next(&mut self, entry: CommandQueueEntry) {
        if self.new_top_commands.len() + self.command_queue.len() > MAX_QUEUE_DEPTH {
            self.handle_queue_overflow();
        }
        if !self.queue_overflow {
            self.new_top_commands.push(entry);
        }
    }

    pub fn queue(entry: CommandQueueEntry) {
        ExecutionContext::with(|context| context.queue_next(entry));
    }

    pub fn discard_at_depth_or_higher(&mut self, depth_to_discard: i32) {
        while self.command_queue.front().is_some_and(|entry| entry.frame.depth >= depth_to_discard) {
            self.command_queue.pop_front();
        }
    }

    fn run_command_queue(server: &mut Server) {
        ExecutionContext::with(ExecutionContext::push_new_commands);
        loop {
            let next = ExecutionContext::with(|context| {
                if context.command_quota <= 0 {
                    info!("Command execution stopped due to limit (executed {} commands)", context.command_limit);
                    return None;
                }
                let entry = context.command_queue.pop_front()?;
                context.current_frame_depth = entry.frame.depth;
                Some(entry)
            });
            let Some(entry) = next else { break };
            entry.execute(server);
            let overflow = ExecutionContext::with(|context| {
                if context.queue_overflow {
                    return true;
                }
                context.push_new_commands();
                false
            });
            if overflow {
                error!("Command execution stopped due to command queue overflow (max {MAX_QUEUE_DEPTH})");
                break;
            }
        }
        ExecutionContext::with(|context| context.current_frame_depth = 0);
    }

    fn push_new_commands(&mut self) {
        for entry in self.new_top_commands.drain(..).rev() {
            self.command_queue.push_front(entry);
        }
    }

    pub fn fork_limit(&self) -> i32 {
        self.fork_limit
    }

    pub fn increment_cost(&mut self) {
        self.command_quota -= 1;
    }
}
