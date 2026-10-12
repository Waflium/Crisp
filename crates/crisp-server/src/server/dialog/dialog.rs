use std::sync::LazyLock;

use crisp_protocol::nbt::Nbt;
use crisp_protocol::{Reader, Writer};

use crate::commands::arguments::resource_or_id_argument::RawResourceOrId;
use crate::commands::functions::string_template::{StringTemplate, is_valid_variable_name};
use crate::util::hash_ops::{self, HashCode};
use crate::world::item::component::codec::{self, Codec, Custom, Dispatch, Field, RangeError, Selector, Stream};
use crate::world::item::component::custom;
use crate::world::item::component::text::text;
use crate::world::item::component::types::{
    Pair, b, bool, compound, def, empty_list, float, i, identifier, list, opt, positive_float, positive_int, record, req, s, string,
};

const DIALOG_ACTIONS: &[&str] = &["close", "none", "wait_for_response"];

fn width() -> Pair {
    (Codec::Int(1, 1024, RangeError::Outside), Stream::VarInt)
}

fn int_range(min: i32, max: i32) -> Pair {
    (Codec::Int(min, max, RangeError::Outside), Stream::VarInt)
}

fn chat_string() -> Pair {
    (Codec::Normalize(Box::new(Codec::string()), validate_chat_string), Stream::Utf8(32767))
}

fn validate_chat_string(value: Nbt) -> Result<Nbt, String> {
    let text = value.as_str().unwrap_or_default();
    if let Some(c) = text.encode_utf16().find(|&c| c == 0xA7 || c < 0x20 || c == 0x7F) {
        return Err(format!("Disallowed chat character: '{}'", char::from_u32(c as u32).unwrap_or('\u{FFFD}')));
    }
    Ok(value)
}

fn untrusted_uri() -> Pair {
    (Codec::Normalize(Box::new(Codec::string()), validate_untrusted_uri), Stream::Utf8(32767))
}

fn validate_untrusted_uri(value: Nbt) -> Result<Nbt, String> {
    let uri = value.as_str().unwrap_or_default();
    let scheme = uri
        .split_once(':')
        .map(|(scheme, _)| scheme)
        .filter(|scheme| scheme.chars().next().is_some_and(|c| c.is_ascii_alphabetic()) && scheme.chars().all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c)));
    match scheme {
        None => Err(format!("Missing protocol in URI: {uri}: {uri}")),
        Some(scheme) if !matches!(scheme.to_ascii_lowercase().as_str(), "http" | "https") => Err(format!("Unsupported protocol in URI: {uri}: {uri}")),
        Some(_) => Ok(value),
    }
}

fn variable() -> Pair {
    (Codec::Normalize(Box::new(Codec::string()), validate_variable), Stream::Utf8(32767))
}

fn validate_variable(value: Nbt) -> Result<Nbt, String> {
    let name = value.as_str().unwrap_or_default();
    if is_valid_variable_name(name) { Ok(value) } else { Err(format!("{name} is not a valid input name")) }
}

fn parsed_template() -> Pair {
    (Codec::Normalize(Box::new(Codec::string()), validate_template), Stream::Utf8(32767))
}

fn validate_template(value: Nbt) -> Result<Nbt, String> {
    let raw = value.as_str().unwrap_or_default();
    match StringTemplate::from_string(raw) {
        Ok(_) => Ok(value),
        Err(message) => Err(format!("Failed to parse template {raw}: {message}")),
    }
}

fn dialog_holder() -> Pair {
    (Codec::Holder("minecraft:dialog", Some(Box::new(Codec::Lazy(direct_codec)))), Stream::Nbt)
}

fn action() -> Pair {
    let cases = vec![
        ("minecraft:open_url", vec![req("url", untrusted_uri())]),
        ("minecraft:run_command", vec![req("command", chat_string())]),
        ("minecraft:suggest_command", vec![req("command", chat_string())]),
        ("minecraft:show_dialog", vec![req("dialog", dialog_holder())]),
        ("minecraft:change_page", vec![req("page", (positive_int(), Stream::VarInt))]),
        ("minecraft:copy_to_clipboard", vec![req("value", string())]),
        ("minecraft:custom", vec![req("id", identifier()), opt("payload", (Codec::AnyTag, Stream::Nbt))]),
        ("minecraft:dynamic/run_command", vec![req("template", parsed_template())]),
        ("minecraft:dynamic/custom", vec![req("id", identifier()), opt("additions", (Codec::Compound, Stream::Nbt))]),
    ];
    (Codec::Dispatch(Box::new(Dispatch { key: "type", selector: Selector::Registry("minecraft:dialog_action_type"), cases })), Stream::Dispatch)
}

