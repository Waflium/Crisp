use crisp_data::packets::play::clientbound;

use crate::commands::source::CommandSourceStack;
use crate::network::connection::Packet;
use crate::server::{Server, Task};
use crate::world::entity::EntityId;

const MAX_COMMAND_SUGGESTIONS: usize = 1000;
const MAX_COMMAND_LENGTH: usize = 32500;

#[derive(Default)]
pub struct ServerCommandSuggestionsProvider {
    last_request: Option<(i32, String)>,
    has_budget: bool,
}

impl Server {
    pub(in crate::server) fn handle_custom_command_suggestions(&mut self, id: EntityId, packet: &Packet) -> anyhow::Result<()> {
        let mut reader = packet.reader();
        let request_id = reader.varint()?;
        let command = reader.string(MAX_COMMAND_LENGTH)?.to_owned();
        let provider = &mut self.player_mut(id).command_suggestions;
        if provider.last_request.replace((request_id, command)).is_none() && provider.has_budget {
            self.tasks.push_back(Task::CommandSuggestions { player: id });
        }
        Ok(())
    }

    pub(in crate::server) fn tick_command_suggestions(&mut self, id: EntityId) {
        self.player_mut(id).command_suggestions.has_budget = true;
        self.try_process_command_suggestions(id);
    }

    pub(in crate::server) fn try_process_command_suggestions(&mut self, id: EntityId) {
        let provider = &mut self.player_mut(id).command_suggestions;
        if !provider.has_budget {
            return;
        }
        let Some((request_id, command)) = provider.last_request.take() else { return };
        provider.has_budget = false;
        let source = CommandSourceStack::player(self, id);
        let Some(mut suggestions) = crate::commands::completion_suggestions(self, &source, &command) else {
            error!("Failed to resolve command suggestions for {}", self.player(id).profile.name);
            return;
        };
        suggestions.list.truncate(MAX_COMMAND_SUGGESTIONS);
        self.player(id).connection.send(clientbound::COMMAND_SUGGESTIONS, |w| {
            w.varint(request_id);
            w.varint(suggestions.range.start as i32);
            w.varint((suggestions.range.end - suggestions.range.start) as i32);
            w.varint(suggestions.list.len() as i32);
            for suggestion in &suggestions.list {
                w.string(&suggestion.text);
                w.bool(suggestion.tooltip.is_some());
                if let Some(tooltip) = &suggestion.tooltip {
                    tooltip.write(w);
                }
            }
        });
    }
}
