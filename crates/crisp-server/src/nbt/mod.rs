pub mod snbt_grammar;
#[cfg(test)]
pub mod tests;
pub mod text_component_tag_visitor;

use crisp_protocol::nbt::{Nbt, java_hash_map_order};

use crate::commands::brigadier::{CommandSyntaxException, SimpleCommandExceptionType, StringReader};
use crate::util::parsing::packrat::ParseState;

pub static ERROR_TRAILING_DATA: SimpleCommandExceptionType = SimpleCommandExceptionType("argument.nbt.trailing");
pub static ERROR_EXPECTED_COMPOUND: SimpleCommandExceptionType = SimpleCommandExceptionType("argument.nbt.expected.compound");

pub fn parse_as_argument(reader: &mut StringReader) -> Result<Nbt, CommandSyntaxException> {
    let mut state = ParseState::new(reader);
    let result = snbt_grammar::literal(&mut state);
    state.finish(result)
}

pub fn parse_compound_as_argument(reader: &mut StringReader) -> Result<Nbt, CommandSyntaxException> {
    match parse_as_argument(reader)? {
        compound @ Nbt::Compound(_) => Ok(compound),
        _ => Err(ERROR_EXPECTED_COMPOUND.create_with_context(reader)),
    }
}

pub fn parse_fully(input: &str) -> Result<Nbt, CommandSyntaxException> {
    let units: Vec<u16> = input.encode_utf16().collect();
    let mut reader = StringReader::new(&units);
    let result = parse_as_argument(&mut reader)?;
    reader.skip_whitespace();
    if reader.can_read() {
        return Err(ERROR_TRAILING_DATA.create_with_context(&reader));
    }
    Ok(result)
}

pub fn merge(target: &mut Vec<(String, Nbt)>, other: &[(String, Nbt)]) {
    for i in java_hash_map_order(&other.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>()) {
        let (key, value) = &other[i];
        match (target.iter_mut().find(|(k, _)| k == key), value) {
            (Some((_, Nbt::Compound(own))), Nbt::Compound(other)) => merge(own, other),
            (Some((_, own)), _) => *own = value.clone(),
            (None, _) => target.push((key.clone(), value.clone())),
        }
    }
}

pub fn to_string(tag: &Nbt) -> String {
    let mut out = String::new();
    write(tag, &mut out);
    out
}

fn write(tag: &Nbt, out: &mut String) {
    let join = |out: &mut String, items: Vec<String>| out.push_str(&items.join(","));
    match tag {
        Nbt::String(s) => quote_and_escape(s, out),
        Nbt::Byte(v) => out.push_str(&format!("{v}b")),
        Nbt::Short(v) => out.push_str(&format!("{v}s")),
        Nbt::Int(v) => out.push_str(&v.to_string()),
        Nbt::Long(v) => out.push_str(&format!("{v}L")),
        Nbt::Float(v) => out.push_str(&format!("{}f", java_float_to_string(*v))),
        Nbt::Double(v) => out.push_str(&format!("{}d", java_double_to_string(*v))),
        Nbt::ByteArray(values) => {
            out.push_str("[B;");
            join(out, values.iter().map(|v| format!("{v}B")).collect());
            out.push(']');
        }
        Nbt::IntArray(values) => {
            out.push_str("[I;");
            join(out, values.iter().map(|v| v.to_string()).collect());
            out.push(']');
        }
        Nbt::LongArray(values) => {
            out.push_str("[L;");
            join(out, values.iter().map(|v| format!("{v}L")).collect());
            out.push(']');
        }
        Nbt::List(items) => {
            out.push('[');
            join(out, items.iter().map(to_string).collect());
            out.push(']');
        }
        Nbt::Compound(fields) => {
            out.push('{');
            let mut sorted: Vec<&(String, Nbt)> = fields.iter().collect();
            sorted.sort_by(|a, b| a.0.encode_utf16().cmp(b.0.encode_utf16()));
            for (i, (key, value)) in sorted.into_iter().enumerate() {
                if i != 0 {
                    out.push(',');
                }
                handle_key_escape(key, out);
                out.push(':');
                write(value, out);
            }
            out.push('}');
        }
    }
}

