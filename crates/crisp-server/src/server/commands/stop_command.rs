use crate::commands::brigadier::{CommandContext, CommandSyntaxException};
use crate::commands::source::CommandSourceStack;
use crate::network::chat::Component;
use crate::server::Server;

pub fn stop(server: &mut Server, _context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    source.send_success(server, || Component::translatable("commands.stop.stopping"), true);
    server.halt();
    Ok(1)
}
