use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::{LazyLock, RwLock};

use crisp_data::RegistryTags;
use crisp_protocol::nbt::Nbt;

use super::packs::repository::{PackRepository, VANILLA_ID};
use super::packs::resources::MultiPackResourceManager;
use crate::advancements::advancement::Advancement;
use crate::advancements::advancement_holder::AdvancementHolder;
use crate::tags::identifier_order;
use crate::world::item::component::json_to_nbt;
use crate::world::item::crafting::recipe::Recipe;
use crate::world::item::crafting::recipe_holder::RecipeHolder;
use crate::world::item::crafting::recipe_manager::RecipeManager;
use crate::world::item::slot::slot_source::SlotSource;
use crate::world::level::storage::loot::functions::loot_item_function::LootItemFunction;
use crate::world::level::storage::loot::loot_table::LootTable;
use crate::world::level::storage::loot::predicates::loot_item_condition::LootItemCondition;
use crate::world::level::storage::loot::providers::number::floats::ContextFloatProvider;
use crate::world::level::storage::loot::providers::number::ints::ContextIntProvider;

pub const LOOT_TABLE: &str = "minecraft:loot_table";
pub const PREDICATE: &str = "minecraft:predicate";
pub const CONTEXT_FLOAT_PROVIDER: &str = "minecraft:context_float_provider";
pub const CONTEXT_INT_PROVIDER: &str = "minecraft:context_int_provider";
pub const ITEM_MODIFIER: &str = "minecraft:item_modifier";
pub const SLOT_SOURCE: &str = "minecraft:slot_source";
pub const ADVANCEMENT: &str = "minecraft:advancement";
pub const RECIPE: &str = "minecraft:recipe";
const RELOADABLE_REGISTRIES: [&str; 8] = [LOOT_TABLE, PREDICATE, CONTEXT_FLOAT_PROVIDER, CONTEXT_INT_PROVIDER, ITEM_MODIFIER, SLOT_SOURCE, ADVANCEMENT, RECIPE];

#[derive(PartialEq)]
struct Source {
    id: String,
    pack: String,
    bytes: Vec<u8>,
}

pub struct ReloadableRegistries {
    sources: Vec<Vec<Source>>,
    tags: &'static [RegistryTags],
    pub loot_tables: HashMap<String, LootTable>,
    pub predicates: HashMap<String, LootItemCondition>,
    pub context_float_providers: HashMap<String, ContextFloatProvider>,
    pub context_int_providers: HashMap<String, ContextIntProvider>,
    pub item_modifiers: HashMap<String, LootItemFunction>,
    pub slot_sources: HashMap<String, SlotSource>,
    pub advancements: Vec<&'static AdvancementHolder>,
}

impl ReloadableRegistries {
    fn contains_key(&self, registry: &str, key: &str) -> bool {
        match registry {
            LOOT_TABLE => self.loot_tables.contains_key(key),
            PREDICATE => self.predicates.contains_key(key),
            CONTEXT_FLOAT_PROVIDER => self.context_float_providers.contains_key(key),
            CONTEXT_INT_PROVIDER => self.context_int_providers.contains_key(key),
            ITEM_MODIFIER => self.item_modifiers.contains_key(key),
            SLOT_SOURCE => self.slot_sources.contains_key(key),
            ADVANCEMENT => self.advancements.iter().any(|holder| holder.id == key),
            _ => false,
        }
    }
}

pub enum LoadResult {
    Unchanged,
    Loaded { registries: ReloadableRegistries, recipes: Vec<RecipeHolder> },
}

thread_local! {
    static PENDING_KEYS: RefCell<Option<HashMap<&'static str, HashSet<String>>>> = const { RefCell::new(None) };
}

static BOUND: RwLock<Option<&'static ReloadableRegistries>> = RwLock::new(None);
static VANILLA: LazyLock<(&'static ReloadableRegistries, &'static RecipeManager)> = LazyLock::new(|| {
    let mut repository = PackRepository::create_vanilla_trusted_repository();
    repository.reload();
    repository.set_selected(&[VANILLA_ID]);
    let resource_manager = MultiPackResourceManager::new(repository.open_all_selected());
    match load(&resource_manager, crisp_data::tags(), None) {
        Ok(LoadResult::Loaded { registries, recipes }) => (Box::leak(Box::new(registries)), Box::leak(Box::new(RecipeManager::new(recipes)))),
        Ok(LoadResult::Unchanged) => unreachable!(),
        Err(errors) => panic!("{errors}"),
    }
});

pub fn get() -> &'static ReloadableRegistries {
    match *BOUND.read().expect("reloadable registries") {
        Some(registries) => registries,
        None => VANILLA.0,
    }
}

pub fn vanilla_recipe_manager() -> &'static RecipeManager {
    VANILLA.1
}

pub fn contains_key(registry: &str, key: &str) -> bool {
    PENDING_KEYS
        .with(|pending| pending.borrow().as_ref().map(|keys| keys.get(registry).is_some_and(|keys| keys.contains(key))))
        .unwrap_or_else(|| get().contains_key(registry, key))
}

pub fn bind(registries: ReloadableRegistries) {
    *BOUND.write().expect("reloadable registries") = Some(Box::leak(Box::new(registries)));
}

fn elements_dir_path(registry: &str) -> &str {
    registry.strip_prefix("minecraft:").unwrap_or(registry)
}

