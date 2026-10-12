use crate::commands::brigadier::{CommandSyntaxException, DynamicCommandExceptionType, StringReader, tr};
use crate::commands::shared_suggestion_provider;
use crate::commands::suggestion::SuggestionsBuilder;
use crate::world::inventory::slot_ranges;

static ERROR_ONLY_SINGLE_SLOT_ALLOWED: DynamicCommandExceptionType = DynamicCommandExceptionType(|id| tr("slot.only_single_allowed", vec![id]));

pub fn parse(reader: &mut StringReader) -> Result<i32, CommandSyntaxException> {
    let result = slot_ranges::read(reader)?;
    if result.slots.len() != 1 {
        return Err(ERROR_ONLY_SINGLE_SLOT_ALLOWED.create_with_context(reader, result.name.clone()));
    }
    Ok(result.slots[0])
}

pub fn list_suggestions(builder: &mut SuggestionsBuilder) {
    shared_suggestion_provider::suggest(slot_ranges::single_slot_names(), builder);
}
