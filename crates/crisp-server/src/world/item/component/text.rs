use std::sync::LazyLock;

use crisp_protocol::nbt::Nbt;
use crisp_protocol::{Reader, Writer};

use super::codec::{self, Codec, Custom, Dispatch, Selector, Stream};
use super::custom;
use super::types::{Pair, b, bool, def, identifier, lenient, lenient_opt, list, opt, record, req, string};
use crate::nbt;
use crate::util::hash_ops::{self, HashCode};

pub fn text() -> Pair {
    (Codec::Custom(Box::new(TextCodec)), Stream::Nbt)
}

pub struct TextCodec;


struct Contents {
    name: &'static str,
    fields: Codec,
}

fn arg() -> Pair {
    (Codec::Custom(Box::new(ArgCodec)), Stream::Nbt)
}

fn contents() -> Vec<Contents> {
    let r = |fields| record(fields).0;
    vec![
        Contents { name: "text", fields: r(vec![req("text", string())]) },
        Contents {
            name: "translatable",
            fields: r(vec![req("translate", string()), lenient_opt("fallback", string()), opt("with", list(arg()))]),
        },
        Contents { name: "keybind", fields: r(vec![req("keybind", string())]) },
        Contents { name: "score", fields: r(vec![req("score", record(vec![req("name", string()), req("objective", string())]))]) },
        Contents { name: "selector", fields: r(vec![req("selector", string()), opt("separator", text())]) },
        Contents {
            name: "nbt",
            fields: r(vec![
                req("nbt", string()),
                lenient("interpret", bool(), b(false)),
                lenient("plain", bool(), b(false)),
                lenient_opt("separator", text()),
            ]),
        },
        Contents { name: "object", fields: r(vec![opt("fallback", text())]) },
    ]
}

fn style_fields() -> Vec<super::codec::Field> {
    let click = Dispatch {
        key: "action",
        selector: Selector::Enum,
        cases: vec![
            ("open_url", vec![req("url", string())]),
            ("open_file", vec![req("path", string())]),
            ("run_command", vec![req("command", string())]),
            ("suggest_command", vec![req("command", string())]),
            ("show_dialog", vec![req("dialog", (Codec::AnyTag, Stream::Nbt))]),
            ("change_page", vec![req("page", (super::types::positive_int(), Stream::VarInt))]),
            ("copy_to_clipboard", vec![req("value", string())]),
            ("custom", vec![req("id", identifier()), opt("payload", (Codec::AnyTag, Stream::Nbt))]),
        ],
    };
    let mut show_item = custom::template_fields();
    show_item.truncate(3);
    let hover = Dispatch {
        key: "action",
        selector: Selector::Enum,
        cases: vec![
            ("show_text", vec![req("value", text())]),
            ("show_item", show_item),
            (
                "show_entity",
                vec![
                    req("id", (Codec::Element("minecraft:entity_type"), Stream::Registry("minecraft:entity_type"))),
                    req("uuid", custom::lenient_uuid()),
                    opt("name", text()),
                ],
            ),
        ],
    };
    vec![
        def("extra", (Codec::List(Box::new(text().0), 1, i32::MAX as usize), Stream::Nbt), Nbt::List(Vec::new())),
        opt("color", (Codec::Custom(Box::new(TextColorCodec)), Stream::Nbt)),
        opt("shadow_color", (super::types::argb_color(), Stream::Nbt)),
        opt("bold", bool()),
        opt("italic", bool()),
        opt("underlined", bool()),
        opt("strikethrough", bool()),
        opt("obfuscated", bool()),
        opt("click_event", (Codec::Dispatch(Box::new(click)), Stream::Nbt)),
        opt("hover_event", (Codec::Dispatch(Box::new(hover)), Stream::Nbt)),
        opt("insertion", string()),
        opt("font", identifier()),
    ]
}

static CONTENTS: LazyLock<Vec<Contents>> = LazyLock::new(contents);
static STYLE: LazyLock<Codec> = LazyLock::new(|| Codec::Record(style_fields()));

static UNION: LazyLock<Codec> = LazyLock::new(|| {
    let mut fields = vec![
        req("text", string()),
        req("translate", string()),
        opt("fallback", text()),
        opt("with", list(arg())),
        req("keybind", string()),
        req("score", record(vec![req("name", string()), req("objective", string())])),
        req("selector", string()),
        opt("separator", text()),
        req("nbt", string()),
        opt("interpret", bool()),
        opt("plain", bool()),
        opt("entity", string()),
        opt("block", string()),
        opt("storage", identifier()),
        opt("atlas", identifier()),
        opt("sprite", identifier()),
        opt("player", custom::profile()),
        opt("hat", bool()),
    ];
    fields.extend(style_fields());
    Codec::Record(fields)
});

fn either_error(tag: &Nbt, list_error: String, map_error: String) -> String {
    let _ = tag;
    format!("Failed to parse either. First: Failed to parse either. First: Not a string; Second: {list_error}; Second: {map_error}")
}

