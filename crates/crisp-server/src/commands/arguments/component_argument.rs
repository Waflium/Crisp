use super::ArgumentValue;
use crate::commands::brigadier::{CommandContext, CommandSyntaxException, DynamicCommandExceptionType, StringReader, tr};
use crate::commands::source::CommandSourceStack;
use crate::network::chat::component_serialization::{decode, decode_style_fields};
use crate::network::chat::component_utils;
use crate::network::chat::{Component, Style};
use crate::server::Server;

static ERROR_INVALID_COMPONENT: DynamicCommandExceptionType = DynamicCommandExceptionType(|message| tr("argument.component.invalid", vec![message]));
static ERROR_INVALID_STYLE: DynamicCommandExceptionType = DynamicCommandExceptionType(|message| tr("argument.style.invalid", vec![message]));

pub fn parse_component(reader: &mut StringReader) -> Result<Component, CommandSyntaxException> {
    let cursor = reader.cursor;
    let tag = crate::nbt::parse_as_argument(reader)?;
    decode(&tag).map_err(|message| {
        reader.cursor = cursor;
        ERROR_INVALID_COMPONENT.create_with_context(reader, message)
    })
}

pub fn parse_style(reader: &mut StringReader) -> Result<Style, CommandSyntaxException> {
    let cursor = reader.cursor;
    let tag = crate::nbt::parse_as_argument(reader)?;
    let result = match &tag {
        crisp_protocol::nbt::Nbt::Compound(fields) => decode_style_fields(fields),
        other => Err(format!("Not a map: {}", crate::nbt::to_string(other))),
    };
    result.map_err(|message| {
        reader.cursor = cursor;
        ERROR_INVALID_STYLE.create_with_context(reader, message)
    })
}

impl CommandContext {
    pub fn get_raw_component(&self, name: &str) -> &Component {
        match self.argument(name) {
            ArgumentValue::Component(v) => v,
            other => panic!("{name} is {other:?}"),
        }
    }

    pub fn get_resolved_component(&self, server: &Server, source: &CommandSourceStack, name: &str) -> Result<Component, CommandSyntaxException> {
        component_utils::resolve(server, source, self.get_raw_component(name))
    }

    pub fn get_style(&self, name: &str) -> &Style {
        match self.argument(name) {
            ArgumentValue::Style(v) => v,
            other => panic!("{name} is {other:?}"),
        }
    }
}
