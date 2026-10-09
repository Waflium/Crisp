use crisp_data::registry;
use crisp_protocol::nbt::Nbt;

use super::{Arg, ClickEvent, Component, Contents, HoverEvent, Style, TextColor};
use crate::commands::arguments::parse_identifier;
use crate::commands::arguments::selector::entity_selector::EntitySelector;
use crate::commands::arguments::selector::entity_selector_parser::EntitySelectorParser;
use crate::commands::brigadier::{StringReader, tr};

struct Error {
    message: String,
    partial: bool,
}

impl Error {
    fn new(message: impl Into<String>) -> Error {
        Error { message: message.into(), partial: false }
    }

    fn partial(self) -> Error {
        Error { partial: true, ..self }
    }
}

impl From<String> for Error {
    fn from(message: String) -> Error {
        Error::new(message)
    }
}

type Result<T> = std::result::Result<T, Error>;

const STYLE_KEYS: [&str; 11] = ["color", "shadow_color", "bold", "italic", "underlined", "strikethrough", "obfuscated", "click_event", "hover_event", "insertion", "font"];

fn snbt(tag: &Nbt) -> String {
    crate::nbt::to_string(tag)
}

fn get<'a>(fields: &'a [(String, Nbt)], key: &str) -> Option<&'a Nbt> {
    fields.iter().find(|(k, _)| k == key).map(|(_, v)| v)
}

fn no_key(key: &str, fields: &[(String, Nbt)]) -> Error {
    Error::new(format!("No key {key} in MapLike[{}]", snbt(&Nbt::Compound(fields.to_vec()))))
}

fn required<'a>(fields: &'a [(String, Nbt)], key: &str) -> Result<&'a Nbt> {
    get(fields, key).ok_or_else(|| no_key(key, fields))
}

fn string(tag: &Nbt) -> Result<String> {
    match tag {
        Nbt::String(s) => Ok(s.clone()),
        _ => Err(Error::new("Not a string")),
    }
}

fn boolean(tag: &Nbt) -> Result<bool> {
    tag.as_f64().map(|v| v != 0.0).ok_or_else(|| Error::new("Not a number"))
}

fn int(tag: &Nbt) -> Result<i32> {
    match tag {
        Nbt::Byte(v) => Ok(*v as i32),
        Nbt::Short(v) => Ok(*v as i32),
        Nbt::Int(v) => Ok(*v),
        Nbt::Long(v) => Ok(*v as i32),
        Nbt::Float(v) => Ok(*v as i32),
        Nbt::Double(v) => Ok(*v as i32),
        _ => Err(Error::new("Not a number")),
    }
}

pub fn decode(tag: &Nbt) -> std::result::Result<Component, String> {
    decode_component(tag).map_err(|e| e.message)
}

pub fn decode_style_fields(fields: &[(String, Nbt)]) -> std::result::Result<Style, String> {
    style_fields(fields).map_err(|e| e.message)
}

fn decode_component(tag: &Nbt) -> Result<Component> {
    let special = match tag {
        Nbt::String(s) => return Ok(Component::literal(s.clone())),
        Nbt::List(items) => match decode_list(items) {
            Ok(list) => return Ok(create_from_list(list)),
            Err(e) if e.partial => return Err(e),
            Err(e) => format!("Failed to parse either. First: Not a string; Second: {}", e.message),
        },
        _ => format!("Failed to parse either. First: Not a string; Second: Not a list: {}", snbt(tag)),
    };
    let full = match tag {
        Nbt::Compound(fields) => decode_full(fields),
        _ => Err(Error::new(format!("Not a map: {}", snbt(tag)))),
    };
    full.map_err(|e| if e.partial { e } else { Error::new(format!("Failed to parse either. First: {special}; Second: {}", e.message)) })
}

fn decode_list(items: &[Nbt]) -> Result<Vec<Component>> {
    let mut list = Vec::new();
    let mut errors = Vec::new();
    for item in items {
        match decode_component(item) {
            Ok(component) => list.push(component),
            Err(e) => errors.push(e.message),
        }
    }
    if !errors.is_empty() {
        if list.is_empty() {
            errors.push("List must have contents".into());
        }
        return Err(Error::new(errors.join("; ")).partial());
    }
    if list.is_empty() {
        return Err(Error::new("List must have contents"));
    }
    Ok(list)
}

fn create_from_list(list: Vec<Component>) -> Component {
    let mut list = list.into_iter();
    let mut result = list.next().expect("a non-empty list");
    result.siblings.extend(list);
    result
}

fn decode_full(fields: &[(String, Nbt)]) -> Result<Component> {
    let contents = decode_contents(fields)?;
    let siblings = match get(fields, "extra") {
        Some(Nbt::List(items)) => decode_list(items).map_err(Error::partial)?,
        Some(other) => return Err(Error::new(format!("Not a list: {}", snbt(other))).partial()),
        None => Vec::new(),
    };
    let style = style_fields(fields)?;
    Ok(Component { contents, style, siblings })
}

const CONTENT_TYPES: [&str; 7] = ["text", "translatable", "keybind", "score", "selector", "nbt", "object"];

