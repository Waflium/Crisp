
mod function_builder;
pub mod macro_function;
pub mod string_template;

use std::sync::Arc;

use crisp_protocol::nbt::Nbt;

use self::function_builder::FunctionBuilder;
use self::macro_function::MacroFunction;
use super::brigadier::{self, CommandSyntaxException, ContextChain, DISPATCHER_UNKNOWN_COMMAND, StringReader, utf16};
use super::execution::ChainStage;
use super::execution::tasks::BuildContexts;
use super::source::CommandSourceStack;
use crate::network::chat::Component;

const MAX_COMMAND_LINE_LENGTH: usize = 2_000_000;

#[derive(Debug)]
pub struct PlainTextFunction {
    pub id: String,
    pub entries: Vec<BuildContexts>,
}

pub type InstantiatedFunction = PlainTextFunction;

impl std::fmt::Debug for BuildContexts {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str(&self.0.chain.executable.input)
    }
}

#[derive(Debug)]
pub enum CommandFunction {
    PlainText(Arc<PlainTextFunction>),
    Macro(MacroFunction),
}

impl CommandFunction {
    pub fn id(&self) -> &str {
        match self {
            CommandFunction::PlainText(function) => &function.id,
            CommandFunction::Macro(function) => function.id(),
        }
    }

    pub fn instantiate(&self, arguments: Option<&Nbt>) -> Result<Arc<InstantiatedFunction>, Component> {
        match self {
            CommandFunction::PlainText(function) => Ok(function.clone()),
            CommandFunction::Macro(function) => function.instantiate(arguments),
        }
    }

    pub fn from_lines(id: &str, compilation_context: &CommandSourceStack, lines: &[String]) -> Result<CommandFunction, String> {
        let mut builder = FunctionBuilder::default();
        let mut i = 0;
        while i < lines.len() {
            let line_number = i + 1;
            let input_line = java_trim(&lines[i]);
            let line = if should_concatenate_next_line(input_line) {
                let mut builder: Vec<u16> = input_line.encode_utf16().collect();
                loop {
                    i += 1;
                    if i == lines.len() {
                        return Err("Line continuation at end of file".into());
                    }
                    builder.pop();
                    builder.extend(java_trim(&lines[i]).encode_utf16());
                    check_command_line_length(&builder)?;
                    if builder.last() != Some(&(b'\\' as u16)) {
                        break;
                    }
                }
                utf16(&builder)
            } else {
                input_line.to_string()
            };
            let units: Vec<u16> = line.encode_utf16().collect();
            check_command_line_length(&units)?;
            let mut input = StringReader::new(&units);
            if input.can_read() && !input.at('#') {
                if input.at('/') {
                    input.skip();
                    if input.at('/') {
                        return Err(format!("Unknown or invalid command '{line}' on line {line_number} (if you intended to make a comment, use '#' not '//')"));
                    }
                    let name = input.read_unquoted_string();
                    return Err(format!("Unknown or invalid command '{line}' on line {line_number} (did you mean '{name}'? Do not use a preceding forwards slash.)"));
                }
                if input.at('$') {
                    builder.add_macro(&utf16(&units[1..]), line_number, compilation_context)?;
                } else {
                    let command = parse_command(compilation_context, input).map_err(|e| format!("Whilst parsing command on line {line_number}: {}", e.get_message()))?;
                    builder.add_command(command);
                }
            }
            i += 1;
        }
        Ok(builder.build(id))
    }
}

fn should_concatenate_next_line(line: &str) -> bool {
    line.ends_with('\\')
}

fn java_trim(s: &str) -> &str {
    s.trim_matches(|c: char| c <= ' ')
}

fn check_command_line_length(line: &[u16]) -> Result<(), String> {
    if line.len() > MAX_COMMAND_LINE_LENGTH {
        let truncated = utf16(&line[..512.min(MAX_COMMAND_LINE_LENGTH)]);
        return Err(format!("Command too long: {} characters, contents: {truncated}...", line.len()));
    }
    Ok(())
}

pub fn parse_command(compilation_context: &CommandSourceStack, input: StringReader) -> Result<BuildContexts, CommandSyntaxException> {
    let line = utf16(input.string());
    let parse = brigadier::parse(input, compilation_context);
    if let Some(e) = super::parse_exception(&parse) {
        return Err(e);
    }
    let chain = ContextChain::try_flatten(parse.context.build(&line)).ok_or_else(|| DISPATCHER_UNKNOWN_COMMAND.create_with_context(&parse.reader))?;
    Ok(BuildContexts(ChainStage::new(chain)))
}
