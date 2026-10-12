use std::fmt;
use std::sync::Arc;

use crisp_protocol::nbt::Nbt;

use super::{is_allowed_in_identifier, parse_identifier};
use crate::commands::brigadier::{CommandSyntaxException, Dynamic2CommandExceptionType, DynamicCommandExceptionType, StringReader, tr};
use crate::commands::shared_suggestion_provider;
use crate::commands::suggestion::SuggestionsBuilder;
use crate::core::holder::{Holder, RegistryElement};
use crate::server::reloadable_server_registries;
use crate::util::codec::DataResult;
use crate::world::item::slot::slot_source::SlotSource;
use crate::world::level::storage::loot::functions::loot_item_function::LootItemFunction;
use crate::world::level::storage::loot::loot_table::LootTable;
use crate::world::level::storage::loot::predicates::loot_item_condition::LootItemCondition;
use crate::world::level::storage::loot::providers::number::floats::ContextFloatProvider;
use crate::world::level::storage::loot::providers::number::ints::ContextIntProvider;

pub static ERROR_FAILED_TO_PARSE: DynamicCommandExceptionType = DynamicCommandExceptionType(|error| tr("argument.resource_or_id.failed_to_parse", vec![error]));
pub static ERROR_NO_SUCH_ELEMENT: Dynamic2CommandExceptionType =
    Dynamic2CommandExceptionType(|id, registry| tr("argument.resource_or_id.no_such_element", vec![id, registry]));

pub struct ResourceOrId<T: RegistryElement>(pub Arc<Holder<T>>);

impl<T: RegistryElement> ResourceOrId<T> {
    pub fn value(&self) -> &T {
        self.0.value()
    }

    pub fn unwrap_key(&self) -> Option<&str> {
        self.0.unwrap_key()
    }
}

impl<T: RegistryElement> Clone for ResourceOrId<T> {
    fn clone(&self) -> ResourceOrId<T> {
        ResourceOrId(self.0.clone())
    }
}

impl<T: RegistryElement> PartialEq for ResourceOrId<T> {
    fn eq(&self, other: &ResourceOrId<T>) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl<T: RegistryElement> fmt::Debug for ResourceOrId<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ResourceOrId[{} / {}]", T::REGISTRY, self.unwrap_key().unwrap_or("<inline>"))
    }
}

fn read_non_empty_identifier(reader: &mut StringReader) -> Option<String> {
    let start = reader.cursor;
    while reader.can_read() && is_allowed_in_identifier(reader.peek()) {
        reader.skip();
    }
    let raw = reader.substring(start, reader.cursor);
    if raw.is_empty() {
        return None;
    }
    parse_identifier(&raw)
}

pub fn parse<T: RegistryElement>(reader: &mut StringReader, codec: fn(&Nbt) -> DataResult<T>) -> Result<ResourceOrId<T>, CommandSyntaxException> {
    let mark = reader.cursor;
    reader.skip_whitespace();
    if let Some(id) = read_non_empty_identifier(reader) {
        if !T::contains_key(&id) {
            return Err(ERROR_NO_SUCH_ELEMENT.create_with_context(reader, id, T::REGISTRY));
        }
        return Ok(ResourceOrId(Arc::new(Holder::Reference(id, Default::default()))));
    }
    reader.cursor = mark;
    let value = crate::nbt::parse_as_argument(reader)?;
    match crate::util::codec::with_nbt_ops(|| codec(&value)) {
        Ok(value) => Ok(ResourceOrId(Arc::new(Holder::direct(value)))),
        Err(message) => Err(ERROR_FAILED_TO_PARSE.create_with_context(reader, message)),
    }
}

pub fn element_ids(registry: &str) -> Vec<&'static str> {
    let registries = reloadable_server_registries::get();
    let keys: Vec<&'static String> = match registry {
        reloadable_server_registries::LOOT_TABLE => registries.loot_tables.keys().collect(),
        reloadable_server_registries::PREDICATE => registries.predicates.keys().collect(),
        reloadable_server_registries::ITEM_MODIFIER => registries.item_modifiers.keys().collect(),
        reloadable_server_registries::CONTEXT_FLOAT_PROVIDER => registries.context_float_providers.keys().collect(),
        reloadable_server_registries::CONTEXT_INT_PROVIDER => registries.context_int_providers.keys().collect(),
        reloadable_server_registries::SLOT_SOURCE => registries.slot_sources.keys().collect(),
        _ => Vec::new(),
    };
    keys.into_iter().map(String::as_str).collect()
}

pub fn list_suggestions(registry: &str, builder: &mut SuggestionsBuilder) {
    shared_suggestion_provider::suggest_resource(element_ids(registry), builder);
}

pub fn loot_table(reader: &mut StringReader) -> Result<ResourceOrId<LootTable>, CommandSyntaxException> {
    parse(reader, LootTable::decode)
}

pub fn loot_modifier(reader: &mut StringReader) -> Result<ResourceOrId<LootItemFunction>, CommandSyntaxException> {
    parse(reader, LootItemFunction::decode_direct)
}

pub fn loot_predicate(reader: &mut StringReader) -> Result<ResourceOrId<LootItemCondition>, CommandSyntaxException> {
    parse(reader, LootItemCondition::decode)
}

pub fn float_provider(reader: &mut StringReader) -> Result<ResourceOrId<ContextFloatProvider>, CommandSyntaxException> {
    parse(reader, ContextFloatProvider::decode)
}

pub fn int_provider(reader: &mut StringReader) -> Result<ResourceOrId<ContextIntProvider>, CommandSyntaxException> {
    parse(reader, ContextIntProvider::decode)
}

pub fn slot_source(reader: &mut StringReader) -> Result<ResourceOrId<SlotSource>, CommandSyntaxException> {
    parse(reader, SlotSource::decode_direct)
}

#[derive(Clone, Debug, PartialEq)]
pub enum RawResourceOrId {
    Reference(String),
    Inline(Nbt),
}

pub fn parse_raw(
    reader: &mut StringReader,
    registry: &str,
    contains: impl Fn(&str) -> bool,
    codec: impl Fn(&Nbt) -> Result<(), String>,
) -> Result<RawResourceOrId, CommandSyntaxException> {
    let mark = reader.cursor;
    reader.skip_whitespace();
    if let Some(id) = read_non_empty_identifier(reader) {
        if !contains(&id) {
            return Err(ERROR_NO_SUCH_ELEMENT.create_with_context(reader, id, registry));
        }
        return Ok(RawResourceOrId::Reference(id));
    }
    reader.cursor = mark;
    let value = crate::nbt::parse_as_argument(reader)?;
    codec(&value).map_err(|message| ERROR_FAILED_TO_PARSE.create_with_context(reader, message))?;
    Ok(RawResourceOrId::Inline(value))
}
