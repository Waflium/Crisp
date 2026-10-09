use std::sync::Arc;
use std::sync::atomic::{AtomicI32, Ordering};

use crisp_data::{GameRule, GameRuleType};
use crisp_protocol::nbt::Nbt;

#[derive(Clone)]
pub struct GameRules(Arc<[AtomicI32]>);

impl Default for GameRules {
    fn default() -> GameRules {
        GameRules(GameRule::all().map(|r| AtomicI32::new(r.info().default)).collect())
    }
}

impl GameRules {
    pub fn has(&self, rule: GameRule) -> bool {
        !rule.info().experimental
    }

    pub fn available(&self) -> impl Iterator<Item = GameRule> + '_ {
        GameRule::all().filter(|&r| self.has(r))
    }

    pub fn get_bool(&self, rule: GameRule) -> bool {
        debug_assert_eq!(rule.info().kind, GameRuleType::Bool);
        self.get(rule) != 0
    }

    pub fn get_int(&self, rule: GameRule) -> i32 {
        debug_assert_eq!(rule.info().kind, GameRuleType::Int);
        self.get(rule)
    }

    pub fn get(&self, rule: GameRule) -> i32 {
        self.0[rule.0 as usize].load(Ordering::Relaxed)
    }

    pub fn set(&self, rule: GameRule, value: i32) {
        self.0[rule.0 as usize].store(value, Ordering::Relaxed);
    }

    pub fn serialize(rule: GameRule, value: i32) -> String {
        match rule.info().kind {
            GameRuleType::Bool => (value != 0).to_string(),
            GameRuleType::Int => value.to_string(),
        }
    }

    pub fn to_nbt(&self) -> Nbt {
        Nbt::Compound(
            self.available()
                .map(|r| {
                    let value = self.get(r);
                    let tag = match r.info().kind {
                        GameRuleType::Bool => Nbt::Byte(value as i8),
                        GameRuleType::Int => Nbt::Int(value),
                    };
                    (format!("minecraft:{}", r.info().name), tag)
                })
                .collect(),
        )
    }

    pub fn load(&self, data: &Nbt) {
        let Nbt::Compound(entries) = data else { return };
        for (name, tag) in entries {
            let Some(rule) = GameRule::by_name(name).filter(|&r| self.has(r)) else { continue };
            let info = rule.info();
            let value = match (info.kind, tag) {
                (GameRuleType::Bool, Nbt::Byte(b)) => (*b != 0) as i32,
                (GameRuleType::Int, Nbt::Int(i)) if (info.min..=info.max).contains(i) => *i,
                _ => continue,
            };
            self.set(rule, value);
        }
    }
}
