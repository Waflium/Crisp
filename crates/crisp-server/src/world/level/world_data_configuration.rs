use crisp_protocol::nbt::Nbt;

use super::data_pack_config::DataPackConfig;
use crate::world::flag::FeatureFlagSet;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct WorldDataConfiguration {
    pub data_packs: DataPackConfig,
    pub enabled_features: FeatureFlagSet,
}

impl WorldDataConfiguration {
    pub fn new(data_packs: DataPackConfig, enabled_features: FeatureFlagSet) -> WorldDataConfiguration {
        WorldDataConfiguration { data_packs, enabled_features }
    }

    pub fn default_configuration() -> WorldDataConfiguration {
        WorldDataConfiguration::new(DataPackConfig::default(), FeatureFlagSet::DEFAULT_FLAGS)
    }

    pub fn parse(level_data: &Nbt) -> WorldDataConfiguration {
        let data_packs = level_data.get("DataPacks").and_then(DataPackConfig::parse).unwrap_or_default();
        let enabled_features = level_data
            .get("enabled_features")
            .and_then(|features| match features {
                Nbt::List(items) => items.iter().map(Nbt::as_str).collect::<Option<Vec<&str>>>(),
                _ => None,
            })
            .and_then(|names| FeatureFlagSet::from_names(names).ok())
            .unwrap_or(FeatureFlagSet::DEFAULT_FLAGS);
        WorldDataConfiguration::new(data_packs, enabled_features)
    }

    pub fn encoded_enabled_features(&self) -> Option<Nbt> {
        (self.enabled_features != FeatureFlagSet::DEFAULT_FLAGS)
            .then(|| Nbt::List(self.enabled_features.to_names().into_iter().map(|name| Nbt::String(name.to_string())).collect()))
    }
}
