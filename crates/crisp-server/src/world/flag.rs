
const NAMES: [&str; 4] = ["minecraft:vanilla", "minecraft:trade_rebalance", "minecraft:redstone_experiments", "minecraft:minecart_improvements"];

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct FeatureFlagSet(u8);

impl FeatureFlagSet {
    pub const DEFAULT_FLAGS: FeatureFlagSet = FeatureFlagSet(1);

    pub fn of(name: &str) -> Option<FeatureFlagSet> {
        NAMES.iter().position(|&n| n == name).map(|i| FeatureFlagSet(1 << i))
    }

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub fn is_subset_of(self, other: FeatureFlagSet) -> bool {
        self.0 & !other.0 == 0
    }

    pub fn join(self, other: FeatureFlagSet) -> FeatureFlagSet {
        FeatureFlagSet(self.0 | other.0)
    }

    pub fn from_names<'a>(names: impl IntoIterator<Item = &'a str>) -> Result<FeatureFlagSet, String> {
        let mut set = FeatureFlagSet::default();
        let mut unknown = Vec::new();
        for name in names {
            let id = crate::commands::arguments::parse_identifier(name).unwrap_or_else(|| name.to_string());
            match FeatureFlagSet::of(&id) {
                Some(flag) => set = set.join(flag),
                None => unknown.push(id),
            }
        }
        if unknown.is_empty() { Ok(set) } else { Err(format!("Unknown feature ids: [{}]", unknown.join(", "))) }
    }
}

pub fn print_missing_flags(allowed: FeatureFlagSet, requested: FeatureFlagSet) -> String {
    let missing: Vec<&str> = (0..NAMES.len()).filter(|&i| requested.0 & (1 << i) != 0 && allowed.0 & (1 << i) == 0).map(|i| NAMES[i]).collect();
    missing.join(", ")
}
