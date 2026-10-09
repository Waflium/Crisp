use crisp_data::Item;
use crisp_protocol::nbt::{Nbt, java_hash_map_order};
use crisp_protocol::{Reader, Writer};

use super::codec::{self, Codec, Custom, Field, Kind, RangeError, Stream, as_i32, as_i64};
use super::types::{
    DYE_COLORS, Pair, always, b, bool, def, dye_color, empty_map, holder_set, i, lenient_opt, opt, record, req, string,
};
use super::{DataComponentPatch, DataComponentType, text};
use crate::commands::brigadier::StringReader;
use crate::nbt;
use crate::util::hash_ops::{self, HashCode};

macro_rules! rd {
    ($e:expr) => {
        $e.map_err(|e| e.to_string())?
    };
}

pub fn parse_snbt(s: &str) -> Result<Nbt, String> {
    let units: Vec<u16> = s.encode_utf16().collect();
    let mut reader = StringReader::new(&units);
    let tag = nbt::parse_as_argument(&mut reader).map_err(|e| e.get_message())?;
    reader.skip_whitespace();
    if reader.can_read() {
        return Err(format!("Expected whitespace to end one argument, but found trailing data at position {}", reader.cursor));
    }
    Ok(tag)
}

fn parse_compound(value: &Nbt) -> Result<Nbt, String> {
    match parse_snbt(codec::string(value)?)? {
        compound @ Nbt::Compound(_) => Ok(compound),
        other => Err(format!("Expected compound tag, got {}", nbt::to_string(&other))),
    }
}

pub fn custom_data_codec() -> Codec {
    Codec::Alt(Box::new(Codec::Compound), Box::new(Codec::string()), parse_compound)
}

pub fn show_icon_default(value: Nbt) -> Result<Nbt, String> {
    let Nbt::Compound(mut fields) = value else { return Ok(value) };
    if !fields.iter().any(|(k, _)| k == "show_icon") {
        let particles = fields.iter().find(|(k, _)| k == "show_particles").map_or(b(true), |(_, v)| v.clone());
        fields.push(("show_icon".into(), particles));
    }
    Ok(Nbt::Compound(fields))
}

pub fn non_empty_compact(value: Nbt) -> Result<Nbt, String> {
    match &value {
        Nbt::List(items) if items.is_empty() => Err("List must have contents".into()),
        _ => Ok(value),
    }
}

struct ItemCodec;

impl Custom for ItemCodec {
    fn decode(&self, tag: &Nbt) -> Result<Nbt, String> {
        let id = Codec::Element("minecraft:item").decode(tag)?;
        if id == Nbt::String("minecraft:air".into()) {
            return Err("Item must not be minecraft:air".into());
        }
        Ok(id)
    }

    fn hash(&self, value: &Nbt) -> HashCode {
        hash_ops::hash_nbt(value)
    }

    fn write(&self, value: &Nbt, w: &mut Writer) -> Result<(), String> {
        w.varint(codec::registry_id("minecraft:item", value.as_str().unwrap_or(""))?);
        Ok(())
    }

    fn read(&self, r: &mut Reader) -> Result<Nbt, String> {
        let id = rd!(r.varint());
        let item = Item::count().gt(&(id as usize)).then(|| Item(id as u16)).ok_or_else(|| format!("No item with id {id}"))?;
        Ok(Nbt::String(item.name().into()))
    }
}

pub struct PatchCodec;

pub fn patch_of(value: &Nbt) -> DataComponentPatch {
    let mut patch = DataComponentPatch::default();
    if let Nbt::Compound(fields) = value {
        for (key, v) in fields {
            let (removed, name) = match key.strip_prefix('!') {
                Some(name) => (true, name),
                None => (false, key.as_str()),
            };
            if let Some(ty) = DataComponentType::by_name(name) {
                patch.put(ty, (!removed).then(|| v.clone()));
            }
        }
    }
    patch
}

