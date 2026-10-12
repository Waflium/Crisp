use std::borrow::Cow;

use crisp_protocol::Writer;
use crisp_protocol::nbt::Nbt;

use super::waypoint_style_assets;
use crate::world::scores::team_color::TeamColor;

#[derive(Clone, PartialEq, Debug)]
pub struct Icon {
    pub style: Cow<'static, str>,
    pub color: Option<i32>,
}

impl Default for Icon {
    fn default() -> Icon {
        Icon::new()
    }
}

pub static NULL: Icon = Icon::new();

impl Icon {
    pub const fn new() -> Icon {
        Icon { style: Cow::Borrowed(waypoint_style_assets::DEFAULT), color: None }
    }

    pub fn has_data(&self) -> bool {
        self.style != waypoint_style_assets::DEFAULT || self.color.is_some()
    }

    pub fn clone_and_assign_style(&self, team_color: Option<TeamColor>) -> Icon {
        let override_style = self.get_override_style();
        let color_override = self.color.or_else(|| team_color.map(|team_color| if team_color == TeamColor::Black { -13619152 } else { team_color.rgb() }));
        Icon { style: override_style, color: color_override }
    }

    pub fn copy_from(&mut self, other: &Icon) {
        self.color = other.color;
        self.style.clone_from(&other.style);
    }

    fn get_override_style(&self) -> Cow<'static, str> {
        if self.style != waypoint_style_assets::DEFAULT { self.style.clone() } else { Cow::Borrowed(waypoint_style_assets::DEFAULT) }
    }

    pub fn write(&self, w: &mut Writer) {
        w.string(&self.style);
        w.bool(self.color.is_some());
        if let Some(color) = self.color {
            w.u8((color >> 16) as u8);
            w.u8((color >> 8) as u8);
            w.u8(color as u8);
        }
    }

    pub fn to_nbt(&self) -> Nbt {
        let mut fields = vec![("style".to_string(), Nbt::String(self.style.to_string()))];
        if let Some(color) = self.color {
            fields.push(("color".to_string(), Nbt::Int(color)));
        }
        Nbt::Compound(fields)
    }

    pub fn from_nbt(nbt: &Nbt) -> Option<Icon> {
        let Nbt::Compound(_) = nbt else { return None };
        let style = crate::commands::arguments::parse_identifier(nbt.get("style")?.as_str()?)?;
        let color = match nbt.get("color") {
            None => None,
            Some(Nbt::List(values)) if values.len() == 3 => {
                let channel = |value: &Nbt| value.as_f64().map(|v| crisp_core::mth::floor((v as f32 * 255.0) as f64));
                let (r, g, b) = (channel(&values[0])?, channel(&values[1])?, channel(&values[2])?);
                Some(255 << 24 | (r & 255) << 16 | (g & 255) << 8 | b & 255)
            }
            Some(value) => Some(value.as_i64()? as i32),
        };
        Some(Icon { style: Cow::Owned(style), color })
    }
}
