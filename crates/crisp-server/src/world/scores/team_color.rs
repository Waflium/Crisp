use super::display_slot::DisplaySlot;
use crate::network::chat::TextColor;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TeamColor {
    Black,
    DarkBlue,
    DarkGreen,
    DarkAqua,
    DarkRed,
    DarkPurple,
    Gold,
    Gray,
    DarkGray,
    Blue,
    Green,
    Aqua,
    Red,
    LightPurple,
    Yellow,
    White,
}

const NAMES: [&str; 16] =
    ["black", "dark_blue", "dark_green", "dark_aqua", "dark_red", "dark_purple", "gold", "gray", "dark_gray", "blue", "green", "aqua", "red", "light_purple", "yellow", "white"];

impl TeamColor {
    pub const VALUES: [TeamColor; 16] = {
        use TeamColor::*;
        [Black, DarkBlue, DarkGreen, DarkAqua, DarkRed, DarkPurple, Gold, Gray, DarkGray, Blue, Green, Aqua, Red, LightPurple, Yellow, White]
    };

    pub fn id(self) -> i32 {
        self as i32
    }

    pub fn by_id(id: i32) -> TeamColor {
        TeamColor::VALUES.get(id as usize).copied().unwrap_or(TeamColor::Black)
    }

    pub fn serialized_name(self) -> &'static str {
        NAMES[self as usize]
    }

    pub fn by_name(name: &str) -> Option<TeamColor> {
        NAMES.iter().position(|&n| n == name).map(|i| TeamColor::VALUES[i])
    }

    pub fn text_color(self) -> TextColor {
        TextColor::named(self as usize)
    }

    pub fn display_slot(self) -> DisplaySlot {
        DisplaySlot::VALUES[3 + self as usize]
    }
}