fn decode_contents(fields: &[(String, Nbt)]) -> Result<Contents> {
    if let Some(kind) = get(fields, "type") {
        let kind = string(kind)?;
        if !CONTENT_TYPES.contains(&kind.as_str()) {
            return Err(Error::new(format!("Unknown ID: {kind}")));
        }
        return decode_content_type(&kind, fields);
    }
    for kind in CONTENT_TYPES {
        if let Ok(contents) = decode_content_type(kind, fields) {
            return Ok(contents);
        }
    }
    Err(Error::new("No matching codec found"))
}

fn decode_content_type(kind: &str, fields: &[(String, Nbt)]) -> Result<Contents> {
    match kind {
        "text" => Ok(Contents::Literal(string(required(fields, "text")?)?)),
        "translatable" => {
            let key = string(required(fields, "translate")?)?;
            let fallback = get(fields, "fallback").and_then(|f| string(f).ok());
            let args = match get(fields, "with") {
                Some(Nbt::List(items)) => items.iter().map(decode_arg).collect::<Result<Vec<_>>>().map_err(Error::partial)?,
                Some(other) => return Err(Error::new(format!("Not a list: {}", snbt(other))).partial()),
                None => Vec::new(),
            };
            Ok(Contents::Translatable { key, fallback, args })
        }
        "keybind" => Ok(Contents::Keybind(string(required(fields, "keybind")?)?)),
        "score" => {
            let score = required(fields, "score")?;
            let Nbt::Compound(score) = score else { return Err(Error::new(format!("Not a map: {}", snbt(score)))) };
            let name = string(required(score, "name")?)?;
            let objective = string(required(score, "objective")?)?;
            let selector = compile_selector(&name).ok().map(Box::new);
            Ok(Contents::Score { name, selector, objective })
        }
        "selector" => {
            let pattern = string(required(fields, "selector")?)?;
            let selector = Box::new(compile_selector(&pattern).map_err(Error::new)?);
            let separator = match get(fields, "separator") {
                Some(separator) => Some(Box::new(decode_component(separator).map_err(Error::partial)?)),
                None => None,
            };
            Ok(Contents::Selector { pattern, selector, separator })
        }
        "nbt" => {
            string(required(fields, "nbt")?)?;
            Ok(Contents::Other(content_fields(fields)))
        }
        _ => {
            if get(fields, "object").is_none() && get(fields, "sprite").is_none() && get(fields, "player").is_none() {
                return Err(no_key("object", fields));
            }
            Ok(Contents::Other(content_fields(fields)))
        }
    }
}

fn content_fields(fields: &[(String, Nbt)]) -> Vec<(String, Nbt)> {
    fields.iter().filter(|(k, _)| k != "extra" && k != "type" && !STYLE_KEYS.contains(&k.as_str())).cloned().collect()
}

fn compile_selector(pattern: &str) -> std::result::Result<EntitySelector, String> {
    let units: Vec<u16> = pattern.encode_utf16().collect();
    let mut reader = StringReader::new(&units);
    let parsed = EntitySelectorParser::new(&mut reader, true).parse();
    let result = parsed.and_then(|selector| {
        if reader.can_read() {
            let parsed = reader.substring(0, reader.cursor);
            let leftovers = reader.substring(reader.cursor, reader.total_length());
            return Err(crate::commands::brigadier::CommandSyntaxException::new(tr("command.trailing_data", vec![format!("{parsed}[{leftovers}]").into()])));
        }
        Ok(selector)
    });
    result.map_err(|e| format!("Invalid selector component: {pattern}: {}", e.get_message()))
}

fn decode_arg(tag: &Nbt) -> Result<Arg> {
    Ok(match tag {
        Nbt::String(s) => Arg::String(s.clone()),
        Nbt::Byte(v) => Arg::Byte(*v),
        Nbt::Short(v) => Arg::Short(*v),
        Nbt::Int(v) => Arg::Int(*v),
        Nbt::Long(v) => Arg::Long(*v),
        Nbt::Float(v) => Arg::Float(*v),
        Nbt::Double(v) => Arg::Double(*v),
        other => {
            let component = decode_component(other).map_err(|e| {
                if e.partial { e } else { Error::new(format!("Failed to parse either. First: This value needs to be parsed as component; Second: {}", e.message)) }
            })?;
            match component.contents {
                Contents::Literal(text) if component.style.is_empty() && component.siblings.is_empty() => Arg::String(text),
                _ => Arg::Component(component),
            }
        }
    })
}

fn style_fields(fields: &[(String, Nbt)]) -> Result<Style> {
    style_fields_strict(fields).map_err(Error::partial)
}

