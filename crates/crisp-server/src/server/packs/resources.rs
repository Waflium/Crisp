
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use regex_lite::Regex;
use serde_json::Value;

use super::{IoSupplier, PackResources, Resource};

struct IdentifierPattern {
    namespace: Option<Regex>,
    path: Option<Regex>,
}

pub struct ResourceFilterSection {
    block_list: Vec<IdentifierPattern>,
}

impl ResourceFilterSection {
    pub fn parse(value: &Value) -> Result<ResourceFilterSection, String> {
        let block = value.get("block").ok_or("No key block in MapLike")?;
        let block = block.as_array().ok_or_else(|| format!("Not a list: {block}"))?;
        let pattern = |entry: &Value, key: &str| -> Result<Option<Regex>, String> {
            match entry.get(key) {
                None => Ok(None),
                Some(Value::String(p)) => Regex::new(p).map(Some).map_err(|e| format!("Invalid regex pattern '{p}': {e}")),
                Some(other) => Err(format!("Not a string: {other}")),
            }
        };
        let block_list = block.iter().map(|entry| Ok(IdentifierPattern { namespace: pattern(entry, "namespace")?, path: pattern(entry, "path")? })).collect::<Result<_, String>>()?;
        Ok(ResourceFilterSection { block_list })
    }

    pub fn is_namespace_filtered(&self, namespace: &str) -> bool {
        self.block_list.iter().any(|p| p.namespace.as_ref().is_none_or(|r| r.is_match(namespace)))
    }

    pub fn is_path_filtered(&self, path: &str) -> bool {
        self.block_list.iter().any(|p| p.path.as_ref().is_none_or(|r| r.is_match(path)))
    }
}

struct PackEntry {
    resources: Option<Arc<PackResources>>,
    filter: Option<Arc<ResourceFilterSection>>,
}

impl PackEntry {
    fn is_filtered(&self, location: &str) -> bool {
        self.filter.as_ref().is_some_and(|f| f.is_path_filtered(location.split_once(':').map_or(location, |(_, path)| path)))
    }
}

struct FallbackResourceManager {
    namespace: String,
    fallbacks: Vec<PackEntry>,
}

impl FallbackResourceManager {
    fn list_resources(&self, directory: &str, selector: &dyn Fn(&str) -> bool, result: &mut BTreeMap<String, Resource>) {
        let mut top: HashMap<String, Resource> = HashMap::new();
        for entry in &self.fallbacks {
            if entry.filter.is_some() {
                top.retain(|location, _| !entry.is_filtered(location));
            }
            if let Some(pack) = &entry.resources {
                pack.list_resources(&self.namespace, directory, &mut |id: String, supplier: IoSupplier| {
                    if !is_metadata(&id) && selector(&id) {
                        top.insert(id, Resource { source_pack_id: pack.pack_id().to_string(), supplier });
                    }
                });
            }
        }
        result.extend(top);
    }

    fn list_resource_stacks(&self, directory: &str, selector: &dyn Fn(&str) -> bool, result: &mut BTreeMap<String, Vec<Resource>>) {
        let mut found: HashMap<String, Vec<Resource>> = HashMap::new();
        for entry in &self.fallbacks {
            if entry.filter.is_some() {
                for (location, sources) in found.iter_mut() {
                    if entry.is_filtered(location) {
                        sources.clear();
                    }
                }
            }
            if let Some(pack) = &entry.resources {
                pack.list_resources(&self.namespace, directory, &mut |id: String, supplier: IoSupplier| {
                    if !is_metadata(&id) && selector(&id) {
                        found.entry(id).or_default().push(Resource { source_pack_id: pack.pack_id().to_string(), supplier });
                    }
                });
            }
        }
        result.extend(found.into_iter().filter(|(_, sources)| !sources.is_empty()));
    }
}

fn is_metadata(location: &str) -> bool {
    location.ends_with(".mcmeta")
}

pub struct MultiPackResourceManager {
    namespaced_managers: HashMap<String, FallbackResourceManager>,
}

impl MultiPackResourceManager {
    pub fn new(packs: Vec<PackResources>) -> MultiPackResourceManager {
        let packs: Vec<Arc<PackResources>> = packs.into_iter().map(Arc::new).collect();
        let provided: Vec<_> = packs.iter().map(|p| p.get_namespaces()).collect();
        let mut namespaces: Vec<String> = Vec::new();
        for set in &provided {
            for namespace in set {
                if !namespaces.contains(namespace) {
                    namespaces.push(namespace.clone());
                }
            }
        }
        let mut namespaced_managers: HashMap<String, FallbackResourceManager> = HashMap::new();
        for (pack, provided_namespaces) in packs.iter().zip(&provided) {
            let filter = pack_filter_section(pack).map(Arc::new);
            for namespace in &namespaces {
                let contains_namespace = provided_namespaces.contains(namespace);
                let filter_matches_namespace = filter.as_ref().is_some_and(|f| f.is_namespace_filtered(namespace));
                if !contains_namespace && !filter_matches_namespace {
                    continue;
                }
                let manager = namespaced_managers.entry(namespace.clone()).or_insert_with(|| FallbackResourceManager { namespace: namespace.clone(), fallbacks: Vec::new() });
                manager.fallbacks.push(PackEntry {
                    resources: contains_namespace.then(|| pack.clone()),
                    filter: filter_matches_namespace.then(|| filter.clone().unwrap()),
                });
            }
        }
        MultiPackResourceManager { namespaced_managers }
    }

    pub fn list_resources(&self, directory: &str, extension: &str) -> BTreeMap<String, Resource> {
        let mut result = BTreeMap::new();
        for manager in self.namespaced_managers.values() {
            manager.list_resources(directory, &|id| id.ends_with(extension), &mut result);
        }
        result
    }

    pub fn list_resource_stacks(&self, directory: &str, extension: &str) -> BTreeMap<String, Vec<Resource>> {
        let mut result = BTreeMap::new();
        for manager in self.namespaced_managers.values() {
            manager.list_resource_stacks(directory, &|id| id.ends_with(extension), &mut result);
        }
        result
    }
}

fn pack_filter_section(pack: &PackResources) -> Option<ResourceFilterSection> {
    let metadata = match pack.load_metadata() {
        Ok(metadata) => metadata,
        Err(_) => {
            error!("Failed to get filter section from pack {}", pack.pack_id());
            return None;
        }
    };
    match ResourceFilterSection::parse(metadata.get("filter")?) {
        Ok(filter) => Some(filter),
        Err(_) => {
            error!("Failed to get filter section from pack {}", pack.pack_id());
            None
        }
    }
}
