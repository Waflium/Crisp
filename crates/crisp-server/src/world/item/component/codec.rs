use crisp_protocol::nbt::Nbt;
use crisp_protocol::{Reader, Writer};

use crate::commands::arguments::parse_identifier;
use crate::nbt;
use crate::util::hash_ops::{self, HashCode};

pub trait Custom: Send + Sync {
    fn decode(&self, tag: &Nbt) -> Result<Nbt, String>;
    fn hash(&self, value: &Nbt) -> HashCode;
    fn write(&self, value: &Nbt, w: &mut Writer) -> Result<(), String>;
    fn read(&self, r: &mut Reader) -> Result<Nbt, String>;
}

#[derive(Clone, Copy)]
pub enum RangeError {
    Within,
    NonNegative,
    Positive,
    Outside,
}

#[derive(Clone)]
pub enum Kind {
    Required,
    Optional,
    Default(Nbt),
    Lenient(Nbt),
    LenientOptional,
    AlwaysPresent(Nbt),
    Plain,
}

pub struct Field {
    pub name: &'static str,
    pub codec: Codec,
    pub stream: Stream,
    pub kind: Kind,
}

pub enum Selector {
    Registry(&'static str),
    Enum,
}

pub struct Dispatch {
    pub key: &'static str,
    pub selector: Selector,
    pub cases: Vec<(&'static str, Vec<Field>)>,
}

pub enum Codec {
    Unit,
    Bool,
    Byte,
    UnsignedByte,
    Short,
    Int(i32, i32, RangeError),
    Long(i64, i64, RangeError),
    Float(f32, f32, bool, RangeError),
    Double,
    String(usize, usize),
    Identifier,
    Element(&'static str),
    Holder(&'static str, Option<Box<Codec>>),
    HolderSet(&'static str),
    Enum(&'static [&'static str]),
    IdEnum(&'static [(&'static str, i32)]),
    Lazy(fn() -> &'static (Codec, Stream)),
    Normalize(Box<Codec>, fn(Nbt) -> Result<Nbt, String>),
    List(Box<Codec>, usize, usize),
    Compact(Box<Codec>),
    Map(Box<Codec>, Box<Codec>),
    Record(Vec<Field>),
    Dispatch(Box<Dispatch>),
    Either(Box<Codec>, Box<Codec>),
    Alt(Box<Codec>, Box<Codec>, fn(&Nbt) -> Result<Nbt, String>),
    Compound,
    AnyTag,
    Uuid,
    Custom(Box<dyn Custom>),
}

pub enum Stream {
    Unit,
    Bool,
    Byte,
    Short,
    VarInt,
    Int,
    VarLong,
    Long,
    Float,
    Double,
    Utf8(usize),
    Identifier,
    Registry(&'static str),
    Holder(&'static str, Option<Box<Stream>>),
    HolderSet(&'static str),
    Enum,
    List(Box<Stream>, usize),
    FixedList(Box<Stream>, usize),
    Map(Box<Stream>, Box<Stream>, usize),
    Record,
    Dispatch,
    Either(Box<Stream>, Box<Stream>),
    Nbt,
    Uuid,
    Custom,
    Lazy,
}

impl Field {
    pub fn new(name: &'static str, codec: Codec, stream: Stream, kind: Kind) -> Field {
        Field { name, codec, stream, kind }
    }
}

fn float_text(v: f32) -> String {
    nbt::java_float_to_string(v)
}

fn range_error<T: std::fmt::Display>(kind: RangeError, value: String, min: T, max: T) -> String {
    match kind {
        RangeError::Within => format!("Value must be within range [{min};{max}]: {value}"),
        RangeError::NonNegative => format!("Value must be non-negative: {value}"),
        RangeError::Positive => format!("Value must be positive: {value}"),
        RangeError::Outside => format!("Value {value} outside of range [{min}:{max}]"),
    }
}

fn number(tag: &Nbt) -> Result<&Nbt, String> {
    match tag {
        Nbt::Byte(_) | Nbt::Short(_) | Nbt::Int(_) | Nbt::Long(_) | Nbt::Float(_) | Nbt::Double(_) => Ok(tag),
        _ => Err("Not a number".into()),
    }
}

pub fn as_i64(tag: &Nbt) -> i64 {
    match *tag {
        Nbt::Byte(v) => v as i64,
        Nbt::Short(v) => v as i64,
        Nbt::Int(v) => v as i64,
        Nbt::Long(v) => v,
        Nbt::Float(v) => v as i64,
        Nbt::Double(v) => v as i64,
        _ => 0,
    }
}

pub fn as_i32(tag: &Nbt) -> i32 {
    match *tag {
        Nbt::Float(v) => v as i32,
        Nbt::Double(v) => v as i32,
        _ => as_i64(tag) as i32,
    }
}

pub fn as_f64(tag: &Nbt) -> f64 {
    match *tag {
        Nbt::Float(v) => v as f64,
        Nbt::Double(v) => v,
        _ => as_i64(tag) as f64,
    }
}

pub fn as_f32(tag: &Nbt) -> f32 {
    match *tag {
        Nbt::Float(v) => v,
        _ => as_f64(tag) as f32,
    }
}

pub fn string(tag: &Nbt) -> Result<&str, String> {
    match tag {
        Nbt::String(s) => Ok(s),
        _ => Err("Not a string".into()),
    }
}

pub fn map(tag: &Nbt) -> Result<&[(String, Nbt)], String> {
    match tag {
        Nbt::Compound(fields) => Ok(fields),
        other => Err(format!("Not a map: {}", nbt::to_string(other))),
    }
}

pub fn list(tag: &Nbt) -> Result<Vec<Nbt>, String> {
    match tag {
        Nbt::List(items) => Ok(items.clone()),
        Nbt::ByteArray(v) => Ok(v.iter().map(|&b| Nbt::Byte(b)).collect()),
        Nbt::IntArray(v) => Ok(v.iter().map(|&b| Nbt::Int(b)).collect()),
        Nbt::LongArray(v) => Ok(v.iter().map(|&b| Nbt::Long(b)).collect()),
        other => Err(format!("Not a list: {}", nbt::to_string(other))),
    }
}

pub fn identifier(tag: &Nbt) -> Result<String, String> {
    crate::resources::identifier::read(string(tag)?)
}

pub fn registry(name: &str) -> &'static [&'static str] {
    crisp_data::registry_entries(name).unwrap_or(&[])
}

pub fn registry_id(name: &str, entry: &str) -> Result<i32, String> {
    registry(name).iter().position(|e| *e == entry).map(|i| i as i32).ok_or_else(|| format!("Can't find id for '{entry}' in registry {name}"))
}

fn registry_entry(name: &str, id: i32) -> Result<&'static str, String> {
    registry(name).get(id as usize).copied().ok_or_else(|| format!("No value with id {id} in registry {name}"))
}

pub fn tag_exists(registry: &str, tag: &str) -> bool {
    crisp_data::tags().iter().any(|r| r.registry == registry && r.tags.iter().any(|t| t.0 == tag))
}

fn missing_key(name: &str, input: &Nbt) -> String {
    format!("No key {name} in MapLike[{}]", nbt::to_string(input))
}

fn join(errors: Vec<String>) -> Result<(), String> {
    if errors.is_empty() { Ok(()) } else { Err(errors.join("; ")) }
}

impl Codec {
    pub fn list(elem: Codec) -> Codec {
        Codec::List(Box::new(elem), 0, i32::MAX as usize)
    }

