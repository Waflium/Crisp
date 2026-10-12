use super::ArgumentValue;
use crate::commands::brigadier::{CommandContext, CommandSyntaxException, DISPATCHER_PARSE_EXCEPTION, DynamicCommandExceptionType, StringReader, tr};

pub const EXAMPLES: [&str; 2] = ["F00", "FF0000"];
static ERROR_INVALID_HEX: DynamicCommandExceptionType = DynamicCommandExceptionType(|value| tr("argument.hexcolor.invalid", vec![value]));

pub fn parse(reader: &mut StringReader) -> Result<i32, CommandSyntaxException> {
    let color_string = reader.read_unquoted_string();
    let s: Vec<char> = color_string.chars().collect();
    let result = match s.len() {
        3 => parse_int(&s, 0, 1, 16).and_then(|r| Ok(color(duplicate_digit(r), duplicate_digit(parse_int(&s, 1, 2, 16)?), duplicate_digit(parse_int(&s, 2, 3, 16)?)))),
        6 => parse_int(&s, 0, 2, 16).and_then(|r| Ok(color(r, parse_int(&s, 2, 4, 16)?, parse_int(&s, 4, 6, 16)?))),
        _ => return Err(ERROR_INVALID_HEX.create_with_context(reader, color_string)),
    };
    result.map_err(|message| DISPATCHER_PARSE_EXCEPTION.create_with_context(reader, message))
}

fn color(red: i32, green: i32, blue: i32) -> i32 {
    255 << 24 | (red & 255) << 16 | (green & 255) << 8 | blue & 255
}

fn duplicate_digit(digit: i32) -> i32 {
    digit * 17
}

fn parse_int(s: &[char], begin_index: usize, end_index: usize, radix: u32) -> Result<i32, String> {
    let error = |error_index: usize| format!("Error at index {} in: \"{}\"", error_index - begin_index, s[begin_index..end_index].iter().collect::<String>());
    let mut negative = false;
    let mut i = begin_index;
    let mut limit = -i32::MAX;
    let first_char = s[i];
    if first_char < '0' {
        if first_char == '-' {
            negative = true;
            limit = i32::MIN;
        } else if first_char != '+' {
            return Err(error(i));
        }
        i += 1;
        if i == end_index {
            return Err(error(i));
        }
    }
    let multmin = limit / radix as i32;
    let mut result = 0i32;
    while i < end_index {
        let digit = s[i].to_digit(radix).map_or(-1, |d| d as i32);
        if digit < 0 || result < multmin {
            return Err(error(i));
        }
        result *= radix as i32;
        if result < limit + digit {
            return Err(error(i));
        }
        i += 1;
        result -= digit;
    }
    Ok(if negative { result } else { -result })
}

impl CommandContext {
    pub fn get_hex_color(&self, name: &str) -> i32 {
        match self.argument(name) {
            ArgumentValue::Int(v) => *v,
            other => panic!("{name} is {other:?}"),
        }
    }
}
