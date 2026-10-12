use crate::commands::arguments::coordinates::get_loaded_block_pos;
use crate::commands::brigadier::{CommandContext, CommandSyntaxException};
use crate::commands::source::CommandSourceStack;
use crate::server::Server;
use crate::world::level::BlockGetter;
use crate::world::level::storage::loot::loot_context::{LootContext, LootContextBuilder};
use crate::world::level::storage::loot::loot_params::LootParams;

pub fn create_context<R>(
    server: &mut Server,
    context: &CommandContext,
    source: &CommandSourceStack,
    output: impl FnOnce(&mut LootContext) -> R,
) -> Result<R, CommandSyntaxException> {
    let pos = if context.has_argument("computePos") { Some(get_loaded_block_pos(server, context, source, "computePos")?) } else { None };
    let target = if context.has_argument("computeTarget") { Some(context.get_entity(server, source, "computeTarget")?) } else { None };
    let level = &mut server.levels[source.level];
    let block = pos.map(|pos| (level.block_state(pos), level.get_block_entity(pos).cloned()));
    let mut params = LootParams::new(level);
    params.this_entity = source.entity;
    params.origin = Some(source.position);
    if let Some((block_state, block_entity)) = &block {
        params.block_state = Some(*block_state);
        params.block_entity = block_entity.as_ref();
    }
    params.target_entity = target;
    let mut loot_context = LootContextBuilder::new(params).create(None);
    Ok(output(&mut loot_context))
}