    pub fn list_max(elem: Codec, max: usize) -> Codec {
        Codec::List(Box::new(elem), 0, max)
    }

    pub fn int() -> Codec {
        Codec::Int(i32::MIN, i32::MAX, RangeError::Within)
    }

    pub fn float() -> Codec {
        Codec::Float(f32::MIN, f32::MAX, false, RangeError::Within)
    }

    pub fn string() -> Codec {
        Codec::String(0, i32::MAX as usize)
    }

    pub fn long() -> Codec {
        Codec::Long(i64::MIN, i64::MAX, RangeError::Within)
    }

    pub fn decode(&self, tag: &Nbt) -> Result<Nbt, String> {
        match self {
            Codec::Unit => map(tag).map(|_| Nbt::Compound(Vec::new())),
            Codec::Bool => number(tag).map(|n| Nbt::Byte((as_f64(n) != 0.0) as i8)),
            Codec::Byte => number(tag).map(|n| Nbt::Byte(as_i64(n) as i8)),
            Codec::UnsignedByte => number(tag).map(|n| Nbt::Byte(as_i64(n) as i8)),
            Codec::Short => number(tag).map(|n| Nbt::Short(as_i64(n) as i16)),
            Codec::Int(min, max, err) => {
                let v = as_i32(number(tag)?);
                if (*min..=*max).contains(&v) { Ok(Nbt::Int(v)) } else { Err(range_error(*err, v.to_string(), min, max)) }
            }
            Codec::Long(min, max, err) => {
                let v = as_i64(number(tag)?);
                if (*min..=*max).contains(&v) { Ok(Nbt::Long(v)) } else { Err(range_error(*err, v.to_string(), min, max)) }
            }
            Codec::Float(min, max, exclusive, err) => {
                let v = as_f32(number(tag)?);
                let ok = if *exclusive { v > *min && v <= *max } else { v >= *min && v <= *max };
                if ok { Ok(Nbt::Float(v)) } else { Err(range_error(*err, float_text(v), float_text(*min), float_text(*max))) }
            }
            Codec::Double => number(tag).map(|n| Nbt::Double(as_f64(n))),
            Codec::String(min, max) => {
                let s = string(tag)?;
                let len = s.encode_utf16().count();
                if len < *min {
                    Err(format!("String \"{s}\" is too short: {len} < {min}"))
                } else if len > *max {
                    Err(format!("String \"{s}\" is too long: {len} > {max}"))
                } else {
                    Ok(Nbt::String(s.into()))
                }
            }
            Codec::Identifier => identifier(tag).map(Nbt::String),
            Codec::Element(reg) => {
                let id = identifier(tag)?;
                if registry(reg).contains(&id.as_str()) {
                    Ok(Nbt::String(id))
                } else {
                    Err(format!("Unknown registry key in ResourceKey[minecraft:root / {reg}]: {id}"))
                }
            }
            Codec::Holder(reg, direct) => match (tag, direct) {
                (Nbt::String(_), _) => {
                    let id = identifier(tag)?;
                    if registry(reg).contains(&id.as_str()) { Ok(Nbt::String(id)) } else { Err(format!("Failed to get element {id} from registry {reg}")) }
                }
                (_, Some(direct)) => direct.decode(tag),
                (_, None) => Err("Not a string".into()),
            },
            Codec::HolderSet(reg) => decode_holder_set(reg, tag),
            Codec::Enum(names) => {
                let s = string(tag)?;
                if names.contains(&s) { Ok(Nbt::String(s.into())) } else { Err(format!("Unknown element name:{s}")) }
            }
            Codec::IdEnum(names) => {
                let s = string(tag)?;
                if names.iter().any(|(n, _)| *n == s) { Ok(Nbt::String(s.into())) } else { Err(format!("Unknown element name:{s}")) }
            }
            Codec::Lazy(pair) => pair().0.decode(tag),
            Codec::Normalize(inner, f) => f(inner.decode(tag)?),
            Codec::List(elem, min, max) => {
                let out = decode_list(elem, tag)?;
                if out.len() > *max {
                    return Err(format!("List is too long: {} > {max}", out.len()));
                }
                if out.len() < *min {
                    return Err(if *min == 1 { "List must have contents".into() } else { format!("List is too short: {} < {min}", out.len()) });
                }
                Ok(Nbt::List(out))
            }
            Codec::Compact(elem) => match decode_list(elem, tag) {
                Ok(mut items) if items.len() == 1 => Ok(items.pop().unwrap()),
                Ok(items) => Ok(Nbt::List(items)),
                Err(first) => elem.decode(tag).map_err(|second| format!("Failed to parse either. First: {first}; Second: {second}")),
            },
            Codec::Map(key, value) => {
                let fields = map(tag)?;
                let mut out = Vec::with_capacity(fields.len());
                let mut errors = Vec::new();
                for (k, v) in fields {
                    match (key.decode(&Nbt::String(k.clone())), value.decode(v)) {
                        (Ok(Nbt::String(k)), Ok(v)) => out.push((k, v)),
                        (Err(e), _) | (_, Err(e)) => errors.push(e),
                        (Ok(_), Ok(_)) => errors.push("key is not a string".into()),
                    }
                }
                join(errors)?;
                Ok(Nbt::Compound(out))
            }
            Codec::Record(fields) => decode_record(fields, tag),
            Codec::Dispatch(d) => decode_dispatch(d, tag),
            Codec::Either(a, b) => a.decode(tag).or_else(|first| b.decode(tag).map_err(|second| format!("Failed to parse either. First: {first}; Second: {second}"))),
            Codec::Alt(a, b, convert) => a
                .decode(tag)
                .or_else(|first| b.decode(tag).and_then(|v| convert(&v)).map_err(|second| format!("Failed to parse either. First: {first}; Second: {second}"))),
            Codec::Compound => map(tag).map(|_| tag.clone()),
            Codec::AnyTag => Ok(tag.clone()),
            Codec::Uuid => decode_uuid(tag),
            Codec::Custom(c) => c.decode(tag),
        }
    }