fn handle_key_escape(key: &str, out: &mut String) {
    let mut chars = key.chars();
    let bare = chars.next().is_some_and(|c| c.is_ascii_alphabetic() || c == '.' || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '+' | '-'))
        && !key.eq_ignore_ascii_case("true")
        && !key.eq_ignore_ascii_case("false");
    if bare {
        out.push_str(key);
    } else {
        quote_and_escape(key, out);
    }
}

pub fn quote_and_escape(input: &str, out: &mut String) {
    let mut body = String::new();
    let mut quote = None;
    for c in input.chars() {
        match c {
            '\\' => body.push_str("\\\\"),
            '"' | '\'' => {
                let q = *quote.get_or_insert(if c == '"' { '\'' } else { '"' });
                if q == c {
                    body.push('\\');
                }
                body.push(c);
            }
            _ => match escape_control_characters(c) {
                Some(escaped) => {
                    body.push('\\');
                    body.push_str(&escaped);
                }
                None => body.push(c),
            },
        }
    }
    let quote = quote.unwrap_or('"');
    out.push(quote);
    out.push_str(&body);
    out.push(quote);
}

fn escape_control_characters(c: char) -> Option<String> {
    Some(match c {
        '\u{8}' => "b".into(),
        '\t' => "t".into(),
        '\n' => "n".into(),
        '\u{c}' => "f".into(),
        '\r' => "r".into(),
        c if (c as u32) < 0x20 => format!("x{:02X}", c as u32),
        _ => return None,
    })
}

pub fn java_double_to_string(v: f64) -> String {
    java_decimal(v.is_nan(), v.is_infinite(), v.is_sign_negative(), v == 0.0, (1e-3..1e7).contains(&v.abs()), format!("{v:e}"))
}

pub fn java_float_to_string(v: f32) -> String {
    java_decimal(v.is_nan(), v.is_infinite(), v.is_sign_negative(), v == 0.0, (1e-3..1e7).contains(&v.abs()), format!("{v:e}"))
}

fn java_decimal(nan: bool, infinite: bool, negative: bool, zero: bool, plain: bool, scientific: String) -> String {
    if nan {
        return "NaN".into();
    }
    let sign = if negative { "-" } else { "" };
    if infinite {
        return format!("{sign}Infinity");
    }
    if zero {
        return format!("{sign}0.0");
    }
    let scientific = scientific.trim_start_matches('-');
    let (mantissa, exponent) = scientific.split_once('e').unwrap();
    let exponent: i32 = exponent.parse().unwrap();
    let digits: String = mantissa.chars().filter(|&c| c != '.').collect();
    if !plain {
        let fraction = if digits.len() > 1 { &digits[1..] } else { "0" };
        return format!("{sign}{}.{fraction}E{exponent}", &digits[..1]);
    }
    let point = exponent + 1;
    let (whole, fraction) = if point <= 0 {
        ("0".to_string(), format!("{}{digits}", "0".repeat(-point as usize)))
    } else if point as usize >= digits.len() {
        (format!("{digits}{}", "0".repeat(point as usize - digits.len())), "0".to_string())
    } else {
        (digits[..point as usize].to_string(), digits[point as usize..].to_string())
    };
    format!("{sign}{whole}.{fraction}")
}

pub fn from_json(value: &serde_json::Value) -> Option<Nbt> {
    Some(match value {
        serde_json::Value::Null => return None,
        serde_json::Value::Bool(b) => Nbt::Byte(*b as i8),
        serde_json::Value::Number(n) => Nbt::Double(n.as_f64().unwrap_or(0.0)),
        serde_json::Value::String(s) => Nbt::String(s.clone()),
        serde_json::Value::Array(values) => Nbt::List(values.iter().filter_map(from_json).collect()),
        serde_json::Value::Object(fields) => Nbt::Compound(fields.iter().filter_map(|(k, v)| Some((k.clone(), from_json(v)?))).collect()),
    })
}
