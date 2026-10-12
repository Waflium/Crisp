
pub mod metadata;
pub mod repository;
pub mod resources;

use std::collections::HashSet;
use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use serde_json::Value;

use crate::network::chat::{Arg, ChatFormatting, Component, HoverEvent};

pub const DATA_DIRECTORY: &str = "data";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PackSource {
    BuiltIn,
    Feature,
    World,
}

impl PackSource {
    pub fn decorate(self, description: Component) -> Component {
        let source = match self {
            PackSource::BuiltIn => "pack.source.builtin",
            PackSource::Feature => "pack.source.feature",
            PackSource::World => "pack.source.world",
        };
        Component::translatable_with("pack.nameAndSource", vec![Arg::Component(description), Arg::Component(Component::translatable(source))])
            .with_color(ChatFormatting::Gray)
    }

    pub fn should_add_automatically(self) -> bool {
        self != PackSource::Feature
    }
}

#[derive(Clone, Debug)]
pub struct PackLocationInfo {
    pub id: String,
    pub title: Component,
    pub source: PackSource,
}

impl PackLocationInfo {
    pub fn create_chat_link(&self, enabled: bool, description: &Component) -> Component {
        let hover = Component::empty().append(self.title.clone()).append(Component::literal("\n")).append(description.clone());
        Component::translatable_with("chat.square_brackets", vec![Arg::Component(self.source.decorate(Component::literal(self.id.as_str())))])
            .with_color(if enabled { ChatFormatting::Green } else { ChatFormatting::Red })
            .with_insertion(&escape_if_required(&self.id))
            .with_hover(HoverEvent::ShowText(Box::new(hover)))
    }
}

pub fn escape_if_required(input: &str) -> String {
    if input.encode_utf16().all(crate::commands::brigadier::StringReader::is_allowed_in_unquoted_string) {
        return input.to_string();
    }
    let mut result = String::from("\"");
    for c in input.chars() {
        if c == '\\' || c == '"' {
            result.push('\\');
        }
        result.push(c);
    }
    result.push('"');
    result
}

enum ZipSource {
    File(File),
    Embedded(io::Cursor<&'static [u8]>),
}

impl Read for ZipSource {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            ZipSource::File(file) => file.read(buf),
            ZipSource::Embedded(cursor) => cursor.read(buf),
        }
    }
}

impl io::Seek for ZipSource {
    fn seek(&mut self, pos: io::SeekFrom) -> io::Result<u64> {
        match self {
            ZipSource::File(file) => file.seek(pos),
            ZipSource::Embedded(cursor) => cursor.seek(pos),
        }
    }
}

pub struct SharedZip {
    file: PathBuf,
    embedded: Option<&'static [u8]>,
    archive: Mutex<Option<zip::ZipArchive<ZipSource>>>,
    names: OnceLock<Arc<[String]>>,
    failed_to_load: AtomicBool,
}

impl SharedZip {
    pub fn new(file: PathBuf) -> SharedZip {
        SharedZip { file, embedded: None, archive: Mutex::new(None), names: OnceLock::new(), failed_to_load: AtomicBool::new(false) }
    }

    pub fn embedded(name: &str, bytes: &'static [u8]) -> SharedZip {
        SharedZip { file: PathBuf::from(name), embedded: Some(bytes), archive: Mutex::new(None), names: OnceLock::new(), failed_to_load: AtomicBool::new(false) }
    }

    fn open(&self) -> zip::result::ZipResult<zip::ZipArchive<ZipSource>> {
        let source = match self.embedded {
            Some(bytes) => ZipSource::Embedded(io::Cursor::new(bytes)),
            None => ZipSource::File(File::open(&self.file)?),
        };
        zip::ZipArchive::new(source)
    }

