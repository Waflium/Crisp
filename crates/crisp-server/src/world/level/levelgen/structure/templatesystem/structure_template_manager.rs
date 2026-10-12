use std::collections::{BTreeMap, HashMap};
use std::io::Read;
use std::sync::{Arc, LazyLock, RwLock};

use flate2::read::GzDecoder;

use super::structure_template::StructureTemplate;
use crate::server::packs::Resource;
use crate::server::packs::repository::{PackRepository, VANILLA_ID};
use crate::server::packs::resources::MultiPackResourceManager;
use crate::world::level::chunk::storage::decode_nbt;

const STRUCTURE_DIRECTORY: &str = "structure";
const STRUCTURE_FILE_EXTENSION: &str = ".nbt";

static RESOURCES: RwLock<Option<Arc<BTreeMap<String, Resource>>>> = RwLock::new(None);
static VANILLA_RESOURCES: LazyLock<Arc<BTreeMap<String, Resource>>> = LazyLock::new(|| {
    let mut repository = PackRepository::create_vanilla_trusted_repository();
    repository.reload();
    repository.set_selected(&[VANILLA_ID]);
    Arc::new(MultiPackResourceManager::new(repository.open_all_selected()).list_resources(STRUCTURE_DIRECTORY, STRUCTURE_FILE_EXTENSION))
});
static STRUCTURE_REPOSITORY: LazyLock<RwLock<HashMap<String, &'static StructureTemplate>>> = LazyLock::new(|| RwLock::new(HashMap::new()));

pub fn on_resource_manager_reload(resource_manager: &MultiPackResourceManager) {
    *RESOURCES.write().expect("structure resources") = Some(Arc::new(resource_manager.list_resources(STRUCTURE_DIRECTORY, STRUCTURE_FILE_EXTENSION)));
    STRUCTURE_REPOSITORY.write().expect("structure repository").clear();
}

fn resources() -> Arc<BTreeMap<String, Resource>> {
    RESOURCES.read().expect("structure resources").clone().unwrap_or_else(|| VANILLA_RESOURCES.clone())
}

pub fn list_templates() -> Vec<String> {
    resources()
        .keys()
        .map(|location| {
            let (namespace, path) = location.split_once(':').unwrap();
            format!("{namespace}:{}", &path[STRUCTURE_DIRECTORY.len() + 1..path.len() - STRUCTURE_FILE_EXTENSION.len()])
        })
        .collect()
}

pub fn get(id: &str) -> Option<&'static StructureTemplate> {
    if let Some(&template) = STRUCTURE_REPOSITORY.read().expect("structure repository").get(id) {
        return Some(template);
    }
    let template: &'static StructureTemplate = Box::leak(Box::new(try_load(id)?));
    Some(*STRUCTURE_REPOSITORY.write().expect("structure repository").entry(id.to_string()).or_insert(template))
}

pub fn get_or_create(id: &str) -> &'static StructureTemplate {
    if let Some(&template) = STRUCTURE_REPOSITORY.read().expect("structure repository").get(id) {
        return template;
    }
    let template: &'static StructureTemplate = Box::leak(Box::new(try_load(id).unwrap_or(StructureTemplate::EMPTY)));
    *STRUCTURE_REPOSITORY.write().expect("structure repository").entry(id.to_string()).or_insert(template)
}

fn try_load(id: &str) -> Option<StructureTemplate> {
    let (namespace, path) = id.split_once(':').unwrap_or(("minecraft", id));
    let location = format!("{namespace}:{STRUCTURE_DIRECTORY}/{path}{STRUCTURE_FILE_EXTENSION}");
    let resource = resources().get(&location)?.clone();
    let mut decompressed = Vec::new();
    let tag = resource
        .supplier
        .read()
        .and_then(|bytes| GzDecoder::new(&bytes[..]).read_to_end(&mut decompressed))
        .and_then(|_| decode_nbt(&decompressed));
    match tag {
        Ok(tag) => Some(StructureTemplate::load(&tag)),
        Err(e) => {
            error!("Couldn't load structure {id}\n{e}");
            None
        }
    }
}
