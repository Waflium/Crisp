use crate::commands::brigadier::{CommandContext, CommandSyntaxException};
use crate::commands::source::CommandSourceStack;
use crate::network::chat::{Arg, ChatFormatting, ClickEvent, Component, HoverEvent};
use crate::server::Server;

pub fn copy_on_click_text(text: &str) -> Component {
    let inner = Component::literal(text)
        .with_color(ChatFormatting::Green)
        .with_click(ClickEvent::CopyToClipboard(text.to_string()))
        .with_hover(HoverEvent::ShowText(Box::new(Component::translatable("chat.copy.click"))))
        .with_insertion(text);
    Component::translatable_with("chat.square_brackets", vec![Arg::Component(inner)])
}

pub fn seed(server: &mut Server, _context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let seed = server.seed;
    let text = copy_on_click_text(&seed.to_string());
    source.send_success(server, || Component::translatable_with("commands.seed.success", vec![Arg::Component(text)]), false);
    Ok(seed as i32)
}
