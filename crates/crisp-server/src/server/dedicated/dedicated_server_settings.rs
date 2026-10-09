use std::path::{Path, PathBuf};

use super::dedicated_server_properties::{DedicatedServerProperties, InvalidProperty};
use super::settings;

pub struct DedicatedServerSettings {
    source: PathBuf,
    properties: DedicatedServerProperties,
}

impl DedicatedServerSettings {
    pub fn new(source: &Path) -> Result<DedicatedServerSettings, InvalidProperty> {
        let properties = DedicatedServerProperties::new(settings::load_from_file(source))?;
        Ok(DedicatedServerSettings { source: source.to_path_buf(), properties })
    }

    pub fn properties(&self) -> &DedicatedServerProperties {
        &self.properties
    }

    pub fn force_save(&self) {
        settings::store_to_file(&self.properties.properties, &self.source);
    }

    pub fn update(&mut self, key: &str, value: String) {
        match self.properties.with(key, value) {
            Ok(properties) => {
                self.properties = properties;
                self.force_save();
            }
            Err(InvalidProperty(e)) => error!("Failed to update {key}: {e}"),
        }
    }
}
