use serde_json::{Map, Value};

use crate::commands::arguments::java_uuid_from_string;
use crate::world::level::storage::format_uuid;

pub type DecodeResult<T> = Result<T, Vec<String>>;

pub fn message(errors: Vec<String>) -> String {
    errors.into_iter().rev().collect::<Vec<_>>().join("; ")
}

pub fn string(value: &Value) -> DecodeResult<String> {
    value.as_str().map(str::to_string).ok_or_else(|| vec![format!("Not a string: {value}")])
}

pub fn boolean(value: &Value) -> DecodeResult<bool> {
    value.as_bool().ok_or_else(|| vec![format!("Not a boolean: {value}")])
}

pub fn int(value: &Value) -> DecodeResult<i32> {
    match value {
        Value::Number(n) => Ok(if let Some(v) = n.as_i64() {
            v as i32
        } else if let Some(v) = n.as_u64() {
            v as i32
        } else {
            n.as_f64().unwrap_or(0.0).trunc() as i128 as i32
        }),
        _ => Err(vec![format!("Not a number: {value}")]),
    }
}

pub fn object(value: &Value) -> DecodeResult<&Map<String, Value>> {
    value.as_object().ok_or_else(|| vec![format!("Not a JSON object: {value}")])
}

pub fn list<T>(value: &Value, element: impl Fn(&Value) -> DecodeResult<T>) -> DecodeResult<Vec<T>> {
    let Value::Array(items) = value else { return Err(vec![format!("Not a json array: {value}")]) };
    let mut result = Vec::with_capacity(items.len());
    let mut errors = Vec::new();
    for item in items {
        match element(item) {
            Ok(decoded) => result.push(decoded),
            Err(e) => errors.extend(e),
        }
    }
    if errors.is_empty() { Ok(result) } else { Err(errors) }
}

pub struct Fields<'a> {
    object: &'a Map<String, Value>,
    errors: Vec<String>,
}

impl<'a> Fields<'a> {
    pub fn of(value: &'a Value) -> DecodeResult<Fields<'a>> {
        Ok(Fields { object: object(value)?, errors: Vec::new() })
    }

    pub fn required<T>(&mut self, name: &str, decode: impl Fn(&Value) -> DecodeResult<T>) -> Option<T> {
        match self.object.get(name) {
            Some(value) => self.record(decode(value)),
            None => {
                self.errors.push(format!("No key {name} in MapLike[{}]", Value::Object(self.object.clone())));
                None
            }
        }
    }

    pub fn optional<T>(&mut self, name: &str, decode: impl Fn(&Value) -> DecodeResult<T>) -> Option<T> {
        let value = self.object.get(name)?;
        self.record(decode(value))
    }

    pub fn lenient<T>(&mut self, name: &str, decode: impl Fn(&Value) -> DecodeResult<T>) -> Option<T> {
        self.object.get(name).and_then(|value| decode(value).ok())
    }

    fn record<T>(&mut self, result: DecodeResult<T>) -> Option<T> {
        match result {
            Ok(value) => Some(value),
            Err(e) => {
                self.errors.extend(e);
                None
            }
        }
    }

    pub fn finish<T>(self, build: impl FnOnce() -> T) -> DecodeResult<T> {
        if self.errors.is_empty() { Ok(build()) } else { Err(self.errors) }
    }
}

pub fn uuid(value: &Value) -> DecodeResult<u128> {
    let s = string(value)?;
    java_uuid_from_string(&s).ok_or_else(|| {
        let reason = if s.len() > 36 { "UUID string too large".to_string() } else { format!("Invalid UUID string: {s}") };
        vec![format!("Invalid UUID {s}: {reason}")]
    })
}

pub fn encode_uuid(id: u128) -> Value {
    Value::String(format_uuid(id))
}

pub fn enum_by_name<T>(value: &Value, by_name: impl Fn(&str) -> Option<T>) -> DecodeResult<T> {
    let name = string(value)?;
    by_name(&name).ok_or_else(|| vec![format!("Unknown element name:{name}")])
}

pub fn instant(value: &Value) -> DecodeResult<i64> {
    let text = string(value)?;
    parse_iso_instant(&text).map_err(|index| vec![format!("Text '{text}' could not be parsed at index {index}")])
}

fn parse_iso_instant(text: &str) -> Result<i64, usize> {
    let bytes = text.as_bytes();
    let mut pos = 0;
    let digits = |pos: &mut usize, min: usize, max: usize| -> Result<i64, usize> {
        let start = *pos;
        while *pos < bytes.len() && *pos - start < max && bytes[*pos].is_ascii_digit() {
            *pos += 1;
        }
        if *pos - start < min {
            return Err(start);
        }
        Ok(text[start..*pos].parse().unwrap())
    };
    let literal = |pos: &mut usize, c: u8| -> Result<(), usize> {
        if bytes.get(*pos).is_some_and(|b| b.eq_ignore_ascii_case(&c)) {
            *pos += 1;
            Ok(())
        } else {
            Err(*pos)
        }
    };
    let negative = match bytes.first() {
        Some(b'-') => {
            pos += 1;
            true
        }
        Some(b'+') => {
            pos += 1;
            false
        }
        _ => false,
    };
    let year = digits(&mut pos, 4, 10)? * if negative { -1 } else { 1 };
    literal(&mut pos, b'-')?;
    let month = digits(&mut pos, 2, 2)?;
    literal(&mut pos, b'-')?;
    let day = digits(&mut pos, 2, 2)?;
    literal(&mut pos, b'T')?;
    let hour = digits(&mut pos, 2, 2)?;
    literal(&mut pos, b':')?;
    let minute = digits(&mut pos, 2, 2)?;
    literal(&mut pos, b':')?;
    let second = digits(&mut pos, 2, 2)?;
    let mut nanos = 0i64;
    if bytes.get(pos) == Some(&b'.') {
        pos += 1;
        let start = pos;
        let fraction = digits(&mut pos, 0, 9)?;
        nanos = fraction * 10i64.pow(9 - (pos - start) as u32);
    }
    let offset_seconds = match bytes.get(pos) {
        Some(b'Z') | Some(b'z') => {
            pos += 1;
            0
        }
        Some(sign @ (b'+' | b'-')) => {
            let sign = if *sign == b'-' { -1 } else { 1 };
            pos += 1;
            let hours = digits(&mut pos, 2, 2)?;
            literal(&mut pos, b':')?;
            let minutes = digits(&mut pos, 2, 2)?;
            sign * (hours * 3600 + minutes * 60)
        }
        _ => return Err(pos),
    };
    if pos != bytes.len() {
        return Err(pos);
    }
    let date = chrono::NaiveDate::from_ymd_opt(year as i32, month as u32, day as u32).ok_or(0usize)?;
    let time = chrono::NaiveTime::from_hms_nano_opt(hour as u32, minute as u32, second as u32, nanos as u32).ok_or(0usize)?;
    let seconds = date.and_time(time).and_utc().timestamp() - offset_seconds;
    Ok(seconds * 1000 + nanos / 1_000_000)
}

pub fn encode_instant(millis: i64) -> Value {
    let time = chrono::DateTime::from_timestamp_millis(millis).unwrap_or_default();
    let fraction = millis.rem_euclid(1000);
    let base = time.format("%Y-%m-%dT%H:%M:%S").to_string();
    Value::String(if fraction == 0 { format!("{base}Z") } else { format!("{base}.{fraction:03}Z") })
}
