use crate::{BUILTIN_REGISTRIES, PACKET_NAMES, SYNCED_REGISTRIES, TAGS};

pub fn registry_entries(name: &str) -> Option<&'static [&'static str]> {
    BUILTIN_REGISTRIES
        .iter()
        .find(|r| r.0 == name)
        .map(|r| r.1)
        .or_else(|| SYNCED_REGISTRIES.iter().find(|r| r.name == name).map(|r| r.entries))
}

pub struct SyncedRegistry {
    pub name: &'static str,
    pub entries: &'static [&'static str],
}

impl SyncedRegistry {
    pub fn get(name: &str) -> &'static SyncedRegistry {
        SYNCED_REGISTRIES
            .iter()
            .find(|r| r.name == name)
            .unwrap_or_else(|| panic!("{name} is not a synchronized registry"))
    }

    pub fn id_of(&self, entry: &str) -> Option<i32> {
        self.entries.iter().position(|&e| e == entry).map(|i| i as i32)
    }
}

pub struct RegistryTags {
    pub registry: &'static str,
    pub tags: &'static [(&'static str, &'static [i32])],
}

pub fn registry_id(entries: &[&str], name: &str) -> i32 {
    entries.iter().position(|&e| e == name).unwrap_or_else(|| panic!("{name} is not registered")) as i32
}

pub fn packet_name(state: &str, direction: &str, id: i32) -> Option<&'static str> {
    PACKET_NAMES.iter().find(|p| p.0 == state && p.1 == direction && p.2 == id).map(|p| p.3)
}

pub fn tag(registry: &str, name: &str) -> &'static [i32] {
    TAGS.iter()
        .find(|r| r.registry == registry)
        .and_then(|r| r.tags.iter().find(|t| t.0 == name))
        .map_or(&[], |t| t.1)
}

pub struct DimensionType {
    pub name: &'static str,
    pub min_y: i32,
    pub height: u32,
    pub has_skylight: bool,
    pub has_ceiling: bool,
    pub has_fixed_time: bool,
    pub infiniburn: &'static crate::Tag,
    pub default_clock: Option<&'static str>,
    pub attributes: &'static str,
    pub timelines: &'static str,
}
