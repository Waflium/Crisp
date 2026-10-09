use std::collections::VecDeque;

use crisp_core::BlockPos;
use crisp_data::commands::CommandNode;
use crisp_data::{BlockState, blocks, game_rule};

use crate::commands::CommandFn;
use crate::commands::arguments::ERROR_INVALID_DIMENSION;
use crate::commands::arguments::blocks::BlockPredicate;
use crate::commands::arguments::coordinates::{ERROR_NOT_LOADED, get_loaded_block_pos};
use crate::commands::brigadier::{CommandContext, CommandSyntaxException, Dynamic2CommandExceptionType, SimpleCommandExceptionType, tr};
use crate::commands::source::CommandSourceStack;
use crate::network::chat::{Arg, Component};
use crate::server::Server;
use crate::server::level::ServerLevel;
use crate::world::level::BlockGetter;
use crate::world::level::levelgen::structure::BoundingBox;

static ERROR_OVERLAP: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.clone.overlap");
static ERROR_AREA_TOO_LARGE: Dynamic2CommandExceptionType = Dynamic2CommandExceptionType(|max, count| tr("commands.clone.toobig", vec![max, count]));
static ERROR_FAILED: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.clone.failed");

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Force,
    Move,
    Normal,
}

impl Mode {
    fn can_overlap(self) -> bool {
        self != Mode::Normal
    }
}

enum Filter<'a> {
    All,
    Masked,
    Predicate(&'a BlockPredicate),
}

pub fn executors() -> Vec<(String, CommandFn)> {
    let mut out: Vec<(String, CommandFn)> = Vec::new();
    for begin in ["clone begin", "clone from sourceDimension begin"] {
        for destination in ["end destination", "end to targetDimension destination"] {
            for strict in ["", " strict"] {
                let base = format!("{begin} {destination}{strict}");
                out.push((base.clone(), clone));
                for filter in ["replace", "masked", "filtered filter"] {
                    out.push((format!("{base} {filter}"), clone));
                    for mode in ["force", "move", "normal"] {
                        out.push((format!("{base} {filter} {mode}"), clone));
                    }
                }
            }
        }
    }
    out
}

fn get_dimension(server: &Server, context: &CommandContext, name: &str) -> Result<(), CommandSyntaxException> {
    let id = context.get_identifier(name);
    if id != server.level.dimension {
        return Err(ERROR_INVALID_DIMENSION.create(id));
    }
    Ok(())
}

pub fn clone(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let literal = |name: &str| context.nodes.iter().any(|n| CommandNode::get(n.node).name == name);
    let from_dimension = |server: &Server| if context.has_argument("sourceDimension") { get_dimension(server, context, "sourceDimension") } else { Ok(()) };
    from_dimension(server)?;
    let begin = get_loaded_block_pos(server, context, source, "begin")?;
    from_dimension(server)?;
    let end = get_loaded_block_pos(server, context, source, "end")?;
    if context.has_argument("targetDimension") {
        get_dimension(server, context, "targetDimension")?;
    }
    let destination = get_loaded_block_pos(server, context, source, "destination")?;
    let filter = if context.has_argument("filter") {
        Filter::Predicate(context.get_block_predicate("filter"))
    } else if literal("masked") {
        Filter::Masked
    } else {
        Filter::All
    };
    let mode = if literal("force") {
        Mode::Force
    } else if literal("move") {
        Mode::Move
    } else {
        Mode::Normal
    };
    clone_blocks(server, source, begin, end, destination, filter, mode, literal("strict"))
}

fn has_chunks_at(level: &ServerLevel, pos0: BlockPos, pos1: BlockPos) -> bool {
    let chunks = &level.chunks;
    if pos1.y < chunks.min_y() || pos0.y > chunks.max_y() {
        return false;
    }
    (pos0.x >> 4..=pos1.x >> 4).all(|x| (pos0.z >> 4..=pos1.z >> 4).all(|z| chunks.has_chunk(crisp_core::ChunkPos::new(x, z))))
}

struct CloneBlockInfo {
    pos: BlockPos,
    state: BlockState,
    previous_state_at_destination: BlockState,
}

#[allow(clippy::too_many_arguments)]
fn clone_blocks(
    server: &mut Server,
    source: &CommandSourceStack,
    start: BlockPos,
    end: BlockPos,
    destination_pos: BlockPos,
    filter: Filter,
    mode: Mode,
    strict: bool,
) -> Result<i32, CommandSyntaxException> {
    let from = BoundingBox::from_corners(start, end);
    let length = from.length();
    let destination_end = destination_pos.offset(length.x, length.y, length.z);
    let destination = BoundingBox::from_corners(destination_pos, destination_end);
    if !mode.can_overlap() && destination.intersects(&from) {
        return Err(ERROR_OVERLAP.create());
    }
    let area = from.x_span() as i64 * from.y_span() as i64 * from.z_span() as i64;
    let limit = server.level.game_rules.get_int(game_rule::MAX_BLOCK_MODIFICATIONS);
    if area > limit as i64 {
        return Err(ERROR_AREA_TOO_LARGE.create(limit, Arg::Long(area)));
    }
    let level = &mut server.level;
    if !has_chunks_at(level, start, end) || !has_chunks_at(level, destination_pos, destination_end) {
        return Err(ERROR_NOT_LOADED.create());
    }
    let mut solid = Vec::new();
    let mut other = Vec::new();
    let mut clear = VecDeque::new();
    let offset = BlockPos::new(destination.min.x - from.min.x, destination.min.y - from.min.y, destination.min.z - from.min.z);
    for z in from.min.z..=from.max.z {
        for y in from.min.y..=from.max.y {
            for x in from.min.x..=from.max.x {
                let source_pos = BlockPos::new(x, y, z);
                let destination_pos = source_pos.offset(offset.x, offset.y, offset.z);
                let state = level.block_state(source_pos);
                let passes = match filter {
                    Filter::All => true,
                    Filter::Masked => !state.is_air(),
                    Filter::Predicate(predicate) => predicate.test(level, source_pos),
                };
                if !passes {
                    continue;
                }
                let info = CloneBlockInfo { pos: destination_pos, state, previous_state_at_destination: level.block_state(destination_pos) };
                if !state.is_solid_render() && !state.is_collision_shape_full_block() {
                    other.push(info);
                    clear.push_front(source_pos);
                } else {
                    solid.push(info);
                    clear.push_back(source_pos);
                }
            }
        }
    }
    let default_flags = 2 | if strict { 816 } else { 0 };
    if mode == Mode::Move {
        for &pos in &clear {
            level.set_block(pos, blocks::BARRIER.default_state(), default_flags | 816);
        }
        let standard_flags = if strict { default_flags } else { 3 };
        for &pos in &clear {
            level.set_block(pos, blocks::AIR.default_state(), standard_flags);
        }
    }
    let infos: Vec<CloneBlockInfo> = solid.into_iter().chain(other).collect();
    for info in infos.iter().rev() {
        level.set_block(info.pos, blocks::BARRIER.default_state(), default_flags | 816);
    }
    let mut count = 0;
    for info in &infos {
        if level.set_block(info.pos, info.state, default_flags) {
            count += 1;
        }
    }
    if !strict {
        for info in infos.iter().rev() {
            level.update_neighbours_on_block_set(info.pos, info.previous_state_at_destination);
        }
    }
    level.chunks.block_ticks.copy_area(&from, offset);
    if count == 0 {
        return Err(ERROR_FAILED.create());
    }
    source.send_success(server, || Component::translatable_with("commands.clone.success", vec![Arg::Int(count)]), true);
    Ok(count)
}
