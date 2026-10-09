use crate::commands::brigadier::{CommandContext, CommandSyntaxException};
use crate::commands::source::CommandSourceStack;
use crate::network::chat::{Arg, Component};
use crate::server::Server;

pub fn set_idle_timeout(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let minutes = context.get_int("minutes");
    server.set_player_idle_timeout(minutes);
    if minutes > 0 {
        source.send_success(server, || Component::translatable_with("commands.setidletimeout.success", vec![Arg::Int(minutes)]), true);
    } else {
        source.send_success(server, || Component::translatable("commands.setidletimeout.success.disabled"), true);
    }
    Ok(minutes)
}