pub fn hash_patch(value: &Nbt) -> HashCode {
    let Nbt::Compound(fields) = value else { return hash_ops::hash_nbt(value) };
    let entries = fields
        .iter()
        .map(|(key, v)| {
            let h = match DataComponentType::by_name(key) {
                Some(ty) => ty.hash(v),
                None => hash_ops::create_map(Vec::new()),
            };
            (hash_ops::create_string(key), h)
        })
        .collect();
    hash_ops::create_map(entries)
}

impl Custom for PatchCodec {
    fn decode(&self, tag: &Nbt) -> Result<Nbt, String> {
        DataComponentPatch::from_nbt(tag).map(|p| p.to_nbt())
    }

    fn hash(&self, value: &Nbt) -> HashCode {
        hash_patch(value)
    }

    fn write(&self, value: &Nbt, w: &mut Writer) -> Result<(), String> {
        patch_of(value).write(w, false)
    }

    fn read(&self, r: &mut Reader) -> Result<Nbt, String> {
        Ok(DataComponentPatch::read(r, false)?.to_nbt())
    }
}

pub fn template_fields() -> Vec<Field> {
    vec![
        req("id", (Codec::Custom(Box::new(ItemCodec)), Stream::Custom)),
        def("count", (Codec::Int(1, 99, RangeError::Within), Stream::VarInt), i(1)),
        def("components", (Codec::Custom(Box::new(PatchCodec)), Stream::Custom), empty_map()),
    ]
}

pub fn template() -> Pair {
    (Codec::Custom(Box::new(TemplateCodec(Codec::Record(template_fields())))), Stream::Custom)
}

struct TemplateCodec(Codec);

impl Custom for TemplateCodec {
    fn decode(&self, tag: &Nbt) -> Result<Nbt, String> {
        self.0.decode(tag).or_else(|first| {
            ItemCodec.decode(tag).map(|id| Nbt::Compound(vec![("id".into(), id)])).map_err(|second| format!("Failed to parse either. First: {first}; Second: {second}"))
        })
    }

    fn hash(&self, value: &Nbt) -> HashCode {
        self.0.hash(value)
    }

    fn write(&self, value: &Nbt, w: &mut Writer) -> Result<(), String> {
        let Codec::Record(fields) = &self.0 else { unreachable!() };
        for field in fields {
            let v = match (&field.kind, value.get(field.name)) {
                (Kind::Default(d), None) => d,
                (_, Some(v)) => v,
                (_, None) => return Err(format!("missing field {}", field.name)),
            };
            codec::write(&field.codec, &field.stream, v, w)?;
        }
        Ok(())
    }

    fn read(&self, r: &mut Reader) -> Result<Nbt, String> {
        let id = ItemCodec.read(r)?;
        let count = rd!(r.varint());
        let patch = DataComponentPatch::read(r, false)?;
        let mut fields = vec![("id".to_string(), id)];
        if count != 1 {
            fields.push(("count".into(), Nbt::Int(count)));
        }
        if !patch.is_empty() {
            fields.push(("components".into(), patch.to_nbt()));
        }
        Ok(Nbt::Compound(fields))
    }
}

pub fn block_pos() -> Pair {
    (Codec::Custom(Box::new(BlockPosCodec)), Stream::Custom)
}

struct BlockPosCodec;

impl Custom for BlockPosCodec {
    fn decode(&self, tag: &Nbt) -> Result<Nbt, String> {
        let ints: Vec<i32> = match tag {
            Nbt::IntArray(v) => v.clone(),
            other => codec::list(other)?.iter().map(as_i32).collect(),
        };
        if ints.len() != 3 {
            return Err(format!("Input is not a list of 3 ints"));
        }
        Ok(Nbt::IntArray(ints))
    }

    fn hash(&self, value: &Nbt) -> HashCode {
        match value {
            Nbt::IntArray(v) => hash_ops::create_int_list(v),
            other => hash_ops::hash_nbt(other),
        }
    }

