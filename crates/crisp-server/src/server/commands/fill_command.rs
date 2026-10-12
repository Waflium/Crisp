use crisp_core::BlockPos;
use crisp_data::{BlockState, blocks, game_rule};

use crate::commands::CommandFn;
use crate::commands::arguments::blocks::{BlockInput, BlockPredicate};
use crate::commands::arguments::coordinates::get_loaded_block_pos;
use crate::commands::brigadier::{CommandContext, CommandSyntaxException, Dynamic2CommandExceptionType, SimpleCommandExceptionType, tr};
use crate::commands::source::CommandSourceStack;
use crate::network::chat::{Arg, Component};
use crate::server::Server;
use crate::server::level::ServerLevel;
use crate::world::level::BlockGetter;
use crate::world::level::levelgen::structure::BoundingBox;
use crate::world::level::update_flags::UPDATE_LIMIT;

static ERROR_AREA_TOO_LARGE: Dynamic2CommandExceptionType = Dynamic2CommandExceptionType(|max, count| tr("commands.fill.toobig", vec![max, count]));
static ERROR_FAILED: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.fill.failed");

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Replace,
    Outline,
    Hollow,
    Destroy,
}

impl Mode {
    fn affect(self, level: &mut ServerLevel, pos: BlockPos) -> bool {
        self == Mode::Destroy && level.destroy_block(pos, true, None, UPDATE_LIMIT)
    }

    fn filter<'a>(self, region: &BoundingBox, pos: BlockPos, block: &'a BlockInput, hollow_core: &'a BlockInput) -> Option<&'a BlockInput> {
        let inside = pos.x != region.min.x && pos.x != region.max.x && pos.y != region.min.y && pos.y != region.max.y && pos.z != region.min.z && pos.z != region.max.z;
        match self {
            Mode::Outline if inside => None,
            Mode::Hollow if inside => Some(hollow_core),
            _ => Some(block),
        }
    }
}

enum Filter<'a> {
    Predicate(&'a BlockPredicate),
    Empty,
}

pub fn executors() -> Vec<(String, CommandFn)> {
    let mut out: Vec<(String, CommandFn)> = Vec::new();
    for base in ["fill from to block", "fill from to block replace filter"] {
        out.push((base.into(), replace));
        out.push((format!("{base} outline"), outline));
        out.push((format!("{base} hollow"), hollow));
        out.push((format!("{base} destroy"), destroy));
        out.push((format!("{base} strict"), strict));
    }
    out.push(("fill from to block replace".into(), replace));
    out.push(("fill from to block keep".into(), keep));
    out
}

fn fill(server: &mut Server, context: &CommandContext, source: &CommandSourceStack, mode: Mode, keep: bool, strict: bool) -> Result<i32, CommandSyntaxException> {
    let from = get_loaded_block_pos(server, context, source, "from")?;
    let to = get_loaded_block_pos(server, context, source, "to")?;
    let filter = if keep {
        Some(Filter::Empty)
    } else if context.has_argument("filter") {
        Some(Filter::Predicate(context.get_block_predicate("filter")))
    } else {
        None
    };
    fill_blocks(server, source, BoundingBox::from_corners(from, to), context.get_block("block"), mode, filter, strict)
}

pub fn replace(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    fill(server, context, source, Mode::Replace, false, false)
}

pub fn outline(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    fill(server, context, source, Mode::Outline, false, false)
}

pub fn hollow(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    fill(server, context, source, Mode::Hollow, false, false)
}

pub fn destroy(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    fill(server, context, source, Mode::Destroy, false, false)
}

pub fn strict(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    fill(server, context, source, Mode::Replace, false, true)
}

pub fn keep(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    fill(server, context, source, Mode::Replace, true, false)
}

fn fill_blocks(
    server: &mut Server,
    source: &CommandSourceStack,
    region: BoundingBox,
    target: &BlockInput,
    mode: Mode,
    filter: Option<Filter>,
    strict: bool,
) -> Result<i32, CommandSyntaxException> {
    let area = region.x_span() as i64 * region.y_span() as i64 * region.z_span() as i64;
    let limit = server.levels[source.level].game_rules.get_int(game_rule::MAX_BLOCK_MODIFICATIONS);
    if area > limit as i64 {
        return Err(ERROR_AREA_TOO_LARGE.create(limit, Arg::Long(area)));
    }
    let hollow_core = BlockInput { state: blocks::AIR.default_state(), properties: Vec::new(), tag: None };
    let level = &mut server.levels[source.level];
    let mut updated: Vec<(BlockPos, BlockState)> = Vec::new();
    let mut count = 0;
    for pos in region.positions() {
        let passes = match &filter {
            None => true,
            Some(Filter::Predicate(predicate)) => predicate.test(level, pos),
            Some(Filter::Empty) => level.block_state(pos).is_air(),
        };
        if !passes {
            continue;
        }
        let old = level.block_state(pos);
        let affected = mode.affect(level, pos);
        match mode.filter(&region, pos, target, &hollow_core) {
            Some(block) if block.place(level, pos, 2 | if strict { 816 } else { 256 }) => {
                if !strict {
                    updated.push((pos, old));
                }
                count += 1;
            }
            _ => {
                if affected {
                    count += 1;
                }
            }
        }
    }
    for (pos, old) in updated {
        level.update_neighbours_on_block_set(pos, old);
    }
    if count == 0 {
        return Err(ERROR_FAILED.create());
    }
    source.send_success(server, || Component::translatable_with("commands.fill.success", vec![Arg::Int(count)]), true);
    Ok(count)
}
