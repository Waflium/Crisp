use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock};

use super::metadata::{self, FormatRange, PackFormat};
use super::{PackLocationInfo, PackResources, PackSource, SharedZip};
use crate::network::chat::Component;
use crate::server::dedicated::settings::java_io_exception;
use crate::util::basic_file_attributes::BasicFileAttributes;
use crate::world::flag::FeatureFlagSet;
use crate::world::level::validation::directory_validator::DirectoryValidator;
use crate::world::level::validation::{ForbiddenSymlinkInfo, content_validation_message};

pub const VANILLA_ID: &str = "vanilla";
const FEATURE_PACKS: [&str; 3] = ["minecart_improvements", "redstone_experiments", "trade_rebalance"];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Position {
    Top,
    Bottom,
}

impl Position {
    pub fn insert<T>(self, list: &mut Vec<T>, value: T, converter: impl Fn(&T) -> PackSelectionConfig, reverse: bool) -> usize {
        let position = if reverse { self.opposite() } else { self };
        let index = match position {
            Position::Bottom => list.iter().position(|v| !is_fixed_at(converter(v), self)).unwrap_or(list.len()),
            Position::Top => list.iter().rposition(|v| !is_fixed_at(converter(v), self)).map_or(0, |i| i + 1),
        };
        list.insert(index, value);
        index
    }

    pub fn opposite(self) -> Position {
        match self {
            Position::Top => Position::Bottom,
            Position::Bottom => Position::Top,
        }
    }
}

