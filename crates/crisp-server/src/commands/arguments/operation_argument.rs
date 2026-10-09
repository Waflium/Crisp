use super::ArgumentValue;
use crate::commands::brigadier::{CommandContext, CommandSyntaxException, SimpleCommandExceptionType, StringReader};

static ERROR_INVALID_OPERATION: SimpleCommandExceptionType = SimpleCommandExceptionType("arguments.operation.invalid");
pub static ERROR_DIVIDE_BY_ZERO: SimpleCommandExceptionType = SimpleCommandExceptionType("arguments.operation.div0");

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Operation {
    Assign,
    Add,
    Subtract,
    Multiply,
    Divide,
    Modulo,
    Min,
    Max,
    Swap,
}

pub fn parse(reader: &mut StringReader) -> Result<Operation, CommandSyntaxException> {
    if !reader.can_read() {
        return Err(ERROR_INVALID_OPERATION.create_with_context(reader));
    }
    let start = reader.cursor;
    while reader.can_read() && reader.peek() != b' ' as u16 {
        reader.skip();
    }
    Ok(match reader.substring(start, reader.cursor).as_str() {
        "=" => Operation::Assign,
        "+=" => Operation::Add,
        "-=" => Operation::Subtract,
        "*=" => Operation::Multiply,
        "/=" => Operation::Divide,
        "%=" => Operation::Modulo,
        "<" => Operation::Min,
        ">" => Operation::Max,
        "><" => Operation::Swap,
        _ => return Err(ERROR_INVALID_OPERATION.create()),
    })
}

impl Operation {
    pub fn apply(self, a: i32, b: i32) -> Result<i32, CommandSyntaxException> {
        Ok(match self {
            Operation::Assign => b,
            Operation::Add => a.wrapping_add(b),
            Operation::Subtract => a.wrapping_sub(b),
            Operation::Multiply => a.wrapping_mul(b),
            Operation::Divide if b == 0 => return Err(ERROR_DIVIDE_BY_ZERO.create()),
            Operation::Divide => a.wrapping_div(b) - ((a.wrapping_rem(b) != 0 && (a ^ b) < 0) as i32),
            Operation::Modulo if b == 0 => return Err(ERROR_DIVIDE_BY_ZERO.create()),
            Operation::Modulo => (a as i64).rem_euclid(b as i64) as i32 + if b < 0 && a.wrapping_rem(b) != 0 { b } else { 0 },
            Operation::Min => a.min(b),
            Operation::Max => a.max(b),
            Operation::Swap => unreachable!("a swap sets both scores"),
        })
    }
}

impl CommandContext {
    pub fn get_operation(&self, name: &str) -> Operation {
        match self.argument(name) {
            ArgumentValue::Operation(v) => *v,
            other => panic!("{name} is {other:?}"),
        }
    }
}
