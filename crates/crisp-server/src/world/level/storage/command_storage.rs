use std::collections::HashMap;
use std::io;
use std::path::PathBuf;

use crisp_data::WORLD_VERSION;
use crisp_protocol::nbt::Nbt;

use super::{read_dat, write_dat};

pub struct CommandStorage {
    dir: Option<PathBuf>,
    namespaces: HashMap<String, Option<Container>>,
}

#[derive(Default)]
struct Container {
    storage: Vec<(String, Vec<(String, Nbt)>)>,
    dirty: bool,
}

impl CommandStorage {
    pub fn new(dir: Option<PathBuf>) -> CommandStorage {
        CommandStorage { dir, namespaces: HashMap::new() }
    }

    fn file(&self, namespace: &str) -> Option<PathBuf> {
        Some(self.dir.as_ref()?.join(namespace).join("command_storage.dat"))
    }

    fn container(&mut self, namespace: &str) -> Option<&mut Container> {
        if !self.namespaces.contains_key(namespace) {
            let loaded = self.file(namespace).and_then(|file| match read_dat(&file) {
                Ok(tag) => tag.and_then(|t| Container::from_nbt(t.get("data")?)),
                Err(_) => {
                    error!("Error loading saved data: SavedDataType[{namespace}:command_storage]");
                    None
                }
            });
            self.namespaces.insert(namespace.to_string(), loaded);
        }
        self.namespaces.get_mut(namespace).unwrap().as_mut()
    }

    pub fn get(&mut self, id: &str) -> Vec<(String, Nbt)> {
        let (namespace, path) = id.split_once(':').unwrap();
        self.container(namespace).and_then(|c| c.storage.iter().find(|(p, _)| p == path)).map(|(_, tag)| tag.clone()).unwrap_or_default()
    }

    pub fn write_back(&mut self, id: &str, contents: Vec<(String, Nbt)>) {
        let (namespace, path) = id.split_once(':').unwrap();
        if let Some(entry) = self.container(namespace).and_then(|c| c.storage.iter_mut().find(|(p, _)| p == path)) {
            entry.1 = contents;
        }
    }

    pub fn set(&mut self, id: &str, contents: Vec<(String, Nbt)>) {
        let (namespace, path) = id.split_once(':').unwrap();
        if self.container(namespace).is_none() {
            self.namespaces.insert(namespace.to_string(), Some(Container::default()));
        }
        let container = self.container(namespace).unwrap();
        let existing = container.storage.iter().position(|(p, _)| p == path);
        match (existing, contents.is_empty()) {
            (Some(i), true) => drop(container.storage.remove(i)),
            (Some(i), false) => container.storage[i].1 = contents,
            (None, true) => {}
            (None, false) => container.storage.push((path.to_string(), contents)),
        }
        container.dirty = true;
    }

    pub fn keys(&self) -> Vec<String> {
        self.namespaces.iter().flat_map(|(ns, c)| c.iter().flat_map(move |c| c.storage.iter().map(move |(p, _)| format!("{ns}:{p}")))).collect()
    }

    pub fn save(&mut self) -> io::Result<()> {
        let Some(dir) = self.dir.clone() else { return Ok(()) };
        for (namespace, container) in &mut self.namespaces {
            let Some(container) = container.as_mut().filter(|c| c.dirty) else { continue };
            let file = Nbt::Compound(vec![("data".into(), container.to_nbt()), ("DataVersion".into(), Nbt::Int(WORLD_VERSION))]);
            write_dat(&dir.join(namespace.as_str()).join("command_storage.dat"), &file)?;
            container.dirty = false;
        }
        Ok(())
    }
}

impl Container {
    fn from_nbt(data: &Nbt) -> Option<Container> {
        let Some(Nbt::Compound(contents)) = data.get("contents") else { return None };
        let storage = contents.iter().filter_map(|(path, tag)| match tag {
            Nbt::Compound(fields) => Some((path.clone(), fields.clone())),
            _ => None,
        });
        Some(Container { storage: storage.collect(), dirty: false })
    }

    fn to_nbt(&self) -> Nbt {
        let contents = self.storage.iter().map(|(path, fields)| (path.clone(), Nbt::Compound(fields.clone()))).collect();
        Nbt::Compound(vec![("contents".into(), Nbt::Compound(contents))])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saves_per_namespace() {
        let dir = std::env::temp_dir().join(format!("crisp-command-storage-{}", std::process::id()));
        let mut storage = CommandStorage::new(Some(dir.clone()));
        storage.set("a:x", vec![("v".into(), Nbt::Int(1))]);
        storage.set("b:y/z", vec![("w".into(), Nbt::String("s".into()))]);
        storage.save().unwrap();
        assert!(dir.join("a/command_storage.dat").exists() && dir.join("b/command_storage.dat").exists());
        let mut loaded = CommandStorage::new(Some(dir.clone()));
        assert_eq!(loaded.get("a:x"), vec![("v".to_string(), Nbt::Int(1))]);
        assert_eq!(loaded.get("b:y/z"), vec![("w".to_string(), Nbt::String("s".into()))]);
        assert_eq!(loaded.get("c:none"), Vec::new());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