fn style_fields_strict(fields: &[(String, Nbt)]) -> Result<Style> {
    let mut style = Style::default();
    if let Some(color) = get(fields, "color") {
        style.color = Some(TextColor::parse_color(&string(color)?)?);
    }
    if let Some(shadow) = get(fields, "shadow_color") {
        style.shadow_color = Some(argb_color(shadow)?);
    }
    for (key, slot) in [
        ("bold", &mut style.bold),
        ("italic", &mut style.italic),
        ("underlined", &mut style.underlined),
        ("strikethrough", &mut style.strikethrough),
        ("obfuscated", &mut style.obfuscated),
    ] {
        if let Some(value) = get(fields, key) {
            *slot = Some(boolean(value)?);
        }
    }
    if let Some(click) = get(fields, "click_event") {
        style.click_event = Some(click_event(click)?);
    }
    if let Some(hover) = get(fields, "hover_event") {
        style.hover_event = Some(hover_event(hover)?);
    }
    if let Some(insertion) = get(fields, "insertion") {
        style.insertion = Some(string(insertion)?);
    }
    if let Some(font) = get(fields, "font") {
        let font = string(font)?;
        style.font = Some(parse_identifier(&font).ok_or_else(|| Error::new(format!("Not a valid resource location: {font}")))?);
    }
    Ok(style)
}

fn argb_color(tag: &Nbt) -> Result<i32> {
    if let Ok(value) = int(tag) {
        return Ok(value);
    }
    let Nbt::List(items) = tag else { return Err(Error::new(format!("Not a list: {}", snbt(tag)))) };
    let channels = items.iter().map(|i| i.as_f64().map(|v| v as f32).ok_or_else(|| Error::new("Not a number"))).collect::<Result<Vec<f32>>>()?;
    if channels.len() != 4 {
        return Err(Error::new(format!("Input is not a list of 4 elements: {}", snbt(tag))));
    }
    let channel = |f: f32| crisp_core::mth::floor((f * 255.0) as f64) & 0xFF;
    Ok(channel(channels[3]) << 24 | channel(channels[0]) << 16 | channel(channels[1]) << 8 | channel(channels[2]))
}

fn click_event(tag: &Nbt) -> Result<ClickEvent> {
    let Nbt::Compound(fields) = tag else { return Err(Error::new(format!("Not a map: {}", snbt(tag)))) };
    let action = string(required(fields, "action")?)?;
    Ok(match action.as_str() {
        "open_url" => ClickEvent::OpenUrl(string(required(fields, "url")?)?),
        "run_command" => ClickEvent::RunCommand(string(required(fields, "command")?)?),
        "suggest_command" => ClickEvent::SuggestCommand(string(required(fields, "command")?)?),
        "copy_to_clipboard" => ClickEvent::CopyToClipboard(string(required(fields, "value")?)?),
        "change_page" => {
            let page = int(required(fields, "page")?)?;
            if page <= 0 {
                return Err(Error::new(format!("Value must be positive: {page}")));
            }
            ClickEvent::ChangePage(page)
        }
        "show_dialog" | "custom" => {
            let action = if action == "custom" { "custom" } else { "show_dialog" };
            ClickEvent::Other { action, fields: fields.iter().filter(|(k, _)| k != "action").cloned().collect() }
        }
        "open_file" => return Err(Error::new("Click event type not allowed: OPEN_FILE")),
        other => return Err(Error::new(format!("Unknown element name:{other}"))),
    })
}

fn hover_event(tag: &Nbt) -> Result<HoverEvent> {
    let Nbt::Compound(fields) = tag else { return Err(Error::new(format!("Not a map: {}", snbt(tag)))) };
    let action = string(required(fields, "action")?)?;
    Ok(match action.as_str() {
        "show_text" => HoverEvent::ShowText(Box::new(decode_component(required(fields, "value")?)?)),
        "show_item" => {
            let raw = string(required(fields, "id")?)?;
            let id = parse_identifier(&raw)
                .and_then(|id| registry::ITEM.iter().find(|&&i| i == id))
                .ok_or_else(|| Error::new(format!("Unknown registry key in ResourceKey[minecraft:root / minecraft:item]: {raw}")))?;
            let count = match get(fields, "count") {
                Some(count) => int(count)?,
                None => 1,
            };
            let components = match get(fields, "components") {
                Some(Nbt::Compound(c)) => c.clone(),
                Some(other) => return Err(Error::new(format!("Not a map: {}", snbt(other)))),
                None => Vec::new(),
            };
            HoverEvent::ShowItem { id, count, components }
        }
        "show_entity" => {
            let raw = string(required(fields, "id")?)?;
            let id = parse_identifier(&raw)
                .and_then(|id| registry::ENTITY_TYPE.iter().find(|&&t| t == id))
                .ok_or_else(|| Error::new(format!("Unknown registry key in ResourceKey[minecraft:root / minecraft:entity_type]: {raw}")))?;
            let uuid = match required(fields, "uuid")? {
                Nbt::IntArray(ints) if ints.len() == 4 => ints.iter().fold(0u128, |acc, &i| acc << 32 | i as u32 as u128),
                Nbt::String(s) => crate::commands::arguments::java_uuid_from_string(s).ok_or_else(|| Error::new(format!("Invalid UUID {s}")))?,
                other => return Err(Error::new(format!("Not an int array: {}", snbt(other)))),
            };
            let name = match get(fields, "name") {
                Some(name) => Some(Box::new(decode_component(name)?)),
                None => None,
            };
            HoverEvent::ShowEntity { id, uuid, name }
        }
        other => return Err(Error::new(format!("Unknown element name:{other}"))),
    })
}