    fn write(&self, value: &Nbt, w: &mut Writer) -> Result<(), String> {
        let Nbt::IntArray(v) = value else { return Err("not a block pos".into()) };
        let packed = ((v[0] as i64 & 0x3FF_FFFF) << 38) | ((v[2] as i64 & 0x3FF_FFFF) << 12) | (v[1] as i64 & 0xFFF);
        w.i64(packed);
        Ok(())
    }

    fn read(&self, r: &mut Reader) -> Result<Nbt, String> {
        let packed = rd!(r.i64());
        let x = (packed >> 38) as i32;
        let y = ((packed << 52) >> 52) as i32;
        let z = ((packed << 26) >> 38) as i32;
        Ok(Nbt::IntArray(vec![x, y, z]))
    }
}

pub fn lenient_uuid() -> Pair {
    (Codec::Alt(Box::new(Codec::Uuid), Box::new(Codec::string()), uuid_from_string), Stream::Uuid)
}

fn uuid_from_string(value: &Nbt) -> Result<Nbt, String> {
    let s = codec::string(value)?;
    let parts: Vec<&str> = s.split('-').collect();
    let parsed = (parts.len() == 5)
        .then(|| parts.iter().map(|p| u64::from_str_radix(p, 16).ok()).collect::<Option<Vec<u64>>>())
        .flatten()
        .ok_or_else(|| format!("Invalid UUID {s}: Invalid UUID string: {s}"))?;
    let msb = (parsed[0] << 32) | ((parsed[1] & 0xFFFF) << 16) | (parsed[2] & 0xFFFF);
    let lsb = ((parsed[3] & 0xFFFF) << 48) | (parsed[4] & 0xFFFF_FFFF_FFFF);
    Ok(Nbt::IntArray(vec![(msb >> 32) as i32, msb as i32, (lsb >> 32) as i32, lsb as i32]))
}

pub fn typed_entity_data(registry: &'static str) -> Pair {
    (Codec::Custom(Box::new(TypedEntityData(registry))), Stream::Custom)
}