pub fn decode(tag: &Nbt) -> Result<Nbt, String> {
    let snbt = || nbt::to_string(tag);
    match tag {
        Nbt::String(s) => Ok(Nbt::String(s.clone())),
        Nbt::Compound(_) => decode_full(tag).map_err(|e| either_error(tag, format!("Not a list: {}", snbt()), e)),
        Nbt::List(_) | Nbt::ByteArray(_) | Nbt::IntArray(_) | Nbt::LongArray(_) => {
            let items = codec::list(tag).unwrap_or_default();
            let decoded: Result<Vec<Nbt>, String> = items.iter().map(decode).collect();
            match decoded {
                Ok(items) if items.is_empty() => Err(either_error(tag, "List must have contents".into(), format!("Not a map: {}", snbt()))),
                Ok(items) => Ok(from_list(items)),
                Err(e) => Err(either_error(tag, e, format!("Not a map: {}", snbt()))),
            }
        }
        _ => Err(either_error(tag, format!("Not a list: {}", snbt()), format!("Not a map: {}", snbt()))),
    }
}

fn from_list(mut items: Vec<Nbt>) -> Nbt {
    let rest = items.split_off(1);
    let first = items.pop().unwrap();
    if rest.is_empty() {
        return first;
    }
    let mut fields = match first {
        Nbt::String(s) => vec![("text".to_string(), Nbt::String(s))],
        Nbt::Compound(fields) => fields,
        other => vec![("text".to_string(), other)],
    };
    match fields.iter_mut().find(|(k, _)| k == "extra") {
        Some((_, Nbt::List(extra))) => extra.extend(rest),
        _ => fields.push(("extra".into(), Nbt::List(rest))),
    }
    Nbt::Compound(fields)
}

fn decode_full(tag: &Nbt) -> Result<Nbt, String> {
    let contents = match tag.get("type") {
        Some(ty) => {
            let name = codec::string(ty)?;
            let Some(c) = CONTENTS.iter().find(|c| c.name == name) else { return Err(format!("Unknown element name:{name}")) };
            c
        }
        None => CONTENTS.iter().find(|c| decode_contents(c, tag).is_ok()).ok_or_else(|| "No matching codec found".to_string())?,
    };
    let mut fields = match decode_contents(contents, tag)? {
        Nbt::Compound(fields) => fields,
        _ => Vec::new(),
    };
    if let Nbt::Compound(style) = STYLE.decode(tag)? {
        fields.extend(style);
    }
    fields.retain(|(k, v)| !(k == "with" && v.as_list().is_empty()));
    Ok(collapse(fields))
}

fn decode_contents(contents: &Contents, tag: &Nbt) -> Result<Nbt, String> {
    let mut value = contents.fields.decode(tag)?;
    let Nbt::Compound(fields) = &mut value else { return Ok(value) };
    match contents.name {
        "nbt" => {
            let source = ["entity", "block", "storage"]
                .into_iter()
                .find_map(|key| tag.get(key).and_then(|v| v.as_str()).map(|v| (key, v)))
                .ok_or_else(|| "No matching codec found".to_string())?;
            let value = if source.0 == "storage" { Nbt::String(codec::identifier(&Nbt::String(source.1.into()))?) } else { Nbt::String(source.1.into()) };
            if fields.iter().any(|(k, v)| k == "interpret" && *v == Nbt::Byte(1)) && fields.iter().any(|(k, v)| k == "plain" && *v == Nbt::Byte(1)) {
                return Err("'interpret' and 'plain' flags can't be both on".into());
            }
            fields.push((source.0.into(), value));
        }
        "object" => {
            let object: Vec<(String, Nbt)> = if tag.get("sprite").is_some() {
                let atlas = match tag.get("atlas") {
                    Some(a) => Some(codec::identifier(a)?),
                    None => None,
                };
                let mut out = vec![("sprite".to_string(), Nbt::String(codec::identifier(tag.get("sprite").unwrap())?))];
                if let Some(atlas) = atlas.filter(|a| a != "minecraft:blocks") {
                    out.push(("atlas".into(), Nbt::String(atlas)));
                }
                out
            } else if let Some(player) = tag.get("player") {
                let mut out = vec![("player".to_string(), custom::profile().0.decode(player)?)];
                if let Some(hat) = tag.get("hat") {
                    let hat = Codec::Bool.decode(hat)?;
                    if hat != b(true) {
                        out.push(("hat".into(), hat));
                    }
                }
                out
            } else {
                return Err("No matching codec found".into());
            };
            fields.extend(object);
        }
        _ => {}
    }
    Ok(value)
}

fn collapse(fields: Vec<(String, Nbt)>) -> Nbt {
    if let [(key, Nbt::String(text))] = fields.as_slice()
        && key == "text"
    {
        return Nbt::String(text.clone());
    }
    Nbt::Compound(fields)
}

