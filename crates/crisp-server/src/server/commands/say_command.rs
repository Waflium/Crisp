use crate::commands::brigadier::{CommandContext, CommandSyntaxException};
use crate::commands::source::CommandSourceStack;
use crate::network::chat_type::{ChatType, broadcast_chat_message};
use crate::server::Server;

pub fn say(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let sender = source.clone();
    context.resolve_chat_message(server, source, "message", move |server, message| {
        let chat_type = ChatType::SayCommand.bind(sender.display_name(server));
        broadcast_chat_message(server, &message, sender.player_id(server), &chat_type);
    })?;
    Ok(1)
}
