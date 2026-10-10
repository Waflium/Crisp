use crate::commands::brigadier::{CommandContext, CommandSyntaxException};
use crate::commands::source::CommandSourceStack;
use crate::network::chat::Component;
use crate::server::Server;

pub fn reload_packs(server: &mut Server, selected_packs: &[String]) {
    server.reload_resources(selected_packs);
}

fn discover_new_packs(server: &mut Server, current_packs: Vec<String>) -> Vec<String> {
    server.pack_repository.reload();
    let mut selected = current_packs;
    let disabled = &server.data_configuration.data_packs.disabled;
    for pack in server.pack_repository.available_ids() {
        if !disabled.iter().any(|d| d == pack) && !selected.iter().any(|s| s == pack) {
            selected.push(pack.to_string());
        }
    }
    selected
}

pub fn reload(server: &mut Server, _context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let current_packs = server.pack_repository.selected_ids().into_iter().map(str::to_string).collect();
    let new_selected_packs = discover_new_packs(server, current_packs);
    source.send_success(server, || Component::translatable("commands.reload.success"), true);
    reload_packs(server, &new_selected_packs);
    Ok(0)
}
