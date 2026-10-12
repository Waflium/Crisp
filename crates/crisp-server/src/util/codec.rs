use std::cell::Cell;

use crisp_protocol::nbt::Nbt;

use crate::world::item::component::codec as component_codec;

pub type DataResult<T> = Result<T, String>;

pub fn field<'a>(tag: &'a Nbt, name: &str) -> DataResult<&'a Nbt> {
    map(tag)?;
    tag.get(name).ok_or_else(|| format!("No key {name} in MapLike[{}]", crate::nbt::to_string(tag)))
}

pub fn optional_field<'a>(tag: &'a Nbt, name: &str) -> Option<&'a Nbt> {
    tag.get(name)
}

pub fn decode_optional<T>(tag: &Nbt, name: &str, decode: impl FnOnce(&Nbt) -> DataResult<T>) -> DataResult<Option<T>> {
    optional_field(tag, name).map(decode).transpose()
}

pub fn decode_or<T>(tag: &Nbt, name: &str, default: T, decode: impl FnOnce(&Nbt) -> DataResult<T>) -> DataResult<T> {
    Ok(decode_optional(tag, name, decode)?.unwrap_or(default))
}

pub fn decode_field<T>(tag: &Nbt, name: &str, decode: impl FnOnce(&Nbt) -> DataResult<T>) -> DataResult<T> {
    decode(field(tag, name)?)
}

pub fn map(tag: &Nbt) -> DataResult<&[(String, Nbt)]> {
    component_codec::map(tag)
}

thread_local! {
    static NBT_OPS: Cell<bool> = const { Cell::new(false) };
}

pub fn with_nbt_ops<R>(decode: impl FnOnce() -> R) -> R {
    let previous = NBT_OPS.with(|nbt_ops| nbt_ops.replace(true));
    let result = decode();
    NBT_OPS.with(|nbt_ops| nbt_ops.set(previous));
    result
}

pub fn is_nbt_ops() -> bool {
    NBT_OPS.with(Cell::get)
}

pub fn number(tag: &Nbt) -> DataResult<&Nbt> {
    match tag {
        Nbt::Byte(_) | Nbt::Short(_) | Nbt::Int(_) | Nbt::Long(_) | Nbt::Float(_) | Nbt::Double(_) => Ok(tag),
        _ if is_nbt_ops() => Err("Not a number".to_string()),
        _ => Err(format!("Not a number: {}", crate::nbt::to_string(tag))),
    }
}

pub fn bool_value(tag: &Nbt) -> DataResult<bool> {
    Ok(component_codec::as_i64(number(tag)?) != 0)
}

pub fn int(tag: &Nbt) -> DataResult<i32> {
    Ok(component_codec::as_i32(number(tag)?))
}

pub fn long(tag: &Nbt) -> DataResult<i64> {
    Ok(component_codec::as_i64(number(tag)?))
}

pub fn float(tag: &Nbt) -> DataResult<f32> {
    Ok(component_codec::as_f32(number(tag)?))
}

pub fn double(tag: &Nbt) -> DataResult<f64> {
    Ok(component_codec::as_f64(number(tag)?))
}

pub fn string(tag: &Nbt) -> DataResult<&str> {
    component_codec::string(tag)
}

pub fn identifier(tag: &Nbt) -> DataResult<String> {
    component_codec::identifier(tag)
}

pub fn list(tag: &Nbt) -> DataResult<Vec<Nbt>> {
    component_codec::list(tag)
}

pub fn list_of<T>(tag: &Nbt, decode: impl Fn(&Nbt) -> DataResult<T>) -> DataResult<Vec<T>> {
    list(tag)?.iter().map(decode).collect()
}

pub fn compact_list_of<T>(tag: &Nbt, decode: impl Fn(&Nbt) -> DataResult<T>) -> DataResult<Vec<T>> {
    match tag {
        Nbt::List(_) | Nbt::ByteArray(_) | Nbt::IntArray(_) | Nbt::LongArray(_) => list_of(tag, decode),
        single => Ok(vec![decode(single)?]),
    }
}