fn is_fixed_at(config: PackSelectionConfig, position: Position) -> bool {
    config.fixed_position && config.default_position == position
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PackSelectionConfig {
    pub required: bool,
    pub default_position: Position,
    pub fixed_position: bool,
}

const DISCOVERED_PACK_SELECTION_CONFIG: PackSelectionConfig = PackSelectionConfig { required: false, default_position: Position::Top, fixed_position: false };
const VANILLA_SELECTION_CONFIG: PackSelectionConfig = PackSelectionConfig { required: false, default_position: Position::Bottom, fixed_position: false };
const FEATURE_SELECTION_CONFIG: PackSelectionConfig = PackSelectionConfig { required: false, default_position: Position::Top, fixed_position: false };

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PackCompatibility {
    TooOld,
    TooNew,
    Unknown,
    Compatible,
}

impl PackCompatibility {
    pub fn is_compatible(self) -> bool {
        self == PackCompatibility::Compatible
    }

    pub fn for_version(declared: FormatRange, game: PackFormat) -> PackCompatibility {
        if declared.min.major == i32::MAX {
            PackCompatibility::Unknown
        } else if declared.max < game {
            PackCompatibility::TooOld
        } else if game < declared.min {
            PackCompatibility::TooNew
        } else {
            PackCompatibility::Compatible
        }
    }
}

#[derive(Clone, Debug)]
pub struct PackMetadata {
    pub description: Component,
    pub compatibility: PackCompatibility,
    pub requested_features: FeatureFlagSet,
    pub overlays: Vec<String>,
}

#[derive(Clone, Debug)]
pub enum ResourcesSupplier {
    BuiltIn,
    Path(PathBuf),
    File(PathBuf),
}

static BUILT_IN_DATA: LazyLock<Arc<SharedZip>> = LazyLock::new(|| Arc::new(SharedZip::embedded(VANILLA_ID, crisp_data::VANILLA_DATA)));

fn built_in_prefix(id: &str) -> String {
    if id == VANILLA_ID { String::new() } else { format!("data/minecraft/datapacks/{id}") }
}

impl ResourcesSupplier {
    fn open_primary(&self, location: &PackLocationInfo, zip: &Option<Arc<SharedZip>>) -> PackResources {
        let id = location.id.clone();
        match self {
            ResourcesSupplier::BuiltIn => PackResources::File { prefix: built_in_prefix(&id), id, zip: BUILT_IN_DATA.clone() },
            ResourcesSupplier::Path(root) => PackResources::Path { id, root: root.clone() },
            ResourcesSupplier::File(_) => PackResources::File { id, zip: zip.clone().unwrap(), prefix: String::new() },
        }
    }

    fn shared_zip(&self) -> Option<Arc<SharedZip>> {
        match self {
            ResourcesSupplier::File(file) => Some(Arc::new(SharedZip::new(file.clone()))),
            _ => None,
        }
    }

    pub fn open_metadata(&self, location: &PackLocationInfo) -> PackResources {
        self.open_primary(location, &self.shared_zip())
    }

    pub fn open_resources(&self, location: &PackLocationInfo, metadata: &PackMetadata) -> PackResources {
        let zip = self.shared_zip();
        let primary = self.open_primary(location, &zip);
        if metadata.overlays.is_empty() {
            return primary;
        }
        let overlays = metadata.overlays.iter().map(|overlay| match self {
            ResourcesSupplier::Path(root) => PackResources::Path { id: location.id.clone(), root: root.join(overlay) },
            ResourcesSupplier::File(_) => PackResources::File { id: location.id.clone(), zip: zip.clone().unwrap(), prefix: overlay.clone() },
            ResourcesSupplier::BuiltIn => PackResources::File { id: location.id.clone(), zip: BUILT_IN_DATA.clone(), prefix: built_in_prefix(&location.id) },
        });
        let mut stack: Vec<PackResources> = overlays.collect();
        stack.reverse();
        stack.push(self.open_primary(location, &zip));
        PackResources::Overlayed { primary: Box::new(primary), stack }
    }
}

#[derive(Debug)]
pub struct Pack {
    pub location: PackLocationInfo,
    pub resources: ResourcesSupplier,
    pub metadata: PackMetadata,
    pub selection_config: PackSelectionConfig,
}

impl Pack {
    pub fn read_meta_and_create(location: PackLocationInfo, resources: ResourcesSupplier, selection_config: PackSelectionConfig) -> Option<Pack> {
        let metadata = read_pack_metadata(&location, &resources)?;
        Some(Pack { location, resources, metadata, selection_config })
    }

    pub fn id(&self) -> &str {
        &self.location.id
    }

    pub fn title(&self) -> &Component {
        &self.location.title
    }

    pub fn description(&self) -> &Component {
        &self.metadata.description
    }

    pub fn chat_link(&self, enabled: bool) -> Component {
        self.location.create_chat_link(enabled, &self.metadata.description)
    }

    pub fn compatibility(&self) -> PackCompatibility {
        self.metadata.compatibility
    }

    pub fn requested_features(&self) -> FeatureFlagSet {
        self.metadata.requested_features
    }

    pub fn open(&self) -> PackResources {
        self.resources.open_resources(&self.location, &self.metadata)
    }

    pub fn is_required(&self) -> bool {
        self.selection_config.required
    }

    pub fn is_fixed_position(&self) -> bool {
        self.selection_config.fixed_position
    }

    pub fn default_position(&self) -> Position {
        self.selection_config.default_position
    }

    pub fn pack_source(&self) -> PackSource {
        self.location.source
    }
}

fn read_pack_metadata(location: &PackLocationInfo, resources: &ResourcesSupplier) -> Option<PackMetadata> {
    if let ResourcesSupplier::BuiltIn = resources {
        return Some(built_in_metadata(&location.id));
    }
    match read_file_pack_metadata(location, resources) {
        Ok(metadata) => metadata,
        Err(exception) => {
            warn!("Failed to read pack {} metadata\n{exception}", location.id);
            None
        }
    }
}

fn built_in_metadata(id: &str) -> PackMetadata {
    let requested_features =
        if id == VANILLA_ID { FeatureFlagSet::DEFAULT_FLAGS } else { FeatureFlagSet::of(&format!("minecraft:{id}")).unwrap_or_default() };
    PackMetadata {
        description: Component::translatable(&format!("dataPack.{id}.description")),
        compatibility: PackCompatibility::for_version(PackFormat::CURRENT.minor_range(), PackFormat::CURRENT),
        requested_features,
        overlays: Vec::new(),
    }
}

fn json_parse_exception(message: String) -> String {
    format!("com.google.gson.JsonParseException: {message}")
}

fn read_file_pack_metadata(location: &PackLocationInfo, resources: &ResourcesSupplier) -> Result<Option<PackMetadata>, String> {
    let pack = resources.open_metadata(location);
    let section = |name: &str| -> Result<Option<serde_json::Value>, String> {
        let metadata = pack.load_metadata().map_err(|e| format!("com.google.gson.JsonSyntaxException: {e}"))?;
        Ok(metadata.get(name).cloned())
    };
    let meta = match section("pack").and_then(|raw| raw.map(|raw| metadata::pack_section(&raw).map_err(json_parse_exception)).transpose()) {
        Ok(meta) => meta,
        Err(exception) => {
            warn!("Error reading pack metadata, attempting fallback type\n{exception}");
            section("pack")?.map(|raw| metadata::fallback_pack_section(&raw).map_err(json_parse_exception)).transpose()?
        }
    };
    let Some(meta) = meta else {
        warn!("Missing metadata in pack {}", location.id);
        return Ok(None);
    };
    let requested_features = section("features")?.map(|raw| metadata::features_section(&raw).map_err(json_parse_exception)).transpose()?.unwrap_or_default();
    let compatibility = PackCompatibility::for_version(meta.supported_formats, PackFormat::CURRENT);
    let overlays = section("overlays")?.map(|raw| metadata::overlays_section(&raw).map_err(json_parse_exception)).transpose()?.unwrap_or_default();
    Ok(Some(PackMetadata { description: meta.description, compatibility, requested_features, overlays }))
}

#[derive(Clone, Debug)]
pub enum RepositorySource {
    ServerPacks,
    Folder { folder: PathBuf, pack_source: PackSource, validator: Arc<DirectoryValidator> },
}

impl RepositorySource {
    fn load_packs(&self, result: &mut dyn FnMut(Pack)) {
        match self {
            RepositorySource::ServerPacks => {
                let vanilla = PackLocationInfo { id: VANILLA_ID.to_string(), title: Component::translatable("dataPack.vanilla.name"), source: PackSource::BuiltIn };
                if let Some(pack) = Pack::read_meta_and_create(vanilla, ResourcesSupplier::BuiltIn, VANILLA_SELECTION_CONFIG) {
                    result(pack);
                }
                for id in FEATURE_PACKS {
                    let location = PackLocationInfo { id: id.to_string(), title: Component::literal(id), source: PackSource::Feature };
                    if let Some(pack) = Pack::read_meta_and_create(location, ResourcesSupplier::BuiltIn, FEATURE_SELECTION_CONFIG) {
                        result(pack);
                    }
                }
            }
            RepositorySource::Folder { folder, pack_source, validator } => {
                let discovered = std::fs::create_dir_all(folder).and_then(|_| {
                    discover_packs(folder, validator, &mut |content, resources| {
                        let name = content.file_name().unwrap_or_default().to_string_lossy().into_owned();
                        let location = PackLocationInfo { id: format!("file/{name}"), title: Component::literal(&name), source: *pack_source };
                        if let Some(pack) = Pack::read_meta_and_create(location, resources, DISCOVERED_PACK_SELECTION_CONFIG) {
                            result(pack);
                        }
                    })
                });
                if let Err(e) = discovered {
                    warn!("Failed to list packs in {}\njava.io.IOException: {e}", folder.display());
                }
            }
        }
    }
}

pub fn discover_packs(folder: &Path, validator: &DirectoryValidator, result: &mut dyn FnMut(&Path, ResourcesSupplier)) -> io::Result<()> {
    for entry in std::fs::read_dir(folder)? {
        let content = entry?.path();
        let mut issues = Vec::new();
        match detect_pack_resources(&content, validator, &mut issues) {
            Ok(_) if !issues.is_empty() => warn!("Ignoring potential pack entry: {}", content_validation_message(&content, &issues)),
            Ok(Some(resources)) => result(&content, resources),
            Ok(None) => info!("Found non-pack entry '{}', ignoring", content.display()),
            Err(e) => warn!("Failed to read properties of '{}', ignoring\n{e}", content.display()),
        }
    }
    Ok(())
}

fn detect_pack_resources(content: &Path, validator: &DirectoryValidator, issues: &mut Vec<ForbiddenSymlinkInfo>) -> Result<Option<ResourcesSupplier>, String> {
    let mut attributes = match BasicFileAttributes::read(content) {
        Ok(attributes) => attributes,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(java_io_exception(&e, content)),
    };
    let mut target_context = content.to_path_buf();
    if attributes.symbolic_link {
        validator.validate_symlink(content, issues)?;
        if !issues.is_empty() {
            return Ok(None);
        }
        target_context = std::fs::read_link(content).map_err(|e| java_io_exception(&e, content))?;
        attributes = BasicFileAttributes::read(&target_context).map_err(|e| java_io_exception(&e, &target_context))?;
    }
    if attributes.directory {
        validator.validate_known_directory(&target_context, issues)?;
        if !issues.is_empty() {
            return Ok(None);
        }
        return Ok(target_context.join("pack.mcmeta").is_file().then_some(ResourcesSupplier::Path(target_context)));
    }
    let is_zip = target_context.file_name().is_some_and(|name| name.to_string_lossy().ends_with(".zip"));
    Ok((attributes.regular_file && is_zip).then_some(ResourcesSupplier::File(target_context)))
}

#[derive(Debug)]
pub struct PackRepository {
    sources: Vec<RepositorySource>,
    available: BTreeMap<String, Arc<Pack>>,
    selected: Vec<Arc<Pack>>,
}

impl PackRepository {
    pub fn new(sources: Vec<RepositorySource>) -> PackRepository {
        PackRepository { sources, available: BTreeMap::new(), selected: Vec::new() }
    }

    pub fn create_pack_repository(datapack_dir: PathBuf, validator: Arc<DirectoryValidator>) -> PackRepository {
        PackRepository::new(vec![RepositorySource::ServerPacks, RepositorySource::Folder { folder: datapack_dir, pack_source: PackSource::World, validator }])
    }

    pub fn create_vanilla_trusted_repository() -> PackRepository {
        PackRepository::new(vec![RepositorySource::ServerPacks])
    }

    pub fn display_pack_list<'a>(packs: impl IntoIterator<Item = &'a Arc<Pack>>) -> String {
        packs
            .into_iter()
            .map(|pack| format!("{}{}", pack.id(), if pack.compatibility().is_compatible() { "" } else { " (incompatible)" }))
            .collect::<Vec<_>>()
            .join(", ")
    }

    pub fn reload(&mut self) {
        let currently_selected: Vec<String> = self.selected.iter().map(|pack| pack.id().to_string()).collect();
        self.available = self.discover_available();
        self.selected = self.rebuild_selected(&currently_selected);
    }

    fn discover_available(&self) -> BTreeMap<String, Arc<Pack>> {
        let mut discovered = BTreeMap::new();
        for source in &self.sources {
            source.load_packs(&mut |pack| {
                discovered.insert(pack.id().to_string(), Arc::new(pack));
            });
        }
        discovered
    }

    pub fn is_able_to_clear_any_pack(&self) -> bool {
        let new_selected = self.rebuild_selected::<&str>(&[]);
        !same_packs(&self.selected, &new_selected)
    }

    pub fn set_selected<S: AsRef<str>>(&mut self, packs: &[S]) {
        self.selected = self.rebuild_selected(packs);
    }

    pub fn add_pack(&mut self, pack_id: &str) -> bool {
        match self.available.get(pack_id) {
            Some(pack) if !self.selected.iter().any(|p| p.id() == pack_id) => {
                self.selected.push(pack.clone());
                true
            }
            _ => false,
        }
    }

    pub fn remove_pack(&mut self, pack_id: &str) -> bool {
        if !self.available.contains_key(pack_id) {
            return false;
        }
        match self.selected.iter().position(|p| p.id() == pack_id) {
            Some(index) => {
                self.selected.remove(index);
                true
            }
            None => false,
        }
    }

    fn rebuild_selected<S: AsRef<str>>(&self, selected_names: &[S]) -> Vec<Arc<Pack>> {
        let mut selected_and_present: Vec<Arc<Pack>> = selected_names.iter().filter_map(|id| self.available.get(id.as_ref()).cloned()).collect();
        for pack in self.available.values() {
            if pack.is_required() && !selected_and_present.iter().any(|p| p.id() == pack.id()) {
                pack.default_position().insert(&mut selected_and_present, pack.clone(), |p| p.selection_config, false);
            }
        }
        selected_and_present
    }

    pub fn available_ids(&self) -> impl Iterator<Item = &str> {
        self.available.keys().map(String::as_str)
    }

    pub fn available_packs(&self) -> impl Iterator<Item = &Arc<Pack>> {
        self.available.values()
    }

    pub fn selected_ids(&self) -> Vec<&str> {
        let mut ids: Vec<&str> = Vec::with_capacity(self.selected.len());
        for pack in &self.selected {
            if !ids.contains(&pack.id()) {
                ids.push(pack.id());
            }
        }
        ids
    }

    pub fn requested_feature_flags(&self) -> FeatureFlagSet {
        self.selected.iter().fold(FeatureFlagSet::default(), |flags, pack| flags.join(pack.requested_features()))
    }

    pub fn selected_packs(&self) -> &[Arc<Pack>] {
        &self.selected
    }

    pub fn pack(&self, id: &str) -> Option<&Arc<Pack>> {
        self.available.get(id)
    }

    pub fn is_available(&self, id: &str) -> bool {
        self.available.contains_key(id)
    }

    pub fn open_all_selected(&self) -> Vec<PackResources> {
        self.selected.iter().map(|pack| pack.open()).collect()
    }
}

fn same_packs(a: &[Arc<Pack>], b: &[Arc<Pack>]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.id() == y.id())
}
