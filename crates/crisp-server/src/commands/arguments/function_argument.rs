
use std::sync::Arc;

use super::{ArgumentValue, read_identifier};
use crate::commands::brigadier::{CommandContext, CommandSyntaxException, DynamicCommandExceptionType, StringReader, tr};
use crate::commands::functions::CommandFunction;
use crate::server::Server;

static ERROR_UNKNOWN_FUNCTION: DynamicCommandExceptionType = DynamicCommandExceptionType(|value| tr("arguments.function.unknown", vec![value]));

pub fn parse(reader: &mut StringReader) -> Result<ArgumentValue, CommandSyntaxException> {
    let tag = reader.at('#');
    if tag {
        reader.skip();
    }
    Ok(ArgumentValue::Function { tag, id: read_identifier(reader)? })
}

pub enum FunctionOrTag {
    Function(Arc<CommandFunction>),
    Tag(Vec<Arc<CommandFunction>>),
}

fn argument<'a>(context: &'a CommandContext, name: &str) -> (bool, &'a str) {
    match context.argument(name) {
        ArgumentValue::Function { tag, id } => (*tag, id),
        other => panic!("{name} is {other:?}"),
    }
}

fn get_function(server: &Server, id: &str) -> Result<Arc<CommandFunction>, CommandSyntaxException> {
    server.functions.get(id).ok_or_else(|| ERROR_UNKNOWN_FUNCTION.create(id))
}

fn get_function_tag(server: &Server, id: &str) -> Vec<Arc<CommandFunction>> {
    server.functions.get_tag(id).to_vec()
}

pub fn get_functions(server: &Server, context: &CommandContext, name: &str) -> Result<Vec<Arc<CommandFunction>>, CommandSyntaxException> {
    Ok(get_function_collection(server, context, name)?.1)
}

pub fn get_function_or_tag(server: &Server, context: &CommandContext, name: &str) -> Result<(String, FunctionOrTag), CommandSyntaxException> {
    let (tag, id) = argument(context, name);
    let result = if tag { FunctionOrTag::Tag(get_function_tag(server, id)) } else { FunctionOrTag::Function(get_function(server, id)?) };
    Ok((id.to_string(), result))
}

pub fn get_function_collection(server: &Server, context: &CommandContext, name: &str) -> Result<(String, Vec<Arc<CommandFunction>>), CommandSyntaxException> {
    let (tag, id) = argument(context, name);
    let functions = if tag { get_function_tag(server, id) } else { vec![get_function(server, id)?] };
    Ok((id.to_string(), functions))
}