    fn with<R>(&self, f: impl FnOnce(&mut zip::ZipArchive<ZipSource>) -> R) -> Option<R> {
        let mut archive = self.archive.lock().unwrap();
        if self.failed_to_load.load(Ordering::Relaxed) {
            return None;
        }
        if archive.is_none() {
            match self.open() {
                Ok(opened) => *archive = Some(opened),
                Err(e) => {
                    let exception = match e {
                        zip::result::ZipError::Io(e) => format!("java.io.IOException: {e}"),
                        _ => "java.util.zip.ZipException: zip END header not found".to_string(),
                    };
                    error!("Failed to open pack {}\n{exception}", self.file.display());
                    self.failed_to_load.store(true, Ordering::Relaxed);
                    return None;
                }
            }
        }
        Some(f(archive.as_mut().unwrap()))
    }

    fn names(&self) -> Arc<[String]> {
        if let Some(names) = self.names.get() {
            return names.clone();
        }
        let mut names: Vec<String> = self.with(|archive| archive.file_names().map(str::to_string).collect()).unwrap_or_default();
        names.sort_unstable();
        self.names.get_or_init(|| names.into()).clone()
    }

    fn names_with_prefix(&self, prefix: &str) -> Vec<String> {
        let names = self.names();
        let start = names.partition_point(|name| name.as_str() < prefix);
        names[start..].iter().take_while(|name| name.starts_with(prefix)).cloned().collect()
    }

    fn read(&self, name: &str) -> io::Result<Vec<u8>> {
        self.with(|archive| {
            let mut entry = archive.by_name(name).map_err(io::Error::other)?;
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes)?;
            Ok(bytes)
        })
        .unwrap_or_else(|| Err(io::Error::other("the pack could not be opened")))
    }

    fn contains(&self, name: &str) -> bool {
        self.with(|archive| archive.index_for_name(name).is_some()).unwrap_or(false)
    }
}

#[derive(Clone)]
pub enum IoSupplier {
    File(PathBuf),
    Zip(Arc<SharedZip>, String),
}

impl IoSupplier {
    pub fn read(&self) -> io::Result<Vec<u8>> {
        match self {
            IoSupplier::File(path) => std::fs::read(path),
            IoSupplier::Zip(zip, name) => zip.read(name),
        }
    }
}

#[derive(Clone)]
pub struct Resource {
    pub source_pack_id: String,
    pub supplier: IoSupplier,
}

pub enum PackResources {
    Path { id: String, root: PathBuf },
    File { id: String, zip: Arc<SharedZip>, prefix: String },
    Overlayed { primary: Box<PackResources>, stack: Vec<PackResources> },
}

impl PackResources {
    pub fn pack_id(&self) -> &str {
        match self {
            PackResources::Path { id, .. } | PackResources::File { id, .. } => id,
            PackResources::Overlayed { primary, .. } => primary.pack_id(),
        }
    }

    fn root_resource(&self, name: &str) -> Option<IoSupplier> {
        match self {
            PackResources::Path { root, .. } => {
                let path = root.join(name);
                path.exists().then_some(IoSupplier::File(path))
            }
            PackResources::File { zip, prefix, .. } => {
                let name = add_prefix(prefix, name);
                zip.contains(&name).then(|| IoSupplier::Zip(zip.clone(), name))
            }
            PackResources::Overlayed { primary, .. } => primary.root_resource(name),
        }
    }

    pub fn load_metadata(&self) -> Result<serde_json::Map<String, Value>, String> {
        let Some(supplier) = self.root_resource("pack.mcmeta") else { return Ok(serde_json::Map::new()) };
        let bytes = supplier.read().map_err(|e| e.to_string())?;
        match serde_json::from_slice::<Value>(&bytes).map_err(|e| e.to_string())? {
            Value::Object(object) => Ok(object),
            other => Err(format!("Expected JsonObject, got {other}")),
        }
    }

