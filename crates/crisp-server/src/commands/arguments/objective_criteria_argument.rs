use super::ArgumentValue;
use crate::commands::brigadier::{CommandContext, CommandSyntaxException, DISPATCHER_PARSE_EXCEPTION, DynamicCommandExceptionType, StringReader, tr};
use crate::world::scores::criteria::ObjectiveCriteria;

static ERROR_INVALID_VALUE: DynamicCommandExceptionType = DynamicCommandExceptionType(|value| tr("argument.criteria.invalid", vec![value]));

pub fn parse(reader: &mut StringReader) -> Result<ObjectiveCriteria, CommandSyntaxException> {
    let start = reader.cursor;
    while reader.can_read() && reader.peek() != b' ' as u16 {
        reader.skip();
    }
    let id = reader.substring(start, reader.cursor);
    match ObjectiveCriteria::by_name(&id) {
        Ok(Some(criteria)) => Ok(criteria),
        Ok(None) => {
            reader.cursor = start;
            Err(ERROR_INVALID_VALUE.create_with_context(reader, id))
        }
        Err(message) => Err(DISPATCHER_PARSE_EXCEPTION.create_with_context(reader, message)),
    }
}

impl CommandContext {
    pub fn get_criteria(&self, name: &str) -> &ObjectiveCriteria {
        match self.argument(name) {
            ArgumentValue::ObjectiveCriteria(v) => v,
            other => panic!("{name} is {other:?}"),
        }
    }
}
