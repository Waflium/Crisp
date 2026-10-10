
use std::cell::RefCell;
use std::sync::Arc;

use crisp_protocol::nbt::Nbt;

use super::string_template::StringTemplate;
use super::{InstantiatedFunction, PlainTextFunction, parse_command};
use crate::commands::brigadier::StringReader;
use crate::commands::execution::tasks::BuildContexts;
use crate::commands::source::CommandSourceStack;
use crate::network::chat::{Arg, Component};
use crisp_core::random::java_string_hash;

const MAX_CACHE_ENTRIES: usize = 8;

#[derive(Debug)]
pub enum Entry {
    PlainText(BuildContexts),
    Macro(MacroEntry),
}

#[derive(Debug)]
pub struct MacroEntry {
    pub template: StringTemplate,
    pub parameters: Vec<usize>,
    pub compilation_context: CommandSourceStack,
}

#[derive(Debug)]
pub struct MacroFunction {
    id: String,
    entries: Vec<Entry>,
    parameters: Vec<String>,
    cache: RefCell<Vec<(Vec<String>, Arc<InstantiatedFunction>)>>,
}

impl MacroFunction {
    pub fn new(id: String, entries: Vec<Entry>, parameters: Vec<String>) -> MacroFunction {
        MacroFunction { id, entries, parameters, cache: RefCell::new(Vec::new()) }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn instantiate(&self, arguments: Option<&Nbt>) -> Result<Arc<InstantiatedFunction>, Component> {
        let id_arg = || Arg::Component(Component::literal(self.id.as_str()));
        let Some(arguments) = arguments else {
            return Err(Component::translatable_with("commands.function.error.missing_arguments", vec![id_arg()]));
        };
        let mut parameter_values = Vec::with_capacity(self.parameters.len());
        for argument in &self.parameters {
            let Some(value) = arguments.get(argument) else {
                return Err(Component::translatable_with("commands.function.error.missing_argument", vec![id_arg(), Arg::String(argument.clone())]));
            };
            parameter_values.push(stringify(value));
        }
        let mut cache = self.cache.borrow_mut();
        if let Some(index) = cache.iter().position(|(values, _)| *values == parameter_values) {
            let entry = cache.remove(index);
            let function = entry.1.clone();
            cache.push(entry);
            return Ok(function);
        }
        if cache.len() >= MAX_CACHE_ENTRIES {
            cache.remove(0);
        }
        let function = Arc::new(self.substitute_and_parse(&parameter_values)?);
        cache.push((parameter_values, function.clone()));
        Ok(function)
    }

    fn substitute_and_parse(&self, values: &[String]) -> Result<InstantiatedFunction, Component> {
        let mut entries = Vec::with_capacity(self.entries.len());
        for entry in &self.entries {
            entries.push(match entry {
                Entry::PlainText(command) => command.clone(),
                Entry::Macro(entry) => {
                    let substitutions: Vec<&str> = entry.parameters.iter().map(|&i| values[i].as_str()).collect();
                    let command = entry.template.substitute(&substitutions).map_err(|e| crate::commands::unexpected_error(&e).message)?;
                    let units: Vec<u16> = command.encode_utf16().collect();
                    parse_command(&entry.compilation_context, StringReader::new(&units)).map_err(|e| {
                        Component::translatable_with(
                            "commands.function.error.parse",
                            vec![Arg::Component(Component::literal(self.id.as_str())), Arg::String(command.clone()), Arg::String(e.get_message())],
                        )
                    })?
                }
            });
        }
        let hash = self.parameters.iter().fold(1i32, |h, p| h.wrapping_mul(31).wrapping_add(java_string_hash(p)));
        Ok(PlainTextFunction { id: format!("{}/{hash}", self.id), entries })
    }
}

fn stringify(tag: &Nbt) -> String {
    match tag {
        Nbt::Float(v) => format_decimal(*v as f64),
        Nbt::Double(v) => format_decimal(*v),
        Nbt::Byte(v) => v.to_string(),
        Nbt::Short(v) => v.to_string(),
        Nbt::Long(v) => v.to_string(),
        Nbt::String(v) => v.clone(),
        other => crate::nbt::to_string(other),
    }
}

pub fn format_decimal(value: f64) -> String {
    if value.is_nan() {
        return "NaN".into();
    }
    let negative = value.is_sign_negative();
    if value.is_infinite() {
        return if negative { "-\u{221E}" } else { "\u{221E}" }.into();
    }
    let shortest = format!("{:e}", value.abs());
    let (mantissa, exponent) = shortest.split_once('e').unwrap();
    let mut digits: Vec<u8> = mantissa.bytes().filter(u8::is_ascii_digit).map(|b| b - b'0').collect();
    let mut point = exponent.parse::<i32>().unwrap() + 1;
    let keep = point + 15;
    if keep < 0 {
        digits.clear();
    } else if (keep as usize) < digits.len() {
        let rest = &digits[keep as usize..];
        let half_or_more = rest[0] >= 5;
        let tie = rest[0] == 5 && rest[1..].iter().all(|&d| d == 0);
        let odd = keep > 0 && digits[keep as usize - 1] % 2 == 1;
        let round_up = if tie { odd } else { half_or_more };
        digits.truncate(keep as usize);
        if round_up {
            let mut i = digits.len();
            loop {
                if i == 0 {
                    digits.insert(0, 1);
                    point += 1;
                    break;
                }
                i -= 1;
                if digits[i] == 9 {
                    digits[i] = 0;
                } else {
                    digits[i] += 1;
                    break;
                }
            }
        }
    }
    while digits.last() == Some(&0) {
        digits.pop();
    }
    let mut out = String::new();
    if negative {
        out.push('-');
    }
    let digit = |i: i32| if i >= 0 && (i as usize) < digits.len() { (b'0' + digits[i as usize]) as char } else { '0' };
    (0..point.max(0)).for_each(|i| out.push(digit(i)));
    let fraction_end = digits.len() as i32;
    if fraction_end > point {
        out.push('.');
        (point..fraction_end).for_each(|i| out.push(digit(i)));
    }
    if out.is_empty() || out == "-" {
        out.push('0');
    }
    out
}