pub fn hash(value: &Nbt) -> HashCode {
    match value {
        Nbt::String(s) => hash_ops::create_string(s),
        other => UNION.hash(other),
    }
}

impl Custom for TextCodec {
    fn decode(&self, tag: &Nbt) -> Result<Nbt, String> {
        decode(tag)
    }

    fn hash(&self, value: &Nbt) -> HashCode {
        hash(value)
    }

    fn write(&self, value: &Nbt, w: &mut Writer) -> Result<(), String> {
        value.write(w);
        Ok(())
    }

    fn read(&self, r: &mut Reader) -> Result<Nbt, String> {
        let tag = Nbt::read(r).map_err(|e| e.to_string())?;
        decode(&tag)
    }
}

struct ArgCodec;

impl Custom for ArgCodec {
    fn decode(&self, tag: &Nbt) -> Result<Nbt, String> {
        match tag {
            Nbt::Byte(_) | Nbt::Short(_) | Nbt::Int(_) | Nbt::Long(_) | Nbt::Float(_) | Nbt::Double(_) | Nbt::String(_) => Ok(tag.clone()),
            other => decode(other).map_err(|e| format!("Failed to parse either. First: This value needs to be parsed as component; Second: {e}")),
        }
    }

    fn hash(&self, value: &Nbt) -> HashCode {
        match value {
            Nbt::Compound(_) | Nbt::List(_) => hash(value),
            other => hash_ops::hash_nbt(other),
        }
    }

    fn write(&self, value: &Nbt, w: &mut Writer) -> Result<(), String> {
        value.write(w);
        Ok(())
    }

    fn read(&self, r: &mut Reader) -> Result<Nbt, String> {
        Nbt::read(r).map_err(|e| e.to_string())
    }
}

pub const NAMED_COLORS: [(&str, i32); 16] = [
    ("black", 0),
    ("dark_blue", 170),
    ("dark_green", 43520),
    ("dark_aqua", 43690),
    ("dark_red", 11141120),
    ("dark_purple", 11141290),
    ("gold", 16755200),
    ("gray", 11184810),
    ("dark_gray", 5592405),
    ("blue", 5592575),
    ("green", 5635925),
    ("aqua", 5636095),
    ("red", 16733525),
    ("light_purple", 16733695),
    ("yellow", 16777045),
    ("white", 16777215),
];

struct TextColorCodec;

impl Custom for TextColorCodec {
    fn decode(&self, tag: &Nbt) -> Result<Nbt, String> {
        let color = codec::string(tag)?;
        if let Some(hex) = color.strip_prefix('#') {
            let parsed = match hex.strip_prefix('+') {
                Some(rest) if !rest.starts_with(['+', '-']) => i64::from_str_radix(rest, 16),
                _ => i64::from_str_radix(hex, 16),
            };
            match parsed {
                Ok(v) if (i32::MIN as i64..=i32::MAX as i64).contains(&v) => {
                    if (0..=0xFFFFFF).contains(&v) {
                        Ok(Nbt::String(format!("#{v:06X}")))
                    } else {
                        Err(format!("Color value out of range: {color}"))
                    }
                }
                _ => Err(format!("Invalid color value: {color}")),
            }
        } else if NAMED_COLORS.iter().any(|(n, _)| *n == color) {
            Ok(Nbt::String(color.into()))
        } else {
            Err(format!("Invalid color name: {color}"))
        }
    }

    fn hash(&self, value: &Nbt) -> HashCode {
        hash_ops::hash_nbt(value)
    }

    fn write(&self, value: &Nbt, w: &mut Writer) -> Result<(), String> {
        value.write(w);
        Ok(())
    }

    fn read(&self, r: &mut Reader) -> Result<Nbt, String> {
        Nbt::read(r).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(snbt: &str) -> Nbt {
        crate::nbt::parse_fully(snbt).unwrap()
    }

    #[test]
    fn plain_text_collapses() {
        assert_eq!(decode(&parse("{text:\"a\"}")), Ok(Nbt::String("a".into())));
        assert_eq!(decode(&parse("{type:\"text\",text:\"a\"}")), Ok(Nbt::String("a".into())));
        assert_eq!(decode(&parse("[\"a\"]")), Ok(Nbt::String("a".into())));
        assert_eq!(decode(&parse("[\"a\",\"b\"]")), Ok(parse("{text:\"a\",extra:[\"b\"]}")));
        assert_eq!(decode(&parse("{text:\"a\",bold:1}")), Ok(parse("{text:\"a\",bold:1b}")));
        assert_eq!(decode(&parse("{translate:\"x\",with:[]}")), Ok(parse("{translate:\"x\"}")));
        assert_eq!(decode(&parse("{text:\"a\",color:\"#ff0000\"}")), Ok(parse("{text:\"a\",color:\"#FF0000\"}")));
        assert!(decode(&parse("{text:\"a\",extra:[]}")).is_err());
        assert!(decode(&parse("{}")).is_err());
    }
}
