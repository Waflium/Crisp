#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DisplaySlot {
    List,
    Sidebar,
    BelowName,
    TeamBlack,
    TeamDarkBlue,
    TeamDarkGreen,
    TeamDarkAqua,
    TeamDarkRed,
    TeamDarkPurple,
    TeamGold,
    TeamGray,
    TeamDarkGray,
    TeamBlue,
    TeamGreen,
    TeamAqua,
    TeamRed,
    TeamLightPurple,
    TeamYellow,
    TeamWhite,
}

const NAMES: [&str; 19] = [
    "list",
    "sidebar",
    "below_name",
    "sidebar.team.black",
    "sidebar.team.dark_blue",
    "sidebar.team.dark_green",
    "sidebar.team.dark_aqua",
    "sidebar.team.dark_red",
    "sidebar.team.dark_purple",
    "sidebar.team.gold",
    "sidebar.team.gray",
    "sidebar.team.dark_gray",
    "sidebar.team.blue",
    "sidebar.team.green",
    "sidebar.team.aqua",
    "sidebar.team.red",
    "sidebar.team.light_purple",
    "sidebar.team.yellow",
    "sidebar.team.white",
];

impl DisplaySlot {
    pub const VALUES: [DisplaySlot; 19] = {
        use DisplaySlot::*;
        [
            List, Sidebar, BelowName, TeamBlack, TeamDarkBlue, TeamDarkGreen, TeamDarkAqua, TeamDarkRed, TeamDarkPurple, TeamGold, TeamGray, TeamDarkGray, TeamBlue, TeamGreen,
            TeamAqua, TeamRed, TeamLightPurple, TeamYellow, TeamWhite,
        ]
    };

    pub fn id(self) -> i32 {
        self as i32
    }

    pub fn serialized_name(self) -> &'static str {
        NAMES[self as usize]
    }

    pub fn by_name(name: &str) -> Option<DisplaySlot> {
        NAMES.iter().position(|&n| n == name).map(|i| DisplaySlot::VALUES[i])
    }
}
