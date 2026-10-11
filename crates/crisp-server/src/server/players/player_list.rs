use crisp_data::packets::play::clientbound;

use crate::commands::arguments::selector::entity_selector::java_equals_ignore_case;
use crate::network::chat::Component;
use crate::network::configuration::ChatVisibility;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;

impl ServerLevel {
    pub fn broadcast_system_message(&self, message: &Component) {
        info!("System chat: {}", message.get_string());
        for p in self.players.values() {
            if p.client.chat_visibility == ChatVisibility::Hidden {
                continue;
            }
            p.connection.send(clientbound::SYSTEM_CHAT, |w| {
                message.write(w);
                w.bool(false);
            });
        }
    }

    pub fn broadcast_system_to_team(&self, player: EntityId, message: &Component) {
        let Some(team) = self.team_of(player) else { return };
        for name in team.players() {
            if let Some(member) = self.players.values().find(|p| java_equals_ignore_case(&p.profile.name, name))
                && member.entity.id != player
            {
                self.send_system_message(member.entity.id, message);
            }
        }
    }

    pub fn broadcast_system_to_all_except_team(&self, player: EntityId, message: &Component) {
        let Some(team) = self.team_of(player).map(|t| t.name().to_string()) else {
            self.broadcast_system_message(message);
            return;
        };
        for p in self.players.values() {
            if self.team_of(p.entity.id).is_none_or(|t| t.name() != team) {
                self.send_system_message(p.entity.id, message);
            }
        }
    }
}
