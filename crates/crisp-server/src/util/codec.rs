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

pub fn number(tag: &Nbt) -> DataResult<&Nbt> {
    match tag {
        Nbt::Byte(_) | Nbt::Short(_) | Nbt::Int(_) | Nbt::Long(_) | Nbt::Float(_) | Nbt::Double(_) => Ok(tag),
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
