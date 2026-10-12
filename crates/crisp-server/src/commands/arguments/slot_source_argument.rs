use std::sync::Arc;

use super::resource_or_id_argument::{self, ResourceOrId};
use crate::commands::brigadier::{CommandSyntaxException, StringReader};
use crate::commands::shared_suggestion_provider;
use crate::commands::suggestion::SuggestionsBuilder;
use crate::server::reloadable_server_registries;
use crate::world::inventory::slot_ranges::{self, SlotRange};
use crate::world::item::slot::slot_source::SlotSource;
use crate::world::level::storage::loot::loot_context_arg::LootContextArg;

#[derive(Clone, Debug, PartialEq)]
pub enum SlotSourceResult {
    Holder(ResourceOrId<SlotSource>),
    Literal(&'static SlotRange, ResourceOrId<SlotSource>),
}

impl SlotSourceResult {
    pub fn value(&self) -> &SlotSource {
        match self {
            SlotSourceResult::Holder(holder) | SlotSourceResult::Literal(_, holder) => holder.value(),
        }
    }

    pub fn name(&self) -> Option<String> {
        match self {
            SlotSourceResult::Holder(holder) => holder.unwrap_key().map(str::to_string),
            SlotSourceResult::Literal(slot_range, _) => Some(slot_range.name.clone()),
        }
    }
}

fn slot_range(slot_range: &'static SlotRange) -> SlotSource {
    SlotSource::SlotRange { source: LootContextArg::Container, slot_range }
}

pub fn parse(reader: &mut StringReader) -> Result<SlotSourceResult, CommandSyntaxException> {
    let start = reader.cursor;
    if let Some(range) = slot_ranges::try_read(reader) {
        return Ok(SlotSourceResult::Literal(range, ResourceOrId(Arc::new(crate::core::holder::Holder::direct(slot_range(range))))));
    }
    reader.cursor = start;
    Ok(SlotSourceResult::Holder(resource_or_id_argument::slot_source(reader)?))
}

pub fn list_suggestions(builder: &mut SuggestionsBuilder) {
    let mut sub = builder.restart();
    shared_suggestion_provider::suggest(slot_ranges::all_names(), &mut sub);
    builder.add(sub);
    resource_or_id_argument::list_suggestions(reloadable_server_registries::SLOT_SOURCE, builder);
}
