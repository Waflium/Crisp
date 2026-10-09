use crisp_core::ChunkPos;

use super::command_response_tracker::{CommandResponseTracker, ElementType};
use crate::commands::arguments::coordinates::ERROR_OUT_OF_WORLD;
use crate::commands::brigadier::{CommandContext, CommandSyntaxException, Dynamic2CommandExceptionType, SimpleCommandExceptionType, tr};
use crate::commands::source::CommandSourceStack;
use crate::network::chat::{Arg, Component};
use crate::server::Server;

const MAX_CHUNK_LIMIT: i64 = 256;

static ERROR_TOO_MANY_CHUNKS: Dynamic2CommandExceptionType = Dynamic2CommandExceptionType(|max, amount| tr("commands.forceload.toobig", vec![max, amount]));
static ERROR_NOT_TICKING: Dynamic2CommandExceptionType = Dynamic2CommandExceptionType(|pos, dimension| tr("commands.forceload.query.failure", vec![pos, dimension]));
static ERROR_ALL_ADDED: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.forceload.added.failure");
static ERROR_NONE_REMOVED: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.forceload.removed.failure");

fn chunk_string(pos: ChunkPos) -> String {
    format!("[{}, {}]", pos.x, pos.z)
}

fn chunk_arg(pos: ChunkPos) -> Arg {
    Component::literal(chunk_string(pos)).into()
}

fn dimension_arg(server: &Server) -> Arg {
    Component::literal(server.level.dimension).into()
}

fn column_pos(server: &Server, context: &CommandContext, source: &CommandSourceStack, name: &str) -> (i32, i32) {
    let pos = context.get_coordinates(name).block_pos(server, source);
    (pos.x, pos.z)
}

pub fn query_force_load(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let (x, z) = column_pos(server, context, source, "pos");
    let pos = ChunkPos::new(x >> 4, z >> 4);
    if !server.level.chunks.force_loaded_chunks().contains(pos.pack()) {
        return Err(ERROR_NOT_TICKING.create(chunk_string(pos), server.level.dimension));
    }
    source.send_success(server, || Component::translatable_with("commands.forceload.query.success", vec![chunk_arg(pos), dimension_arg(server)]), false);
    Ok(1)
}

pub fn list_force_load(server: &mut Server, _context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let mut chunks: Vec<i64> = server.level.chunks.force_loaded_chunks().iter().collect();
    let count = chunks.len() as i32;
    if count > 0 {
        chunks.sort();
        let list = chunks.into_iter().map(|c| chunk_string(ChunkPos::unpack(c))).collect::<Vec<_>>().join(", ");
        let message = if count == 1 {
            Component::translatable_with("commands.forceload.list.single", vec![dimension_arg(server), list.into()])
        } else {
            Component::translatable_with("commands.forceload.list.multiple", vec![count.into(), dimension_arg(server), list.into()])
        };
        source.send_success(server, || message.clone(), false);
    } else {
        source.send_failure(server, Component::translatable_with("commands.forceload.added.none", vec![dimension_arg(server)]));
    }
    Ok(count)
}

pub fn remove_all(server: &mut Server, _context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let chunks: Vec<i64> = server.level.chunks.force_loaded_chunks().iter().collect();
    for chunk in chunks {
        let pos = ChunkPos::unpack(chunk);
        server.level.set_chunk_forced(pos.x, pos.z, false);
    }
    source.send_success(server, || Component::translatable_with("commands.forceload.removed.all", vec![dimension_arg(server)]), true);
    Ok(0)
}

pub fn add(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    change_force_load(server, context, source, true)
}

pub fn remove(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    change_force_load(server, context, source, false)
}

fn change_force_load(server: &mut Server, context: &CommandContext, source: &CommandSourceStack, add: bool) -> Result<i32, CommandSyntaxException> {
    let from = column_pos(server, context, source, "from");
    let to = if context.has_argument("to") { column_pos(server, context, source, "to") } else { from };
    let (min_x, min_z, max_x, max_z) = (from.0.min(to.0), from.1.min(to.1), from.0.max(to.0), from.1.max(to.1));
    if min_x < -30_000_000 || min_z < -30_000_000 || max_x >= 30_000_000 || max_z >= 30_000_000 {
        return Err(ERROR_OUT_OF_WORLD.create());
    }
    let (min_chunk_x, min_chunk_z, max_chunk_x, max_chunk_z) = (min_x >> 4, min_z >> 4, max_x >> 4, max_z >> 4);
    let chunk_count = (max_chunk_x as i64 - min_chunk_x as i64 + 1) * (max_chunk_z as i64 - min_chunk_z as i64 + 1);
    if chunk_count > MAX_CHUNK_LIMIT {
        return Err(ERROR_TOO_MANY_CHUNKS.create(MAX_CHUNK_LIMIT as i32, Arg::Long(chunk_count)));
    }
    let mut tracker = CommandResponseTracker::new();
    for x in min_chunk_x..=max_chunk_x {
        for z in min_chunk_z..=max_chunk_z {
            let changed = server.level.set_chunk_forced(x, z, add);
            tracker.track_value(ChunkPos::new(x, z), changed as i32);
        }
    }
    let (min, max) = (ChunkPos::new(min_chunk_x, min_chunk_z), ChunkPos::new(max_chunk_x, max_chunk_z));
    let (single, multiple, on_zero): (&str, &str, fn() -> CommandSyntaxException) = if add {
        ("commands.forceload.added.single", "commands.forceload.added.multiple", || ERROR_ALL_ADDED.create())
    } else {
        ("commands.forceload.removed.single", "commands.forceload.removed.multiple", || ERROR_NONE_REMOVED.create())
    };
    tracker.send_feedback_of(
        server,
        source,
        true,
        ElementType::NonZero,
        Some(on_zero),
        |pos, _| Component::translatable_with(single, vec![chunk_arg(pos), dimension_arg(server)]),
        |count, _| Component::translatable_with(multiple, vec![count.into(), dimension_arg(server), chunk_arg(min), chunk_arg(max)]),
    )
}
