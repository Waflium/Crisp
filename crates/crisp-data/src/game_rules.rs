use crate::GAME_RULES;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct GameRule(pub u8);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GameRuleType {
    Bool,
    Int,
}

pub struct GameRuleInfo {
    pub name: &'static str,
    pub category: &'static str,
    pub kind: GameRuleType,
    pub default: i32,
    pub min: i32,
    pub max: i32,
    pub experimental: bool,
}

impl GameRule {
    pub fn info(self) -> &'static GameRuleInfo {
        &GAME_RULES[self.0 as usize]
    }

    pub fn count() -> usize {
        GAME_RULES.len()
    }

    pub fn all() -> impl Iterator<Item = GameRule> {
        (0..GAME_RULES.len()).map(|i| GameRule(i as u8))
    }

    pub fn by_name(name: &str) -> Option<GameRule> {
        let path = name.strip_prefix("minecraft:").unwrap_or(name);
        GAME_RULES.iter().position(|r| r.name == path).map(|i| GameRule(i as u8))
    }
}