fn action_button_fields() -> Vec<Field> {
    vec![req("label", text()), opt("tooltip", text()), def("width", width(), i(150)), opt("action", action())]
}

fn action_button() -> Pair {
    record(action_button_fields())
}

fn plain_message_fields() -> Vec<Field> {
    vec![req("contents", text()), def("width", width(), i(200))]
}

fn plain_message() -> Pair {
    (Codec::Alt(Box::new(Codec::Record(plain_message_fields())), Box::new(text().0), |v| Ok(compound(&[("contents", v.clone())]))), Stream::Record)
}

fn dialog_body() -> Pair {
    let cases = vec![
        (
            "minecraft:item",
            vec![
                req("item", custom::template()),
                opt("description", plain_message()),
                def("show_decorations", bool(), b(true)),
                def("show_tooltip", bool(), b(true)),
                def("width", int_range(1, 256), i(16)),
                def("height", int_range(1, 256), i(16)),
            ],
        ),
        ("minecraft:plain_message", plain_message_fields()),
    ];
    (Codec::Dispatch(Box::new(Dispatch { key: "type", selector: Selector::Registry("minecraft:dialog_body_type"), cases })), Stream::Dispatch)
}

fn single_option_entry() -> Pair {
    let full = Codec::Record(vec![req("id", string()), opt("display", text()), def("initial", bool(), b(false))]);
    (Codec::Alt(Box::new(full), Box::new(Codec::string()), |v| Ok(compound(&[("id", v.clone())]))), Stream::Record)
}

fn input() -> Pair {
    let key = || req("key", variable());
    let cases = vec![
        (
            "minecraft:boolean",
            vec![key(), req("label", text()), def("initial", bool(), b(false)), def("on_true", string(), s("true")), def("on_false", string(), s("false"))],
        ),
        (
            "minecraft:number_range",
            vec![
                key(),
                def("width", width(), i(200)),
                req("label", text()),
                def("label_format", string(), s("options.generic_value")),
                req("start", float(Codec::float())),
                req("end", float(Codec::float())),
                opt("initial", float(Codec::float())),
                opt("step", float(positive_float())),
            ],
        ),
        (
            "minecraft:single_option",
            vec![
                key(),
                def("width", width(), i(200)),
                req("options", (Codec::List(Box::new(single_option_entry().0), 1, i32::MAX as usize), Stream::Nbt)),
                req("label", text()),
                def("label_visible", bool(), b(true)),
            ],
        ),
        (
            "minecraft:text",
            vec![
                key(),
                def("width", width(), i(200)),
                req("label", text()),
                def("label_visible", bool(), b(true)),
                def("initial", string(), s("")),
                def("max_length", (positive_int(), Stream::VarInt), i(32)),
                opt("multiline", record(vec![opt("max_lines", (positive_int(), Stream::VarInt)), opt("height", int_range(1, 512))])),
            ],
        ),
    ];
    let dispatch = Codec::Dispatch(Box::new(Dispatch { key: "type", selector: Selector::Registry("minecraft:input_control_type"), cases }));
    (Codec::Normalize(Box::new(dispatch), validate_input), Stream::Dispatch)
}

fn validate_input(value: Nbt) -> Result<Nbt, String> {
    match value.get("type").and_then(Nbt::as_str) {
        Some("minecraft:number_range") => {
            if let Some(initial) = value.get("initial").map(codec::as_f32) {
                let (start, end) = (value.get("start").map_or(0.0, codec::as_f32), value.get("end").map_or(0.0, codec::as_f32));
                let (initial, min, max) = (initial as f64, start.min(end) as f64, start.max(end) as f64);
                if initial < min || initial > max {
                    let d = crate::nbt::java_double_to_string;
                    return Err(format!("Initial value {} is outside of range [{}, {}]", d(initial), d(min), d(max)));
                }
            }
        }
        Some("minecraft:single_option") => {
            let initial_count = value.get("options").map(Nbt::as_list).unwrap_or_default().iter().filter(|o| o.get("initial").is_some_and(|i| codec::as_i64(i) != 0)).count();
            if initial_count > 1 {
                return Err("Multiple initial values".into());
            }
        }
        Some("minecraft:text") => {
            let initial = value.get("initial").and_then(Nbt::as_str).unwrap_or_default().encode_utf16().count() as i64;
            let max_length = value.get("max_length").map_or(32, codec::as_i64);
            if initial > max_length {
                return Err("Default text length exceeds allowed size".into());
            }
        }
        _ => {}
    }
    Ok(value)
}

