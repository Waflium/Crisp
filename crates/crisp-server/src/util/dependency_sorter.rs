use std::collections::{HashMap, HashSet};

use crisp_core::hash_map::java_hash_map_order_with_capacity;

use crate::util::java_collections::{JavaHashMap, identifier_hash};

pub trait Entry {
    fn visit_required_dependencies(&self, output: &mut dyn FnMut(&str));

    fn visit_optional_dependencies(&self, output: &mut dyn FnMut(&str));
}

pub struct DependencySorter<V> {
    contents: JavaHashMap<V>,
}

#[derive(Default)]
struct Multimap {
    values: HashMap<String, Vec<String>>,
}

impl Multimap {
    fn get(&self, key: &str) -> &[String] {
        self.values.get(key).map_or(&[], Vec::as_slice)
    }

    fn put(&mut self, key: &str, value: &str) {
        let values = self.values.entry(key.to_string()).or_default();
        if !values.iter().any(|v| v == value) {
            values.push(value.to_string());
        }
    }

    fn ordered(&self, key: &str) -> Vec<&str> {
        let values = self.get(key);
        let hashes: Vec<i32> = values.iter().map(|v| identifier_hash(v)).collect();
        java_hash_map_order_with_capacity(&hashes, 4).into_iter().map(|i| values[i].as_str()).collect()
    }
}

impl<V: Entry> DependencySorter<V> {
    pub fn new() -> DependencySorter<V> {
        DependencySorter { contents: JavaHashMap::new(identifier_hash) }
    }

    pub fn add_entry(&mut self, id: String, value: V) {
        self.contents.insert(id, value);
    }

    fn visit_dependencies_and_element(&self, dependencies: &Multimap, already_visited: &mut HashSet<String>, id: &str, output: &mut dyn FnMut(&str, &V)) {
        if already_visited.insert(id.to_string()) {
            for dependency in dependencies.ordered(id) {
                self.visit_dependencies_and_element(dependencies, already_visited, dependency, output);
            }
            if let Some(current) = self.contents.get(id) {
                output(id, current);
            }
        }
    }

    pub fn order_by_dependencies(&self, output: &mut dyn FnMut(&str, &V)) {
        let mut direct_dependencies = Multimap::default();
        for (id, value) in self.contents.iter() {
            value.visit_required_dependencies(&mut |dep| add_dependency_if_not_cyclic(&mut direct_dependencies, id, dep));
        }
        for (id, value) in self.contents.iter() {
            value.visit_optional_dependencies(&mut |dep| add_dependency_if_not_cyclic(&mut direct_dependencies, id, dep));
        }
        let mut already_visited = HashSet::new();
        for (id, _) in self.contents.iter() {
            self.visit_dependencies_and_element(&direct_dependencies, &mut already_visited, id, output);
        }
    }
}

fn is_cyclic(direct_dependencies: &Multimap, from: &str, to: &str) -> bool {
    let dependencies = direct_dependencies.get(to);
    dependencies.iter().any(|d| d == from) || dependencies.iter().any(|dep| is_cyclic(direct_dependencies, from, dep))
}

fn add_dependency_if_not_cyclic(direct_dependencies: &mut Multimap, from: &str, to: &str) {
    if !is_cyclic(direct_dependencies, from, to) {
        direct_dependencies.put(from, to);
    }
}
