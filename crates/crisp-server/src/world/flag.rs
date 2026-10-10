use crisp_core::hash_map::java_hash_map_order;

use crate::util::java_collections::identifier_hash;

const NAMES: [&str; 4] = ["minecraft:vanilla", "minecraft:trade_rebalance", "minecraft:redstone_experiments", "minecraft:minecart_improvements"];

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct FeatureFlagSet(u8);

impl FeatureFlagSet {
    pub const VANILLA_SET: FeatureFlagSet = FeatureFlagSet(1);
    pub const DEFAULT_FLAGS: FeatureFlagSet = FeatureFlagSet::VANILLA_SET;
    pub const ALL_FLAGS: FeatureFlagSet = FeatureFlagSet((1 << NAMES.len()) - 1);

    pub fn of(name: &str) -> Option<FeatureFlagSet> {
        NAMES.iter().position(|&n| n == name).map(|i| FeatureFlagSet(1 << i))
    }

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub fn is_subset_of(self, other: FeatureFlagSet) -> bool {
        self.0 & !other.0 == 0
    }

    pub fn intersects(self, other: FeatureFlagSet) -> bool {
        self.0 & other.0 != 0
    }

    pub fn join(self, other: FeatureFlagSet) -> FeatureFlagSet {
        FeatureFlagSet(self.0 | other.0)
    }

    pub fn subtract(self, other: FeatureFlagSet) -> FeatureFlagSet {
        FeatureFlagSet(self.0 & !other.0)
    }

    pub fn to_names(self) -> Vec<&'static str> {
        let names: Vec<&'static str> = (0..NAMES.len()).filter(|&i| self.0 & (1 << i) != 0).map(|i| NAMES[i]).collect();
        java_hash_set_order(names)
    }

    pub fn from_names<'a>(names: impl IntoIterator<Item = &'a str>) -> Result<FeatureFlagSet, String> {
        let mut set = FeatureFlagSet::default();
        let mut unknown: Vec<String> = Vec::new();
        for name in names {
            let id = crate::resources::identifier::read(name)?;
            match FeatureFlagSet::of(&id) {
                Some(flag) => set = set.join(flag),
                None if !unknown.contains(&id) => unknown.push(id),
                None => {}
            }
        }
        if unknown.is_empty() { Ok(set) } else { Err(format!("Unknown feature ids: [{}]", java_hash_set_order(unknown).join(", "))) }
    }
}

fn java_hash_set_order<S: AsRef<str>>(ids: Vec<S>) -> Vec<S> {
    let hashes: Vec<i32> = ids.iter().map(|id| identifier_hash(id.as_ref())).collect();
    let mut slots: Vec<Option<S>> = ids.into_iter().map(Some).collect();
    java_hash_map_order(&hashes).into_iter().map(|i| slots[i].take().unwrap()).collect()
}

pub fn print_missing_flags(allowed: FeatureFlagSet, requested: FeatureFlagSet) -> String {
    let allowed = allowed.to_names();
    requested.to_names().into_iter().filter(|f| !allowed.contains(f)).collect::<Vec<_>>().join(", ")
}
