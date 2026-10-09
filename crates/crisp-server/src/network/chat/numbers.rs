use crisp_protocol::Writer;
use crisp_protocol::nbt::Nbt;

use super::{ChatFormatting, Component, Style};

#[derive(Clone, Debug, PartialEq)]
pub enum NumberFormat {
    Blank,
    Styled(Style),
    Fixed(Component),
}

impl NumberFormat {
    pub fn no_style() -> NumberFormat {
        NumberFormat::Styled(Style::default())
    }

    pub fn sidebar_default() -> NumberFormat {
        NumberFormat::Styled(Style { color: Some(ChatFormatting::Red.color()), ..Style::default() })
    }

    pub fn format(&self, value: i32) -> Component {
        match self {
            NumberFormat::Blank => Component::empty(),
            NumberFormat::Styled(style) => Component::literal(value.to_string()).with_style(style),
            NumberFormat::Fixed(component) => component.clone(),
        }
    }

    fn type_id(&self) -> i32 {
        match self {
            NumberFormat::Blank => 0,
            NumberFormat::Styled(_) => 1,
            NumberFormat::Fixed(_) => 2,
        }
    }

    fn type_name(&self) -> &'static str {
        match self {
            NumberFormat::Blank => "minecraft:blank",
            NumberFormat::Styled(_) => "minecraft:styled",
            NumberFormat::Fixed(_) => "minecraft:fixed",
        }
    }

    pub fn to_nbt(&self) -> Nbt {
        let mut fields = vec![("type".to_string(), Nbt::String(self.type_name().into()))];
        match self {
            NumberFormat::Blank => {}
            NumberFormat::Styled(style) => style.write_fields(&mut fields),
            NumberFormat::Fixed(component) => fields.push(("value".into(), component.to_nbt())),
        }
        Nbt::Compound(fields)
    }

    pub fn from_nbt(tag: &Nbt) -> Result<NumberFormat, String> {
        use super::component_serialization::{decode, decode_style_fields};
        let Nbt::Compound(fields) = tag else { return Err(format!("Not a map: {}", crate::nbt::to_string(tag))) };
        let get = |key: &str| fields.iter().find(|(k, _)| k == key).map(|(_, v)| v);
        let Some(kind) = get("type") else { return Err(format!("No key type in MapLike[{}]", crate::nbt::to_string(tag))) };
        let Some(kind) = kind.as_str() else { return Err("Not a string".into()) };
        match crate::commands::arguments::parse_identifier(kind).as_deref() {
            Some("minecraft:blank") => Ok(NumberFormat::Blank),
            Some("minecraft:styled") => Ok(NumberFormat::Styled(decode_style_fields(fields)?)),
            Some("minecraft:fixed") => match get("value") {
                Some(value) => Ok(NumberFormat::Fixed(decode(value)?)),
                None => Err(format!("No key value in MapLike[{}]", crate::nbt::to_string(tag))),
            },
            _ => Err(format!("Unknown registry key in ResourceKey[minecraft:root / minecraft:number_format_type]: {kind}")),
        }
    }

    pub fn write(&self, w: &mut Writer) {
        w.varint(self.type_id());
        match self {
            NumberFormat::Blank => {}
            NumberFormat::Styled(style) => style.to_nbt().write(w),
            NumberFormat::Fixed(component) => component.write(w),
        }
    }

    pub fn write_optional(format: Option<&NumberFormat>, w: &mut Writer) {
        w.bool(format.is_some());
        if let Some(format) = format {
            format.write(w);
        }
    }
}