    pub fn hash(&self, value: &Nbt) -> HashCode {
        match self {
            Codec::Unit => hash_ops::create_map(Vec::new()),
            Codec::Bool => hash_ops::create_boolean(as_i64(value) != 0),
            Codec::Holder(_, Some(direct)) if !matches!(value, Nbt::String(_)) => direct.hash(value),
            Codec::List(elem, _, _) => hash_ops::create_list(value.as_list().iter().map(|v| elem.hash(v))),
            Codec::Compact(elem) => match value {
                Nbt::List(items) if !elem.accepts(value) => hash_ops::create_list(items.iter().map(|v| elem.hash(v))),
                single => elem.hash(single),
            },
            Codec::Map(key, v) => match value {
                Nbt::Compound(fields) => hash_ops::create_map(fields.iter().map(|(k, x)| (key.hash(&Nbt::String(k.clone())), v.hash(x))).collect()),
                other => hash_ops::hash_nbt(other),
            },
            Codec::Record(fields) => hash_record(fields, value, None),
            Codec::Dispatch(d) => {
                let case = value.get(d.key).and_then(Nbt::as_str).and_then(|t| d.cases.iter().find(|(n, _)| *n == t));
                match case {
                    Some((_, fields)) => hash_record(fields, value, Some(d.key)),
                    None => hash_ops::hash_nbt(value),
                }
            }
            Codec::Either(a, b) | Codec::Alt(a, b, _) => {
                if matches!(self, Codec::Alt(..)) || a.accepts(value) {
                    a.hash(value)
                } else {
                    b.hash(value)
                }
            }
            Codec::Uuid => match value {
                Nbt::IntArray(v) => hash_ops::create_int_list(v),
                other => hash_ops::hash_nbt(other),
            },
            Codec::Custom(c) => c.hash(value),
            Codec::Lazy(pair) => pair().0.hash(value),
            Codec::Normalize(inner, _) => inner.hash(value),
            _ => hash_ops::hash_nbt(value),
        }
    }

