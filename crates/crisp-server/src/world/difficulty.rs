use crate::network::chat::Component;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Difficulty {
    Peaceful,
    Easy,
    Normal,
    Hard,
}

impl Difficulty {
    pub const VALUES: [Difficulty; 4] = [Difficulty::Peaceful, Difficulty::Easy, Difficulty::Normal, Difficulty::Hard];

    pub fn id(self) -> i32 {
        self as i32
    }

    pub fn key(self) -> &'static str {
        match self {
            Difficulty::Peaceful => "peaceful",
            Difficulty::Easy => "easy",
            Difficulty::Normal => "normal",
            Difficulty::Hard => "hard",
        }
    }

    pub fn by_name(name: &str) -> Option<Difficulty> {
        Self::VALUES.into_iter().find(|d| d.key() == name)
    }

    pub fn display_name(self) -> Component {
        Component::translatable(&format!("options.difficulty.{}", self.key()))
    }
}
