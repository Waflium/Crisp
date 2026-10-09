use crate::network::chat::Component;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Visibility {
    Always,
    Never,
    HideForOtherTeams,
    HideForOwnTeam,
}

impl Visibility {
    pub const VALUES: [Visibility; 4] = [Visibility::Always, Visibility::Never, Visibility::HideForOtherTeams, Visibility::HideForOwnTeam];

    pub fn id(self) -> i32 {
        self as i32
    }

    pub fn name(self) -> &'static str {
        match self {
            Visibility::Always => "always",
            Visibility::Never => "never",
            Visibility::HideForOtherTeams => "hideForOtherTeams",
            Visibility::HideForOwnTeam => "hideForOwnTeam",
        }
    }

    pub fn by_name(name: &str) -> Option<Visibility> {
        Visibility::VALUES.into_iter().find(|v| v.name() == name)
    }

    pub fn display_name(self) -> Component {
        Component::translatable(&format!("team.visibility.{}", self.name()))
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CollisionRule {
    Always,
    Never,
    PushOtherTeams,
    PushOwnTeam,
}

impl CollisionRule {
    pub const VALUES: [CollisionRule; 4] = [CollisionRule::Always, CollisionRule::Never, CollisionRule::PushOtherTeams, CollisionRule::PushOwnTeam];

    pub fn id(self) -> i32 {
        self as i32
    }

    pub fn name(self) -> &'static str {
        match self {
            CollisionRule::Always => "always",
            CollisionRule::Never => "never",
            CollisionRule::PushOtherTeams => "pushOtherTeams",
            CollisionRule::PushOwnTeam => "pushOwnTeam",
        }
    }

    pub fn by_name(name: &str) -> Option<CollisionRule> {
        CollisionRule::VALUES.into_iter().find(|v| v.name() == name)
    }

    pub fn display_name(self) -> Component {
        Component::translatable(&format!("team.collision.{}", self.name()))
    }
}