struct TypedEntityData(&'static str);

impl Custom for TypedEntityData {
    fn decode(&self, tag: &Nbt) -> Result<Nbt, String> {
        let compound = custom_data_codec().decode(tag)?;
        let Nbt::Compound(mut fields) = compound else { unreachable!() };
        let Some(index) = fields.iter().position(|(k, _)| k == "id") else {
            return Err(format!("Expected 'id' field in {}", nbt::to_string(tag)));
        };
        let (_, ty) = fields.remove(index);
        let id = Codec::Element(self.0).decode(&ty)?;
        fields.push(("id".into(), id));
        Ok(Nbt::Compound(fields))
    }

    fn hash(&self, value: &Nbt) -> HashCode {
        hash_ops::hash_nbt(value)
    }

    fn write(&self, value: &Nbt, w: &mut Writer) -> Result<(), String> {
        let Nbt::Compound(fields) = value else { return Err("not a compound".into()) };
        let id = value.get("id").and_then(Nbt::as_str).unwrap_or("");
        w.varint(codec::registry_id(self.0, id)?);
        Nbt::Compound(fields.iter().filter(|(k, _)| k != "id").cloned().collect()).write(w);
        Ok(())
    }

    fn read(&self, r: &mut Reader) -> Result<Nbt, String> {
        let id = rd!(r.varint());
        let name = codec::registry(self.0).get(id as usize).ok_or_else(|| format!("No value with id {id}"))?;
        let Nbt::Compound(mut fields) = rd!(Nbt::read(r)) else { return Err("Not a compound tag".into()) };
        fields.retain(|(k, _)| k != "id");
        fields.push(("id".into(), Nbt::String((*name).into())));
        Ok(Nbt::Compound(fields))
    }
}

pub fn container() -> Pair {
    (Codec::Custom(Box::new(Container(Codec::Record(vec![req("slot", (Codec::Int(0, 255, RangeError::Outside), Stream::VarInt)), req("item", template())])))), Stream::Custom)
}

struct Container(Codec);

impl Container {
    fn normalize(slots: Vec<Nbt>) -> Nbt {
        let mut by_index: Vec<Option<Nbt>> = Vec::new();
        for slot in slots {
            let index = slot.get("slot").map_or(0, as_i64) as usize;
            if by_index.len() <= index {
                by_index.resize(index + 1, None);
            }
            by_index[index] = Some(slot);
        }
        Nbt::List(by_index.into_iter().flatten().collect())
    }
}

impl Custom for Container {
    fn decode(&self, tag: &Nbt) -> Result<Nbt, String> {
        let slots = Codec::List(Box::new(Codec::AnyTag), 0, 256).decode(tag)?;
        let mut out = Vec::new();
        let mut errors = Vec::new();
        for slot in slots.as_list() {
            match self.0.decode(slot) {
                Ok(v) => out.push(v),
                Err(e) => errors.push(e),
            }
        }
        if !errors.is_empty() {
            return Err(errors.join("; "));
        }
        Ok(Container::normalize(out))
    }

    fn hash(&self, value: &Nbt) -> HashCode {
        hash_ops::create_list(value.as_list().iter().map(|v| self.0.hash(v)))
    }

    fn write(&self, value: &Nbt, w: &mut Writer) -> Result<(), String> {
        let slots = value.as_list();
        let size = slots.iter().map(|s| s.get("slot").map_or(0, as_i64) as usize + 1).max().unwrap_or(0);
        if size > 256 {
            return Err(format!("{size} elements exceeded max size of: 256"));
        }
        w.varint(size as i32);
        let (template_codec, template_stream) = template();
        for index in 0..size {
            match slots.iter().find(|s| s.get("slot").map_or(0, as_i64) as usize == index).and_then(|s| s.get("item")) {
                Some(item) => {
                    w.bool(true);
                    codec::write(&template_codec, &template_stream, item, w)?;
                }
                None => w.bool(false),
            }
        }
        Ok(())
    }

    fn read(&self, r: &mut Reader) -> Result<Nbt, String> {
        let size = rd!(r.varint());
        if size > 256 {
            return Err(format!("{size} elements exceeded max size of: 256"));
        }
        let (template_codec, template_stream) = template();
        let mut slots = Vec::new();
        for index in 0..size.max(0) {
            if rd!(r.bool()) {
                let item = codec::read(&template_codec, &template_stream, r)?;
                slots.push(Nbt::Compound(vec![("slot".into(), Nbt::Int(index)), ("item".into(), item)]));
            }
        }
        Ok(Nbt::List(slots))
    }
}

pub fn sign_text() -> Pair {
    let lines = || (Codec::List(Box::new(text::text().0), 4, 4), Stream::FixedList(Box::new(Stream::Nbt), 4));
    let fields = vec![
        req("messages", lines()),
        lenient_opt("filtered_messages", lines()),
        always("color", dye_color(), Nbt::String(DYE_COLORS[15].into())),
        always("has_glowing_text", bool(), b(false)),
    ];
    (Codec::Normalize(Box::new(Codec::Record(fields)), drop_unfiltered), Stream::Record)
}

fn drop_unfiltered(value: Nbt) -> Result<Nbt, String> {
    let Nbt::Compound(mut fields) = value else { return Ok(value) };
    if fields.iter().any(|(k, v)| k == "filtered_messages" && Some(v) == fields.iter().find(|(k, _)| k == "messages").map(|(_, v)| v)) {
        fields.retain(|(k, _)| k != "filtered_messages");
    }
    Ok(Nbt::Compound(fields))
}

pub fn profile() -> Pair {
    (Codec::Custom(Box::new(Profile)), Stream::Custom)
}

struct Profile;

fn player_name(tag: &Nbt) -> Result<Nbt, String> {
    let name = Codec::String(0, 16).decode(tag)?;
    let s = name.as_str().unwrap_or("");
    if s.chars().any(|c| (c as u32) <= 32 || (c as u32) >= 127) {
        return Err(format!("Player name contained disallowed characters: '{s}'"));
    }
    Ok(name)
}

fn property_map(tag: &Nbt) -> Result<Nbt, String> {
    let property = Codec::Record(vec![
        req("name", (Codec::String(0, 64), Stream::Utf8(64))),
        req("value", (Codec::String(0, 32767), Stream::Utf8(32767))),
        opt("signature", (Codec::String(0, 1024), Stream::Utf8(1024))),
    ]);
    match tag {
        Nbt::Compound(fields) => {
            if fields.len() > 16 {
                return Err(format!("Cannot have more than 16 properties, but was {}", fields.len()));
            }
            let mut out = Vec::new();
            for (name, values) in fields {
                for value in codec::list(values)? {
                    let value = Codec::string().decode(&value)?;
                    out.push(Nbt::Compound(vec![("name".into(), Nbt::String(name.clone())), ("value".into(), value)]));
                }
            }
            Ok(Nbt::List(out))
        }
        other => Codec::List(Box::new(property), 0, 16).decode(other),
    }
}

const SKIN_FIELDS: [&str; 3] = ["texture", "cape", "elytra"];

impl Profile {
    fn decode_map(tag: &Nbt) -> Result<Nbt, String> {
        codec::map(tag)?;
        let field = |name: &str| tag.get(name);
        let properties = match field("properties") {
            Some(p) => Some(property_map(p)?).filter(|p| !p.as_list().is_empty()),
            None => None,
        };
        let id = field("id").map(|v| Codec::Uuid.decode(v)).transpose();
        let name = field("name").map(player_name).transpose();
        let mut fields = Vec::new();
        match (id, name) {
            (Ok(id), Ok(name)) => {
                if let Some(name) = name {
                    fields.push(("name".to_string(), name));
                }
                if let Some(id) = id {
                    fields.push(("id".to_string(), id));
                }
            }
            (Err(e), _) | (_, Err(e)) => return Err(e),
        }
        if let Some(properties) = properties {
            fields.push(("properties".into(), properties));
        }
        for key in SKIN_FIELDS {
            if let Some(v) = field(key) {
                fields.push((key.into(), Nbt::String(codec::identifier(v)?)));
            }
        }
        if let Some(model) = field("model") {
            fields.push(("model".into(), Codec::Enum(&["slim", "wide"]).decode(model)?));
        }
        Ok(Nbt::Compound(fields))
    }

    fn write_properties(value: &Nbt, w: &mut Writer) {
        let properties = value.get("properties").map(Nbt::as_list).unwrap_or(&[]);
        w.varint(properties.len() as i32);
        for p in properties {
            w.string(p.get("name").and_then(Nbt::as_str).unwrap_or(""));
            w.string(p.get("value").and_then(Nbt::as_str).unwrap_or(""));
            match p.get("signature").and_then(Nbt::as_str) {
                Some(signature) => {
                    w.bool(true);
                    w.string(signature);
                }
                None => w.bool(false),
            }
        }
    }

    fn read_properties(r: &mut Reader) -> Result<Option<Nbt>, String> {
        let n = rd!(r.varint());
        if n > 16 {
            return Err(format!("{n} elements exceeded max size of: 16"));
        }
        let mut out = Vec::new();
        for _ in 0..n.max(0) {
            let mut p = vec![("name".to_string(), Nbt::String(rd!(r.string(64)).into())), ("value".into(), Nbt::String(rd!(r.string(32767)).into()))];
            if rd!(r.bool()) {
                p.push(("signature".into(), Nbt::String(rd!(r.string(1024)).into())));
            }
            out.push(Nbt::Compound(p));
        }
        Ok((!out.is_empty()).then_some(Nbt::List(out)))
    }
}

fn write_uuid(value: &Nbt, w: &mut Writer) {
    let v = match value {
        Nbt::IntArray(v) if v.len() == 4 => v.clone(),
        _ => vec![0; 4],
    };
    w.i64(((v[0] as u32 as i64) << 32) | v[1] as u32 as i64);
    w.i64(((v[2] as u32 as i64) << 32) | v[3] as u32 as i64);
}

fn read_uuid(r: &mut Reader) -> Result<Nbt, String> {
    let (msb, lsb) = (rd!(r.i64()), rd!(r.i64()));
    Ok(Nbt::IntArray(vec![(msb >> 32) as i32, msb as i32, (lsb >> 32) as i32, lsb as i32]))
}

impl Custom for Profile {
    fn decode(&self, tag: &Nbt) -> Result<Nbt, String> {
        match tag {
            Nbt::String(_) => player_name(tag)
                .map(|name| Nbt::Compound(vec![("name".into(), name)]))
                .map_err(|e| format!("Failed to parse either. First: Not a map: {}; Second: {e}", nbt::to_string(tag))),
            other => Profile::decode_map(other).map_err(|e| format!("Failed to parse either. First: {e}; Second: Not a string")),
        }
    }

    fn hash(&self, value: &Nbt) -> HashCode {
        let Nbt::Compound(fields) = value else { return hash_ops::hash_nbt(value) };
        let entries = fields
            .iter()
            .map(|(k, v)| {
                let h = match (k.as_str(), v) {
                    ("id", Nbt::IntArray(ints)) => hash_ops::create_int_list(ints),
                    _ => hash_ops::hash_nbt(v),
                };
                (hash_ops::create_string(k), h)
            })
            .collect();
        hash_ops::create_map(entries)
    }

    fn write(&self, value: &Nbt, w: &mut Writer) -> Result<(), String> {
        let id = value.get("id");
        let name = value.get("name").and_then(Nbt::as_str);
        match (id, name) {
            (Some(id), Some(name)) => {
                w.bool(true);
                write_uuid(id, w);
                w.string(name);
            }
            _ => {
                w.bool(false);
                match name {
                    Some(name) => {
                        w.bool(true);
                        w.string(name);
                    }
                    None => w.bool(false),
                }
                match id {
                    Some(id) => {
                        w.bool(true);
                        write_uuid(id, w);
                    }
                    None => w.bool(false),
                }
            }
        }
        Profile::write_properties(value, w);
        for key in SKIN_FIELDS {
            match value.get(key).and_then(Nbt::as_str) {
                Some(v) => {
                    w.bool(true);
                    w.string(v);
                }
                None => w.bool(false),
            }
        }
        match value.get("model").and_then(Nbt::as_str) {
            Some(model) => {
                w.bool(true);
                w.bool(model == "slim");
            }
            None => w.bool(false),
        }
        Ok(())
    }

    fn read(&self, r: &mut Reader) -> Result<Nbt, String> {
        let mut fields = Vec::new();
        if rd!(r.bool()) {
            let id = read_uuid(r)?;
            fields.push(("name".to_string(), Nbt::String(rd!(r.string(16)).into())));
            fields.push(("id".to_string(), id));
        } else {
            if rd!(r.bool()) {
                fields.push(("name".to_string(), Nbt::String(rd!(r.string(16)).into())));
            }
            if rd!(r.bool()) {
                fields.push(("id".to_string(), read_uuid(r)?));
            }
        }
        if let Some(properties) = Profile::read_properties(r)? {
            fields.push(("properties".into(), properties));
        }
        for key in SKIN_FIELDS {
            if rd!(r.bool()) {
                let s = rd!(r.string(32767));
                fields.push((key.into(), Nbt::String(crate::commands::arguments::parse_identifier(s).unwrap_or_else(|| s.to_string()))));
            }
        }
        if rd!(r.bool()) {
            fields.push(("model".into(), Nbt::String(if rd!(r.bool()) { "slim" } else { "wide" }.into())));
        }
        Ok(Nbt::Compound(fields))
    }
}

pub fn block_predicate() -> Pair {
    let state = (
        Codec::Map(
            Box::new(Codec::string()),
            Box::new(Codec::Either(Box::new(Codec::string()), Box::new(Codec::Record(vec![opt("min", string()), opt("max", string())])))),
        ),
        Stream::Map(
            Box::new(Stream::Utf8(32767)),
            Box::new(Stream::Either(Box::new(Stream::Utf8(32767)), Box::new(Stream::Record))),
            i32::MAX as usize,
        ),
    );
    record(vec![
        opt("blocks", holder_set("minecraft:block")),
        opt("state", state),
        opt("nbt", (Codec::Custom(Box::new(NbtPredicate)), Stream::Custom)),
        def("components", (Codec::Custom(Box::new(ExactPredicate)), Stream::Custom), empty_map()),
        def("predicates", (Codec::Custom(Box::new(PartialPredicates)), Stream::Custom), empty_map()),
    ])
}

fn int_bounds() -> Codec {
    let range = Codec::Record(vec![opt("min", (Codec::int(), Stream::VarInt)), opt("max", (Codec::int(), Stream::VarInt))]);
    Codec::Normalize(Box::new(Codec::Either(Box::new(range), Box::new(Codec::int()))), |value| {
        let (Some(min), Some(max)) = (value.get("min"), value.get("max")) else { return Ok(value) };
        let (min, max) = (as_i32(min), as_i32(max));
        if min > max {
            Err(format!("Swapped bounds in range: Optional[{min}] is higher than Optional[{max}]"))
        } else if min == max {
            Ok(Nbt::Int(min))
        } else {
            Ok(value)
        }
    })
}

pub fn item_predicate() -> Codec {
    Codec::Record(vec![
        opt("items", holder_set("minecraft:item")),
        def("count", (int_bounds(), Stream::Nbt), empty_map()),
        def("components", (Codec::Custom(Box::new(ExactPredicate)), Stream::Custom), empty_map()),
        def("predicates", (Codec::Custom(Box::new(PartialPredicates)), Stream::Custom), empty_map()),
    ])
}

struct NbtPredicate;

impl Custom for NbtPredicate {
    fn decode(&self, tag: &Nbt) -> Result<Nbt, String> {
        match tag {
            Nbt::String(_) => parse_compound(tag).map(|c| Nbt::String(nbt::to_string(&c))),
            Nbt::Compound(_) => Ok(Nbt::String(nbt::to_string(tag))),
            other => Err(format!("Failed to parse either. First: Not a string; Second: Not a map: {}", nbt::to_string(other))),
        }
    }

    fn hash(&self, value: &Nbt) -> HashCode {
        hash_ops::hash_nbt(value)
    }

    fn write(&self, value: &Nbt, w: &mut Writer) -> Result<(), String> {
        parse_compound(value)?.write(w);
        Ok(())
    }

    fn read(&self, r: &mut Reader) -> Result<Nbt, String> {
        Ok(Nbt::String(nbt::to_string(&rd!(Nbt::read(r)))))
    }
}

struct ExactPredicate;

impl Custom for ExactPredicate {
    fn decode(&self, tag: &Nbt) -> Result<Nbt, String> {
        let fields = codec::map(tag)?;
        let mut out = Vec::new();
        for (key, value) in fields {
            let id = codec::identifier(&Nbt::String(key.clone()))?;
            let ty = DataComponentType::by_name(&id).ok_or_else(|| format!("Unknown registry key in ResourceKey[minecraft:root / minecraft:data_component_type]: {id}"))?;
            if ty.is_transient() {
                return Err(format!("Encountered transient component {id}"));
            }
            out.push((id, ty.decode(value)?));
        }
        Ok(Nbt::Compound(out))
    }

    fn hash(&self, value: &Nbt) -> HashCode {
        hash_patch(value)
    }

    fn write(&self, value: &Nbt, w: &mut Writer) -> Result<(), String> {
        let Nbt::Compound(fields) = value else { return Err("not a map".into()) };
        w.varint(fields.len() as i32);
        for i in java_hash_map_order(&fields.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>()) {
            let (key, v) = &fields[i];
            let ty = DataComponentType::by_name(key).ok_or("unknown component")?;
            w.varint(ty.0 as i32);
            ty.write(v, w)?;
        }
        Ok(())
    }

    fn read(&self, r: &mut Reader) -> Result<Nbt, String> {
        let n = rd!(r.varint());
        let mut out = Vec::new();
        for _ in 0..n.max(0) {
            let ty = DataComponentType::by_id(rd!(r.varint())).ok_or("unknown component")?;
            out.push((ty.name().to_string(), ty.read(r)?));
        }
        Ok(Nbt::Compound(out))
    }
}

struct PartialPredicates;

const PREDICATE_TYPES: &str = "minecraft:data_component_predicate_type";

impl Custom for PartialPredicates {
    fn decode(&self, tag: &Nbt) -> Result<Nbt, String> {
        let fields = codec::map(tag)?;
        let mut out = Vec::new();
        for (key, value) in fields {
            let id = codec::identifier(&Nbt::String(key.clone()))?;
            if codec::registry(PREDICATE_TYPES).contains(&id.as_str()) {
                out.push((id, value.clone()));
            } else if DataComponentType::by_name(&id).is_some() {
                out.push((id, Codec::Unit.decode(value)?));
            } else {
                return Err(format!(
                    "Failed to parse either. First: Unknown registry key in ResourceKey[minecraft:root / {PREDICATE_TYPES}]: {id}; Second: Unknown registry key in ResourceKey[minecraft:root / minecraft:data_component_type]: {id}"
                ));
            }
        }
        Ok(Nbt::Compound(out))
    }

    fn hash(&self, value: &Nbt) -> HashCode {
        hash_ops::hash_nbt(value)
    }

    fn write(&self, value: &Nbt, w: &mut Writer) -> Result<(), String> {
        let Nbt::Compound(fields) = value else { return Err("not a map".into()) };
        if fields.len() > 64 {
            return Err(format!("{} elements exceeded max size of: 64", fields.len()));
        }
        w.varint(fields.len() as i32);
        for (key, v) in fields {
            match codec::registry_id(PREDICATE_TYPES, key) {
                Ok(id) => {
                    w.bool(true);
                    w.varint(id);
                }
                Err(_) => {
                    w.bool(false);
                    w.varint(DataComponentType::by_name(key).ok_or("unknown component")?.0 as i32);
                }
            }
            v.write(w);
        }
        Ok(())
    }

    fn read(&self, r: &mut Reader) -> Result<Nbt, String> {
        let n = rd!(r.varint());
        if n > 64 {
            return Err(format!("{n} elements exceeded max size of: 64"));
        }
        let mut out = Vec::new();
        for _ in 0..n.max(0) {
            let id = if rd!(r.bool()) {
                let id = rd!(r.varint());
                codec::registry(PREDICATE_TYPES).get(id as usize).ok_or("unknown predicate type")?.to_string()
            } else {
                DataComponentType::by_id(rd!(r.varint())).ok_or("unknown component")?.name().to_string()
            };
            out.push((id, rd!(Nbt::read(r))));
        }
        Ok(Nbt::Compound(out))
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_pos_packs_like_vanilla() {
        let mut w = Writer::new();
        let pos = Nbt::IntArray(vec![-1, 64, 3]);
        BlockPosCodec.write(&pos, &mut w).unwrap();
        assert_eq!(BlockPosCodec.read(&mut Reader::new(w.as_slice())).unwrap(), pos);
    }
}