    pub fn get_namespaces(&self) -> HashSet<String> {
        let mut namespaces = HashSet::new();
        match self {
            PackResources::Path { root, .. } => {
                let dir = root.join(DATA_DIRECTORY);
                let Ok(entries) = std::fs::read_dir(&dir) else { return namespaces };
                for entry in entries.flatten() {
                    let path = entry.path();
                    if !path.is_dir() {
                        warn!("Non-directory entry {} found in namespace directory, rejecting", path.display());
                        continue;
                    }
                    let namespace = entry.file_name().to_string_lossy().into_owned();
                    if is_valid_namespace(&namespace) {
                        namespaces.insert(namespace);
                    } else {
                        warn!("Non [a-z0-9_.-] character in namespace {namespace} in pack directory {}, ignoring", dir.display());
                    }
                }
            }
            PackResources::File { zip, prefix, .. } => {
                let type_prefix = add_prefix(prefix, &format!("{DATA_DIRECTORY}/"));
                for name in zip.names_with_prefix(&type_prefix) {
                    let Some(rest) = name.strip_prefix(&type_prefix) else { continue };
                    let namespace = rest.split('/').next().unwrap_or_default();
                    if namespace.is_empty() {
                        continue;
                    }
                    if is_valid_namespace(namespace) {
                        namespaces.insert(namespace.to_string());
                    } else {
                        warn!("Non [a-z0-9_.-] character in namespace {namespace} in pack {}, ignoring", zip.file.display());
                    }
                }
            }
            PackResources::Overlayed { stack, .. } => stack.iter().for_each(|pack| namespaces.extend(pack.get_namespaces())),
        }
        namespaces
    }

    pub fn list_resources(&self, namespace: &str, directory: &str, output: &mut dyn FnMut(String, IoSupplier)) {
        match self {
            PackResources::Path { root, .. } => {
                let top = root.join(DATA_DIRECTORY).join(namespace);
                let mut files = Vec::new();
                list_files(&top.join(directory), &mut files);
                for file in files {
                    let relative: Vec<String> = file.strip_prefix(&top).unwrap().iter().map(|p| p.to_string_lossy().into_owned()).collect();
                    let path = relative.join("/");
                    match try_build(namespace, &path) {
                        Some(id) => output(id, IoSupplier::File(file)),
                        None => error!("Invalid path in pack: {namespace}:{path}, ignoring"),
                    }
                }
            }
            PackResources::File { zip, prefix, .. } => {
                let root = add_prefix(prefix, &format!("{DATA_DIRECTORY}/{namespace}/"));
                let prefix = format!("{root}{directory}/");
                for name in zip.names_with_prefix(&prefix) {
                    if name.ends_with('/') || !name.starts_with(&prefix) {
                        continue;
                    }
                    let path = &name[root.len()..];
                    match try_build(namespace, path) {
                        Some(id) => output(id, IoSupplier::Zip(zip.clone(), name.clone())),
                        None => warn!("Invalid path in datapack: {namespace}:{path}, ignoring"),
                    }
                }
            }
            PackResources::Overlayed { stack, .. } => {
                let mut result: Vec<(String, IoSupplier)> = Vec::new();
                let mut seen = HashSet::new();
                for pack in stack {
                    pack.list_resources(namespace, directory, &mut |id, supplier| {
                        if seen.insert(id.clone()) {
                            result.push((id, supplier));
                        }
                    });
                }
                result.into_iter().for_each(|(id, supplier)| output(id, supplier));
            }
        }
    }
}

fn add_prefix(prefix: &str, path: &str) -> String {
    if prefix.is_empty() { path.to_string() } else { format!("{prefix}/{path}") }
}

fn list_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else { continue };
        if file_type.is_dir() {
            list_files(&entry.path(), out);
        } else if file_type.is_file() {
            out.push(entry.path());
        }
    }
}

pub fn is_valid_namespace(namespace: &str) -> bool {
    namespace != ".." && namespace.bytes().all(|c| matches!(c, b'_' | b'-' | b'a'..=b'z' | b'0'..=b'9' | b'.'))
}

fn try_build(namespace: &str, path: &str) -> Option<String> {
    let valid_path = path.bytes().all(|c| matches!(c, b'_' | b'-' | b'a'..=b'z' | b'0'..=b'9' | b'/' | b'.'));
    (is_valid_namespace(namespace) && valid_path).then(|| format!("{namespace}:{path}"))
}
