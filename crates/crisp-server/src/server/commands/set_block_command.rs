use crisp_core::BlockPos;

use crate::commands::arguments::blocks::BlockInput;
use crate::commands::arguments::coordinates::get_loaded_block_pos;
use crate::commands::brigadier::{CommandContext, CommandSyntaxException, SimpleCommandExceptionType};
use crate::commands::source::CommandSourceStack;
use crate::network::chat::{Arg, Component};
use crate::server::Server;
use crate::world::level::BlockGetter;
use crate::world::level::update_flags::UPDATE_LIMIT;

static ERROR_FAILED: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.setblock.failed");

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Replace,
    Destroy,
}

pub fn replace(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let pos = get_loaded_block_pos(server, context, source, "pos")?;
    set_block(server, source, pos, context.get_block("block"), Mode::Replace, false, false)
}

pub fn destroy(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let pos = get_loaded_block_pos(server, context, source, "pos")?;
    set_block(server, source, pos, context.get_block("block"), Mode::Destroy, false, false)
}

pub fn keep(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let pos = get_loaded_block_pos(server, context, source, "pos")?;
    set_block(server, source, pos, context.get_block("block"), Mode::Replace, true, false)
}

pub fn strict(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let pos = get_loaded_block_pos(server, context, source, "pos")?;
    set_block(server, source, pos, context.get_block("block"), Mode::Replace, false, true)
}

fn set_block(server: &mut Server, source: &CommandSourceStack, pos: BlockPos, block: &BlockInput, mode: Mode, keep: bool, strict: bool) -> Result<i32, CommandSyntaxException> {
    let level = &mut server.level;
    if keep && !level.block_state(pos).is_air() {
        return Err(ERROR_FAILED.create());
    }
    let place_needed = if mode == Mode::Destroy {
        level.destroy_block(pos, true, None, UPDATE_LIMIT);
        !block.state.is_air() || !level.block_state(pos).is_air()
    } else {
        true
    };
    let old = level.block_state(pos);
    if place_needed && !block.place(level, pos, 2 | if strict { 816 } else { 256 }) {
        return Err(ERROR_FAILED.create());
    }
    if !strict {
        level.update_neighbours_on_block_set(pos, old);
    }
    source.send_success(server, || Component::translatable_with("commands.setblock.success", vec![Arg::Int(pos.x), Arg::Int(pos.y), Arg::Int(pos.z)]), true);
    Ok(1)
}