fn list_sources(resource_manager: &MultiPackResourceManager, registry: &str) -> Vec<Source> {
    let directory = elements_dir_path(registry);
    let mut resources: Vec<_> = resource_manager.list_resources(directory, ".json").into_iter().collect();
    identifier_order(&mut resources);
    resources
        .into_iter()
        .map(|(location, resource)| {
            let (namespace, path) = location.split_once(':').unwrap();
            let id = format!("{namespace}:{}", &path[directory.len() + 1..path.len() - ".json".len()]);
            let bytes = resource.supplier.read().unwrap_or_default();
            Source { id, pack: resource.source_pack_id, bytes }
        })
        .collect()
}

struct LoadingErrors(BTreeMap<(String, String), String>);

impl LoadingErrors {
    fn report(&self) -> String {
        let mut by_registry: BTreeMap<&str, Vec<(&str, &str)>> = BTreeMap::new();
        for ((registry, id), error) in &self.0 {
            by_registry.entry(registry).or_default().push((id, error));
        }
        let mut collected_errors = String::new();
        for (registry, mut elements) in by_registry {
            collected_errors.push_str(&format!("> Errors in registry {registry}:\n"));
            elements.sort_by(|a, b| {
                let split = |id: &str| id.split_once(':').map(|(namespace, path)| (path.to_string(), namespace.to_string())).unwrap_or_default();
                split(a.0).cmp(&split(b.0))
            });
            for (id, error) in elements {
                collected_errors.push_str(&format!(">> Errors in element {id}:\n{error}\n"));
            }
        }
        format!("Registry loading errors:\n{collected_errors}")
    }
}

fn decode_registry<T>(registry: &str, sources: &[Source], decode: fn(&Nbt) -> Result<T, String>, errors: &mut LoadingErrors) -> HashMap<String, T> {
    let mut elements = HashMap::with_capacity(sources.len());
    for source in sources {
        let parsed = serde_json::from_slice::<serde_json::Value>(&source.bytes)
            .map_err(|e| format!("com.google.gson.JsonSyntaxException: {e}"))
            .and_then(|json| crate::nbt::with_json_source(|| decode(&json_to_nbt(&json))).map_err(|e| format!("java.lang.IllegalStateException: {e}")));
        match parsed {
            Ok(element) => {
                elements.insert(source.id.clone(), element);
            }
            Err(cause) => {
                let error = format!("java.lang.IllegalStateException: Failed to parse {} from pack {}\nCaused by: {cause}", source.id, source.pack);
                errors.0.insert((registry.to_string(), source.id.clone()), error);
            }
        }
    }
    elements
}

pub fn load(resource_manager: &MultiPackResourceManager, tags: &'static [RegistryTags], current: Option<&ReloadableRegistries>) -> Result<LoadResult, String> {
    let sources: Vec<Vec<Source>> = RELOADABLE_REGISTRIES.iter().map(|registry| list_sources(resource_manager, registry)).collect();
    if current.is_some_and(|current| std::ptr::eq(current.tags, tags) && current.sources == sources) {
        return Ok(LoadResult::Unchanged);
    }
    let keys = RELOADABLE_REGISTRIES.iter().zip(&sources).map(|(&registry, sources)| (registry, sources.iter().map(|source| source.id.clone()).collect())).collect();
    let previous = PENDING_KEYS.with(|pending| pending.replace(Some(keys)));
    let mut errors = LoadingErrors(BTreeMap::new());
    let (registries, recipes) = crisp_data::with_pending_tags(tags, || {
        let loot_tables = decode_registry(LOOT_TABLE, &sources[0], LootTable::decode, &mut errors);
        let predicates = decode_registry(PREDICATE, &sources[1], LootItemCondition::decode, &mut errors);
        let context_float_providers = decode_registry(CONTEXT_FLOAT_PROVIDER, &sources[2], ContextFloatProvider::decode, &mut errors);
        let context_int_providers = decode_registry(CONTEXT_INT_PROVIDER, &sources[3], ContextIntProvider::decode, &mut errors);
        let item_modifiers = decode_registry(ITEM_MODIFIER, &sources[4], LootItemFunction::decode_typed, &mut errors);
        let slot_sources = decode_registry(SLOT_SOURCE, &sources[5], SlotSource::decode_typed, &mut errors);
        let mut decoded_advancements = decode_registry(ADVANCEMENT, &sources[6], Advancement::decode, &mut errors);
        let advancements: Vec<&'static AdvancementHolder> = sources[6]
            .iter()
            .filter_map(|source| decoded_advancements.remove(&source.id).map(|value| &*Box::leak(Box::new(AdvancementHolder { id: source.id.clone(), value }))))
            .collect();
        let mut decoded_recipes = decode_registry(RECIPE, &sources[7], Recipe::decode, &mut errors);
        let recipes: Vec<RecipeHolder> =
            sources[7].iter().filter_map(|source| decoded_recipes.remove(&source.id).map(|value| RecipeHolder { id: source.id.clone().leak(), value })).collect();
        (
            ReloadableRegistries {
                sources: Vec::new(),
                tags,
                loot_tables,
                predicates,
                context_float_providers,
                context_int_providers,
                item_modifiers,
                slot_sources,
                advancements,
            },
            recipes,
        )
    });
    PENDING_KEYS.with(|pending| pending.replace(previous));
    if !errors.0.is_empty() {
        let report = errors.report();
        error!("{report}");
        return Err(report);
    }
    Ok(LoadResult::Loaded { registries: ReloadableRegistries { sources, ..registries }, recipes })
}
