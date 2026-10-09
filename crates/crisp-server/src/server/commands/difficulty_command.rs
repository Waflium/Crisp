use crisp_data::commands::CommandNode;

use crate::commands::brigadier::{CommandContext, CommandSyntaxException, DynamicCommandExceptionType, tr};
use crate::commands::source::CommandSourceStack;
use crate::network::chat::Component;
use crate::server::Server;
use crate::world::difficulty::Difficulty;

static ERROR_ALREADY_SAME_DIFFICULTY: DynamicCommandExceptionType = DynamicCommandExceptionType(|difficulty| tr("commands.difficulty.failure", vec![difficulty]));

pub fn query(server: &mut Server, _context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let difficulty = server.difficulty;
    source.send_success(server, || Component::translatable_with("commands.difficulty.query", vec![difficulty.display_name().into()]), false);
    Ok(difficulty.id())
}

pub fn set_difficulty(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let difficulty = Difficulty::by_name(CommandNode::get(context.nodes[1].node).name).expect("a difficulty literal");
    if server.difficulty == difficulty {
        return Err(ERROR_ALREADY_SAME_DIFFICULTY.create(difficulty.display_name()));
    }
    server.set_difficulty(difficulty, true);
    source.send_success(server, || Component::translatable_with("commands.difficulty.success", vec![difficulty.display_name().into()]), true);
    Ok(0)
}
