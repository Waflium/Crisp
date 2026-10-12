use crisp_data::registry;

use super::stat::Stat;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum StatType {
    Mined,
    Crafted,
    Used,
    Broken,
    PickedUp,
    Dropped,
    Killed,
    KilledBy,
    Custom,
}

impl StatType {
    pub const VALUES: [StatType; 9] = [
        StatType::Mined,
        StatType::Crafted,
        StatType::Used,
        StatType::Broken,
        StatType::PickedUp,
        StatType::Dropped,
        StatType::Killed,
        StatType::KilledBy,
        StatType::Custom,
    ];

    pub fn id(self) -> i32 {
        self as i32
    }

    pub fn key(self) -> &'static str {
        registry::STAT_TYPE[self as usize]
    }

    pub fn by_key(key: &str) -> Option<StatType> {
        StatType::VALUES.into_iter().find(|t| t.key() == key)
    }

    pub fn by_id(id: i32) -> Option<StatType> {
        StatType::VALUES.get(usize::try_from(id).ok()?).copied()
    }

    pub fn get_registry(self) -> &'static [&'static str] {
        match self {
            StatType::Mined => registry::BLOCK,
            StatType::Crafted | StatType::Used | StatType::Broken | StatType::PickedUp | StatType::Dropped => registry::ITEM,
            StatType::Killed | StatType::KilledBy => registry::ENTITY_TYPE,
            StatType::Custom => registry::CUSTOM_STAT,
        }
    }

    pub fn registry_key(self) -> &'static str {
        match self {
            StatType::Mined => "minecraft:block",
            StatType::Crafted | StatType::Used | StatType::Broken | StatType::PickedUp | StatType::Dropped => "minecraft:item",
            StatType::Killed | StatType::KilledBy => "minecraft:entity_type",
            StatType::Custom => "minecraft:custom_stat",
        }
    }

    pub const fn get(self, value: u16) -> Stat {
        Stat { stat_type: self, value }
    }
}