    pub fn accepts(&self, value: &Nbt) -> bool {
        self.decode(value).is_ok()
    }

    fn enum_id(&self, name: &str) -> Option<i32> {
        match self {
            Codec::Enum(names) => names.iter().position(|n| *n == name).map(|i| i as i32),
            Codec::IdEnum(names) => names.iter().find(|(n, _)| *n == name).map(|(_, id)| *id),
            _ => None,
        }
    }

    fn enum_name(&self, id: i32) -> &'static str {
        match self {
            Codec::Enum(names) => names.get(id as usize).or(names.first()).copied().unwrap_or(""),
            Codec::IdEnum(names) => names.iter().find(|(_, i)| *i == id).or(names.first()).map_or("", |(n, _)| n),
            _ => "",
        }
    }
}

fn decode_list(elem: &Codec, tag: &Nbt) -> Result<Vec<Nbt>, String> {
    let items = list(tag)?;
    let mut out = Vec::with_capacity(items.len());
    let mut errors = Vec::new();
    for item in &items {
        match elem.decode(item) {
            Ok(v) => out.push(v),
            Err(e) => errors.push(e),
        }
    }
    join(errors)?;
    Ok(out)
}

fn decode_holder_set(reg: &'static str, tag: &Nbt) -> Result<Nbt, String> {
    let element = Codec::Element(reg);
    match tag {
        Nbt::String(s) if s.starts_with('#') => {
            let id = identifier(&Nbt::String(s[1..].into())).map_err(|e| format!("Failed to parse either. First: {e}; Second: Not a list"))?;
            if tag_exists(reg, &id) { Ok(Nbt::String(format!("#{id}"))) } else { Err(format!("Missing tag: '{id}' in '{reg}'")) }
        }
        Nbt::String(_) => element.decode(tag),
        _ => {
            let items = list(tag).map_err(|e| format!("Failed to parse either. First: Not a string; Second: {e}"))?;
            let mut out = Vec::new();
            for item in &items {
                out.push(element.decode(item)?);
            }
            if out.len() == 1 { Ok(out.pop().unwrap()) } else { Ok(Nbt::List(out)) }
        }
    }
}

fn decode_uuid(tag: &Nbt) -> Result<Nbt, String> {
    match tag {
        Nbt::IntArray(v) if v.len() == 4 => Ok(tag.clone()),
        Nbt::IntArray(v) => Err(format!("Input is not a list of 4 ints: {}", v.len())),
        _ => {
            let items = list(tag)?;
            if items.len() != 4 {
                return Err(format!("Input is not a list of 4 ints: {}", items.len()));
            }
            let mut out = Vec::new();
            for item in &items {
                out.push(as_i32(number(item)?));
            }
            Ok(Nbt::IntArray(out))
        }
    }
}

fn decode_record(fields: &[Field], tag: &Nbt) -> Result<Nbt, String> {
    let input = map(tag)?;
    let mut out = Vec::new();
    decode_fields(fields, tag, input, &mut out)?;
    Ok(Nbt::Compound(out))
}

fn decode_fields(fields: &[Field], tag: &Nbt, input: &[(String, Nbt)], out: &mut Vec<(String, Nbt)>) -> Result<(), String> {
    let mut errors = Vec::new();
    for field in fields {
        let present = input.iter().find(|(k, _)| k == field.name).map(|(_, v)| v);
        match (&field.kind, present) {
            (Kind::Required, None) => errors.push(missing_key(field.name, tag)),
            (Kind::AlwaysPresent(default), None) => out.push((field.name.into(), default.clone())),
            (_, None) => {}
            (kind, Some(v)) => match field.codec.decode(v) {
                Ok(v) => match kind {
                    Kind::Default(d) | Kind::Lenient(d) if *d == v => {}
                    _ => out.push((field.name.into(), v)),
                },
                Err(e) => match kind {
                    Kind::Lenient(_) | Kind::LenientOptional => {}
                    _ => errors.push(e),
                },
            },
        }
    }
    join(errors)
}

fn decode_dispatch(d: &Dispatch, tag: &Nbt) -> Result<Nbt, String> {
    let input = map(tag)?;
    let Some((_, ty)) = input.iter().find(|(k, _)| k == d.key) else { return Err(missing_key(d.key, tag)) };
    let name = match d.selector {
        Selector::Registry(reg) => {
            let id = identifier(ty)?;
            if !d.cases.iter().any(|(n, _)| *n == id) {
                return Err(format!("Unknown registry key in ResourceKey[minecraft:root / {reg}]: {id}"));
            }
            id
        }
        Selector::Enum => {
            let s = string(ty)?;
            if !d.cases.iter().any(|(n, _)| *n == s) {
                return Err(format!("Unknown element name:{s}"));
            }
            s.to_string()
        }
    };
    let (_, fields) = d.cases.iter().find(|(n, _)| *n == name).unwrap();
    let mut out = Vec::new();
    decode_fields(fields, tag, input, &mut out)?;
    out.push((d.key.to_string(), Nbt::String(name)));
    Ok(Nbt::Compound(out))
}

fn hash_record(fields: &[Field], value: &Nbt, key: Option<&str>) -> HashCode {
    let Nbt::Compound(entries) = value else { return hash_ops::hash_nbt(value) };
    let mut out = Vec::with_capacity(entries.len());
    for (k, v) in entries {
        let h = match fields.iter().find(|f| f.name == k) {
            Some(f) => f.codec.hash(v),
            None if Some(k.as_str()) == key => hash_ops::create_string(v.as_str().unwrap_or("")),
            None => hash_ops::hash_nbt(v),
        };
        out.push((hash_ops::create_string(k), h));
    }
    hash_ops::create_map(out)
}

fn elements<'a>(codec: &Codec, value: &'a Nbt) -> Vec<&'a Nbt> {
    match (codec, value) {
        (Codec::Compact(elem), Nbt::List(items)) if !elem.accepts(value) => items.iter().collect(),
        (Codec::Compact(_), single) => vec![single],
        (_, Nbt::List(items)) => items.iter().collect(),
        _ => Vec::new(),
    }
}

fn element_codec(codec: &Codec) -> &Codec {
    match codec {
        Codec::List(elem, _, _) | Codec::Compact(elem) => elem,
        other => other,
    }
}

fn write_count(w: &mut Writer, n: usize, max: usize) -> Result<(), String> {
    if n > max {
        return Err(format!("{n} elements exceeded max size of: {max}"));
    }
    w.varint(n as i32);
    Ok(())
}

fn read_count(r: &mut Reader, max: usize) -> Result<usize, String> {
    let n = r.varint().map_err(|e| e.to_string())?;
    if n as i64 > max as i64 {
        return Err(format!("{n} elements exceeded max size of: {max}"));
    }
    Ok(n.max(0) as usize)
}

macro_rules! rd {
    ($e:expr) => {
        $e.map_err(|e| e.to_string())?
    };
}

pub fn read_utf8(r: &mut Reader, max: usize) -> Result<String, String> {
    Ok(rd!(r.string(max)).to_string())
}

pub fn write(codec: &Codec, stream: &Stream, value: &Nbt, w: &mut Writer) -> Result<(), String> {
    match codec {
        Codec::Normalize(inner, _) => return write(inner, stream, value, w),
        Codec::Alt(primary, _, _) if !matches!(stream, Stream::Either(..)) => return write(primary, stream, value, w),
        _ => {}
    }
    match stream {
        Stream::Lazy => match codec {
            Codec::Lazy(pair) => {
                let (c, s) = pair();
                write(c, s, value, w)?
            }
            _ => return Err("not a recursive codec".into()),
        },
        Stream::Unit => {}
        Stream::Bool => w.bool(as_i64(value) != 0),
        Stream::Byte => w.u8(java_i64(codec, value) as u8),
        Stream::Short => w.bytes(&(java_i64(codec, value) as i16).to_be_bytes()),
        Stream::VarInt => w.varint(java_i64(codec, value) as i32),
        Stream::Int => w.i32(java_i64(codec, value) as i32),
        Stream::VarLong => w.varlong(java_i64(codec, value)),
        Stream::Long => w.i64(java_i64(codec, value)),
        Stream::Float => w.f32(as_f32(value)),
        Stream::Double => w.f64(as_f64(value)),
        Stream::Utf8(max) => {
            let s = value.as_str().unwrap_or("");
            if s.encode_utf16().count() > *max {
                return Err(format!("String too big (was {} characters, max {max})", s.len()));
            }
            w.string(s)
        }
        Stream::Identifier => w.string(value.as_str().unwrap_or("")),
        Stream::Registry(reg) => w.varint(registry_id(reg, value.as_str().unwrap_or(""))?),
        Stream::Holder(reg, direct_stream) => match value {
            Nbt::String(id) => w.varint(registry_id(reg, id)? + 1),
            direct => {
                w.varint(0);
                match (codec, direct_stream) {
                    (Codec::Holder(_, Some(inner)), Some(s)) => write(inner, s, direct, w)?,
                    _ => return Err("not a direct holder".into()),
                }
            }
        },
        Stream::HolderSet(reg) => match value {
            Nbt::String(s) if s.starts_with('#') => {
                w.varint(0);
                w.string(&s[1..]);
            }
            Nbt::String(id) => {
                w.varint(2);
                w.varint(registry_id(reg, id)?);
            }
            other => {
                let items = other.as_list();
                w.varint(items.len() as i32 + 1);
                for item in items {
                    w.varint(registry_id(reg, item.as_str().unwrap_or(""))?);
                }
            }
        },
        Stream::Enum => {
            let name = value.as_str().unwrap_or("");
            w.varint(codec.enum_id(name).ok_or_else(|| format!("Unknown element {name}"))?);
        }
        Stream::List(elem_stream, max) => {
            let items = elements(codec, value);
            write_count(w, items.len(), *max)?;
            let elem = element_codec(codec);
            for item in items {
                write(elem, elem_stream, item, w)?;
            }
        }
        Stream::FixedList(elem_stream, size) => {
            let items = elements(codec, value);
            if items.len() != *size {
                return Err(format!("Invalid list size, expected {size}, but got {}", items.len()));
            }
            let elem = element_codec(codec);
            for item in items {
                write(elem, elem_stream, item, w)?;
            }
        }
        Stream::Map(ks, vs, max) => {
            let (Codec::Map(kc, vc), Nbt::Compound(fields)) = (codec, value) else { return Err("not a map".into()) };
            write_count(w, fields.len(), *max)?;
            for i in crisp_protocol::nbt::java_hash_map_order(&fields.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>()) {
                let (k, v) = &fields[i];
                write(kc, ks, &Nbt::String(k.clone()), w)?;
                write(vc, vs, v, w)?;
            }
        }
        Stream::Record => match codec {
            Codec::Record(fields) => write_fields(fields, value, w)?,
            _ => return Err("not a record".into()),
        },
        Stream::Dispatch => {
            let Codec::Dispatch(d) = codec else { return Err("not a dispatch".into()) };
            let name = value.get(d.key).and_then(Nbt::as_str).unwrap_or("");
            let index = d.cases.iter().position(|(n, _)| *n == name).ok_or_else(|| format!("Unknown type {name}"))?;
            match d.selector {
                Selector::Registry(reg) => w.varint(registry_id(reg, name)?),
                Selector::Enum => w.varint(index as i32),
            }
            write_fields(&d.cases[index].1, value, w)?;
        }
        Stream::Either(left, right) => {
            let (Codec::Either(a, b) | Codec::Alt(a, b, _)) = codec else { return Err("not an either".into()) };
            if a.accepts(value) {
                w.bool(true);
                write(a, left, value, w)?;
            } else {
                w.bool(false);
                write(b, right, value, w)?;
            }
        }
        Stream::Nbt => value.write(w),
        Stream::Uuid => {
            let v = match value {
                Nbt::IntArray(v) if v.len() == 4 => v.clone(),
                _ => vec![0; 4],
            };
            w.i64(((v[0] as u32 as i64) << 32) | v[1] as u32 as i64);
            w.i64(((v[2] as u32 as i64) << 32) | v[3] as u32 as i64);
        }
        Stream::Custom => match codec {
            Codec::Custom(c) => c.write(value, w)?,
            _ => return Err("no custom codec".into()),
        },
    }
    Ok(())
}

fn write_fields(fields: &[Field], value: &Nbt, w: &mut Writer) -> Result<(), String> {
    for field in fields {
        let present = value.get(field.name);
        match (&field.kind, present) {
            (Kind::Optional | Kind::LenientOptional, None) => w.bool(false),
            (Kind::Optional | Kind::LenientOptional, Some(v)) => {
                w.bool(true);
                write(&field.codec, &field.stream, v, w)?;
            }
            (Kind::Default(d) | Kind::Lenient(d) | Kind::AlwaysPresent(d), v) => write(&field.codec, &field.stream, v.unwrap_or(d), w)?,
            (Kind::Required | Kind::Plain, Some(v)) => write(&field.codec, &field.stream, v, w)?,
            (Kind::Required | Kind::Plain, None) => return Err(format!("missing field {}", field.name)),
        }
    }
    Ok(())
}

fn java_i64(codec: &Codec, value: &Nbt) -> i64 {
    match (codec, value) {
        (Codec::UnsignedByte, Nbt::Byte(b)) => *b as u8 as i64,
        (Codec::Bool, v) => (as_i64(v) != 0) as i64,
        (_, v) => as_i64(v),
    }
}

fn canonical_number(codec: &Codec, v: i64) -> Nbt {
    match codec {
        Codec::Byte | Codec::UnsignedByte => Nbt::Byte(v as i8),
        Codec::Short => Nbt::Short(v as i16),
        Codec::Long(..) => Nbt::Long(v),
        Codec::Bool => Nbt::Byte((v != 0) as i8),
        Codec::Float(..) => Nbt::Float(v as f32),
        Codec::Double => Nbt::Double(v as f64),
        _ => Nbt::Int(v as i32),
    }
}

pub fn read(codec: &Codec, stream: &Stream, r: &mut Reader) -> Result<Nbt, String> {
    match codec {
        Codec::Normalize(inner, f) => return f(read(inner, stream, r)?),
        Codec::Alt(primary, _, _) if !matches!(stream, Stream::Either(..)) && !matches!(stream, Stream::Nbt) => return read(primary, stream, r),
        _ => {}
    }
    Ok(match stream {
        Stream::Lazy => match codec {
            Codec::Lazy(pair) => {
                let (c, s) = pair();
                read(c, s, r)?
            }
            _ => return Err("not a recursive codec".into()),
        },
        Stream::Unit => Nbt::Compound(Vec::new()),
        Stream::Bool => Nbt::Byte(rd!(r.bool()) as i8),
        Stream::Byte => canonical_number(codec, rd!(r.u8()) as i8 as i64),
        Stream::Short => canonical_number(codec, rd!(r.i16()) as i64),
        Stream::VarInt => canonical_number(codec, rd!(r.varint()) as i64),
        Stream::Int => canonical_number(codec, rd!(r.i32()) as i64),
        Stream::VarLong => canonical_number(codec, rd!(r.varlong())),
        Stream::Long => canonical_number(codec, rd!(r.i64())),
        Stream::Float => match codec {
            Codec::Double => Nbt::Double(rd!(r.f32()) as f64),
            _ => Nbt::Float(rd!(r.f32())),
        },
        Stream::Double => Nbt::Double(rd!(r.f64())),
        Stream::Utf8(max) => Nbt::String(read_utf8(r, *max)?),
        Stream::Identifier => {
            let s = read_utf8(r, 32767)?;
            Nbt::String(parse_identifier(&s).ok_or_else(|| format!("Not a valid resource location: {s}"))?)
        }
        Stream::Registry(reg) => Nbt::String(registry_entry(reg, rd!(r.varint()))?.into()),
        Stream::Holder(reg, direct_stream) => match rd!(r.varint()) {
            0 => match (codec, direct_stream) {
                (Codec::Holder(_, Some(inner)), Some(s)) => read(inner, s, r)?,
                _ => return Err("direct holder not supported".into()),
            },
            id => Nbt::String(registry_entry(reg, id - 1)?.into()),
        },
        Stream::HolderSet(reg) => {
            let n = rd!(r.varint()) - 1;
            if n == -1 {
                let tag = read_utf8(r, 32767)?;
                Nbt::String(format!("#{}", parse_identifier(&tag).unwrap_or(tag)))
            } else {
                let mut items = Vec::new();
                for _ in 0..n.max(0) {
                    items.push(Nbt::String(registry_entry(reg, rd!(r.varint()))?.into()));
                }
                if items.len() == 1 { items.pop().unwrap() } else { Nbt::List(items) }
            }
        }
        Stream::Enum => Nbt::String(codec.enum_name(rd!(r.varint())).into()),
        Stream::List(elem_stream, max) => {
            let n = read_count(r, *max)?;
            let elem = element_codec(codec);
            let mut items = Vec::with_capacity(n.min(65536));
            for _ in 0..n {
                items.push(read(elem, elem_stream, r)?);
            }
            match codec {
                Codec::Compact(_) if items.len() == 1 => items.pop().unwrap(),
                _ => Nbt::List(items),
            }
        }
        Stream::FixedList(elem_stream, size) => {
            let elem = element_codec(codec);
            let mut items = Vec::with_capacity(*size);
            for _ in 0..*size {
                items.push(read(elem, elem_stream, r)?);
            }
            Nbt::List(items)
        }
        Stream::Map(ks, vs, max) => {
            let Codec::Map(kc, vc) = codec else { return Err("not a map".into()) };
            let n = read_count(r, *max)?;
            let mut fields = Vec::with_capacity(n.min(65536));
            for _ in 0..n {
                let k = read(kc, ks, r)?;
                let v = read(vc, vs, r)?;
                let k = match k {
                    Nbt::String(k) => k,
                    _ => return Err("key is not a string".into()),
                };
                fields.retain(|(existing, _): &(String, Nbt)| *existing != k);
                fields.push((k, v));
            }
            Nbt::Compound(fields)
        }
        Stream::Record => match codec {
            Codec::Record(fields) => {
                let mut out = Vec::new();
                read_fields(fields, r, &mut out)?;
                Nbt::Compound(out)
            }
            _ => return Err("not a record".into()),
        },
        Stream::Dispatch => {
            let Codec::Dispatch(d) = codec else { return Err("not a dispatch".into()) };
            let id = rd!(r.varint());
            let index = match d.selector {
                Selector::Registry(reg) => {
                    let name = registry_entry(reg, id)?;
                    d.cases.iter().position(|(n, _)| *n == name).ok_or_else(|| format!("Unknown type {name}"))?
                }
                Selector::Enum => (id as usize).min(d.cases.len().saturating_sub(1)),
            };
            let (name, fields) = &d.cases[index];
            let mut out = vec![(d.key.to_string(), Nbt::String((*name).into()))];
            read_fields(fields, r, &mut out)?;
            Nbt::Compound(out)
        }
        Stream::Either(left, right) => {
            let (Codec::Either(a, b) | Codec::Alt(a, b, _)) = codec else { return Err("not an either".into()) };
            if rd!(r.bool()) { read(a, left, r)? } else { read(b, right, r)? }
        }
        Stream::Nbt => {
            let tag = rd!(Nbt::read(r));
            codec.decode(&tag).map_err(|e| format!("Failed to decode: {e} {}", nbt::to_string(&tag)))?
        }
        Stream::Uuid => {
            let (msb, lsb) = (rd!(r.i64()), rd!(r.i64()));
            Nbt::IntArray(vec![(msb >> 32) as i32, msb as i32, (lsb >> 32) as i32, lsb as i32])
        }
        Stream::Custom => match codec {
            Codec::Custom(c) => c.read(r)?,
            _ => return Err("no custom codec".into()),
        },
    })
}

fn read_fields(fields: &[Field], r: &mut Reader, out: &mut Vec<(String, Nbt)>) -> Result<(), String> {
    for field in fields {
        match &field.kind {
            Kind::Optional | Kind::LenientOptional => {
                if rd!(r.bool()) {
                    out.push((field.name.into(), read(&field.codec, &field.stream, r)?));
                }
            }
            Kind::Default(d) | Kind::Lenient(d) => {
                let v = read(&field.codec, &field.stream, r)?;
                if v != *d {
                    out.push((field.name.into(), v));
                }
            }
            Kind::AlwaysPresent(_) | Kind::Required | Kind::Plain => out.push((field.name.into(), read(&field.codec, &field.stream, r)?)),
        }
    }
    Ok(())
}
