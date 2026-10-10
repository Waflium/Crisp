pub mod tag_entry;

use std::fmt;
use std::sync::Arc;

use serde_json::Value;

use self::tag_entry::{Lookup, TagEntry};
use crate::server::packs::resources::MultiPackResourceManager;
use crate::util::dependency_sorter::{DependencySorter, Entry};
use crate::util::java_collections::{JavaHashMap, identifier_hash};

pub struct EntryWithSource {
    pub entry: TagEntry,
    pub source: String,
}

impl fmt::Display for EntryWithSource {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} (from {})", self.entry, self.source)
    }
}

struct SortingEntry<'a>(&'a [EntryWithSource]);

impl Entry for SortingEntry<'_> {
    fn visit_required_dependencies(&self, output: &mut dyn FnMut(&str)) {
        self.0.iter().for_each(|e| e.entry.visit_required_dependencies(output));
    }

    fn visit_optional_dependencies(&self, output: &mut dyn FnMut(&str)) {
        self.0.iter().for_each(|e| e.entry.visit_optional_dependencies(output));
    }
}

pub struct TagLoader {
    directory: String,
}

struct TagFile {
    entries: Vec<TagEntry>,
    replace: bool,
}

impl TagFile {
    fn parse(value: &Value) -> Result<TagFile, String> {
        let object = value.as_object().ok_or_else(|| format!("Not a map: {value}"))?;
        let values = object.get("values").ok_or_else(|| format!("No key values in MapLike[{value}]"))?;
        let values = values.as_array().ok_or_else(|| format!("Not a list: {values}"))?;
        let entries = values.iter().map(TagEntry::parse).collect::<Result<Vec<_>, _>>()?;
        let replace = match object.get("replace") {
            None => false,
            Some(Value::Bool(replace)) => *replace,
            Some(other) => return Err(format!("Not a boolean: {other}")),
        };
        Ok(TagFile { entries, replace })
    }
}

pub fn identifier_order<T>(entries: &mut [(String, T)]) {
    entries.sort_by(|(a, _), (b, _)| {
        let split = |id: &str| -> (String, String) {
            let (namespace, path) = id.split_once(':').unwrap_or(("minecraft", id));
            (path.to_string(), namespace.to_string())
        };
        let (a, b) = (split(a), split(b));
        a.0.encode_utf16().cmp(b.0.encode_utf16()).then_with(|| a.1.encode_utf16().cmp(b.1.encode_utf16()))
    });
}

impl TagLoader {
    pub fn new(directory: &str) -> TagLoader {
        TagLoader { directory: directory.to_string() }
    }

    fn file_to_id(&self, location: &str) -> String {
        let (namespace, path) = location.split_once(':').unwrap();
        let path = &path[self.directory.len() + 1..path.len() - ".json".len()];
        format!("{namespace}:{path}")
    }

    pub fn load(&self, resource_manager: &MultiPackResourceManager) -> JavaHashMap<Vec<EntryWithSource>> {
        let mut builders: JavaHashMap<Vec<EntryWithSource>> = JavaHashMap::new(identifier_hash);
        let mut stacks: Vec<_> = resource_manager.list_resource_stacks(&self.directory, ".json").into_iter().collect();
        identifier_order(&mut stacks);
        for (location, resources) in stacks {
            let id = self.file_to_id(&location);
            for resource in resources {
                let parsed = resource
                    .supplier
                    .read()
                    .map_err(|e| format!("java.io.IOException: {e}"))
                    .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).map_err(|e| format!("com.google.gson.JsonSyntaxException: {e}")))
                    .and_then(|element| {
                        if builders.get(&id).is_none() {
                            builders.insert(id.clone(), Vec::new());
                        }
                        TagFile::parse(&element).map_err(|e| format!("java.lang.IllegalStateException: {e}"))
                    });
                match parsed {
                    Ok(parsed) => {
                        let contents = builders.get_mut(&id).unwrap();
                        if parsed.replace {
                            contents.clear();
                        }
                        contents.extend(parsed.entries.into_iter().map(|entry| EntryWithSource { entry, source: resource.source_pack_id.clone() }));
                    }
                    Err(exception) => {
                        error!("Couldn't read tag list {id} from {location} in data pack {}\n{exception}", resource.source_pack_id);
                    }
                }
            }
        }
        builders
    }

    pub fn build<T>(
        &self,
        builders: &JavaHashMap<Vec<EntryWithSource>>,
        element_lookup: &dyn Fn(&str, bool) -> Option<Arc<T>>,
    ) -> JavaHashMap<Vec<Arc<T>>> {
        let mut new_tags: JavaHashMap<Vec<Arc<T>>> = JavaHashMap::new(identifier_hash);
        let mut sorter = DependencySorter::new();
        for (id, entries) in builders.iter() {
            sorter.add_entry(id.to_string(), SortingEntry(entries));
        }
        sorter.order_by_dependencies(&mut |id, contents| {
            let lookup = TagLookup { element_lookup, tags: &new_tags };
            let mut values: Vec<Arc<T>> = Vec::new();
            let mut missing_elements: Vec<&EntryWithSource> = Vec::new();
            for entry in contents.0 {
                let present = entry.entry.build(&lookup, &mut |value: Arc<T>| {
                    if !values.iter().any(|v| Arc::ptr_eq(v, &value)) {
                        values.push(value);
                    }
                });
                if !present {
                    missing_elements.push(entry);
                }
            }
            if missing_elements.is_empty() {
                new_tags.insert(id.to_string(), values);
            } else {
                let missing: Vec<String> = missing_elements.iter().map(|e| e.to_string()).collect();
                error!("Couldn't load tag {id} as it is missing following references: {}", missing.join(", "));
            }
        });
        new_tags
    }
}

struct TagLookup<'a, T> {
    element_lookup: &'a dyn Fn(&str, bool) -> Option<Arc<T>>,
    tags: &'a JavaHashMap<Vec<Arc<T>>>,
}

impl<T> Lookup<Arc<T>> for TagLookup<'_, T> {
    fn element(&self, key: &str, required: bool) -> Option<Arc<T>> {
        (self.element_lookup)(key, required)
    }

    fn tag(&self, key: &str) -> Option<Vec<Arc<T>>> {
        self.tags.get(key).cloned()
    }
}