fn common_dialog_data() -> Vec<Field> {
    vec![
        req("title", text()),
        opt("external_title", text()),
        def("can_close_with_escape", bool(), b(true)),
        def("pause", bool(), b(true)),
        def("after_action", (Codec::Enum(DIALOG_ACTIONS), Stream::Enum), s("close")),
        def("body", (Codec::Compact(Box::new(dialog_body().0)), Stream::Nbt), empty_list()),
        def("inputs", list(input()), empty_list()),
    ]
}

fn with_common(fields: Vec<Field>) -> Vec<Field> {
    let mut all = common_dialog_data();
    all.extend(fields);
    all
}

fn default_action() -> Nbt {
    Codec::Record(action_button_fields()).decode(&compound(&[("label", compound(&[("translate", s("gui.ok"))]))])).expect("default action")
}

static DIRECT: LazyLock<Pair> = LazyLock::new(|| {
    let cases = vec![
        ("minecraft:notice", with_common(vec![def("action", action_button(), default_action())])),
        ("minecraft:server_links", with_common(vec![opt("exit_action", action_button()), def("columns", (positive_int(), Stream::VarInt), i(2)), def("button_width", width(), i(150))])),
        (
            "minecraft:dialog_list",
            with_common(vec![
                req("dialogs", (Codec::Custom(Box::new(DialogListCodec)), Stream::Nbt)),
                opt("exit_action", action_button()),
                def("columns", (positive_int(), Stream::VarInt), i(2)),
                def("button_width", width(), i(150)),
            ]),
        ),
        (
            "minecraft:multi_action",
            with_common(vec![
                req("actions", (Codec::List(Box::new(action_button().0), 1, i32::MAX as usize), Stream::Nbt)),
                opt("exit_action", action_button()),
                def("columns", (positive_int(), Stream::VarInt), i(2)),
            ]),
        ),
        ("minecraft:confirmation", with_common(vec![req("yes", action_button()), req("no", action_button())])),
    ];
    let dispatch = Codec::Dispatch(Box::new(Dispatch { key: "type", selector: Selector::Registry("minecraft:dialog_type"), cases }));
    (Codec::Normalize(Box::new(dispatch), validate_common), Stream::Nbt)
});

fn validate_common(value: Nbt) -> Result<Nbt, String> {
    let pause = value.get("pause").is_none_or(|p| codec::as_i64(p) != 0);
    let will_unpause = matches!(value.get("after_action").and_then(Nbt::as_str).unwrap_or("close"), "close" | "wait_for_response");
    if pause && !will_unpause {
        return Err("Dialogs that pause the game must use after_action values that unpause it after user action!".into());
    }
    Ok(value)
}

fn direct_codec() -> &'static Pair {
    &DIRECT
}

pub fn validate(tag: &Nbt) -> Result<(), String> {
    DIRECT.0.decode(tag).map(drop)
}

pub fn write_holder(dialog: &RawResourceOrId, w: &mut Writer) {
    match dialog {
        RawResourceOrId::Reference(id) => w.varint(crisp_data::SyncedRegistry::get("minecraft:dialog").id_of(id).expect("registered dialog") as i32 + 1),
        RawResourceOrId::Inline(value) => {
            w.varint(0);
            DIRECT.0.decode(value).expect("validated dialog").write(w);
        }
    }
}

struct DialogListCodec;

impl Custom for DialogListCodec {
    fn decode(&self, tag: &Nbt) -> Result<Nbt, String> {
        if let Nbt::String(raw) = tag
            && let Some(tag_id) = raw.strip_prefix('#')
        {
            let id = codec::identifier(&Nbt::String(tag_id.into()))?;
            return if codec::tag_exists("minecraft:dialog", &id) { Ok(Nbt::String(format!("#{id}"))) } else { Err(format!("Missing tag: '{id}' in 'minecraft:dialog'")) };
        }
        Codec::Compact(Box::new(dialog_holder().0)).decode(tag)
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