pub fn either<A, B>(tag: &Nbt, first: impl FnOnce(&Nbt) -> DataResult<A>, second: impl FnOnce(&Nbt) -> DataResult<B>) -> DataResult<Result<A, B>> {
    match first(tag) {
        Ok(a) => Ok(Ok(a)),
        Err(first_error) => match second(tag) {
            Ok(b) => Ok(Err(b)),
            Err(second_error) => Err(format!("Failed to parse either. First: {first_error}; Second: {second_error}")),
        },
    }
}

pub struct DecodeError {
    pub message: String,
    pub partial: bool,
}

pub fn either_or_partial<A, B>(
    tag: &Nbt,
    first: impl FnOnce(&Nbt) -> DataResult<A>,
    second: impl FnOnce(&Nbt) -> Result<B, DecodeError>,
) -> DataResult<Result<A, B>> {
    match first(tag) {
        Ok(a) => Ok(Ok(a)),
        Err(first_error) => match second(tag) {
            Ok(b) => Ok(Err(b)),
            Err(DecodeError { message, partial: true }) => Err(message),
            Err(DecodeError { message, .. }) => Err(format!("Failed to parse either. First: {first_error}; Second: {message}")),
        },
    }
}

pub fn holder_set<T>(tag: &Nbt, registry: &str, element: impl Fn(&Nbt) -> DataResult<T>) -> Result<Vec<T>, DecodeError> {
    let tag_error = match tag {
        Nbt::String(name) => match name.strip_prefix('#') {
            Some(location) => {
                let location = crate::resources::identifier::read(location).map_err(|message| DecodeError { message, partial: false })?;
                return Err(DecodeError { message: format!("Missing tag: '{location}' in '{registry}'"), partial: false });
            }
            None => "Not a tag id".to_string(),
        },
        _ => "Not a string".to_string(),
    };
    let compact_error = match tag {
        Nbt::List(_) | Nbt::ByteArray(_) | Nbt::IntArray(_) | Nbt::LongArray(_) => {
            let mut elements = Vec::new();
            let mut errors: Vec<String> = Vec::new();
            for value in list(tag).unwrap_or_default().iter() {
                match element(value) {
                    Ok(value) => elements.push(value),
                    Err(error) => errors.push(error),
                }
            }
            if errors.is_empty() {
                return Ok(elements);
            }
            errors.reverse();
            return Err(DecodeError { message: errors.join("; "), partial: true });
        }
        single => match element(single) {
            Ok(value) => return Ok(vec![value]),
            Err(error) => format!("Failed to parse either. First: Not a list: {}; Second: {error}", crate::nbt::to_string(single)),
        },
    };
    Err(DecodeError { message: format!("Failed to parse either. First: {tag_error}; Second: {compact_error}"), partial: false })
}

pub fn string_enum<T: Copy>(tag: &Nbt, values: &[(&str, T)]) -> DataResult<T> {
    let name = string(tag)?;
    values.iter().find(|(n, _)| *n == name).map(|(_, v)| *v).ok_or_else(|| format!("Unknown element name:{name}"))
}

pub fn to_json(tag: &Nbt) -> serde_json::Value {
    use serde_json::Value;
    match tag {
        Nbt::Byte(v) => Value::from(*v),
        Nbt::Short(v) => Value::from(*v),
        Nbt::Int(v) => Value::from(*v),
        Nbt::Long(v) => Value::from(*v),
        Nbt::Float(v) => Value::from(*v),
        Nbt::Double(v) => Value::from(*v),
        Nbt::String(s) => Value::String(s.clone()),
        Nbt::ByteArray(a) => Value::Array(a.iter().map(|&v| Value::from(v)).collect()),
        Nbt::IntArray(a) => Value::Array(a.iter().map(|&v| Value::from(v)).collect()),
        Nbt::LongArray(a) => Value::Array(a.iter().map(|&v| Value::from(v)).collect()),
        Nbt::List(items) => Value::Array(items.iter().map(to_json).collect()),
        Nbt::Compound(fields) => Value::Object(fields.iter().map(|(k, v)| (k.clone(), to_json(v))).collect()),
    }
}

pub fn unbounded_map<K, V>(tag: &Nbt, key: impl Fn(&str) -> DataResult<K>, value: impl Fn(&Nbt) -> DataResult<V>) -> DataResult<Vec<(K, V)>> {
    map(tag)?.iter().map(|(k, v)| Ok((key(k)?, value(v)?))).collect()
}
