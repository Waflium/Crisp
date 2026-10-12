use std::path::PathBuf;

use crisp_data::packets::play::clientbound;

use crate::commands::arguments::selector::entity_selector::java_equals_ignore_case;
use crate::network::chat::Component;
use crate::network::configuration::ChatVisibility;
use crate::network::login::Profile;
use crate::server::Server;
use crate::server::level::ServerLevel;
use crate::stats::server_stats_counter::ServerStatsCounter;
use crate::util::file_util;
use crate::world::entity::EntityId;
use crate::world::level::storage::{WorldDir, format_uuid};

impl Server {
    pub fn get_player_stats(&self, profile: &Profile) -> ServerStatsCounter {
        match &self.world_dir {
            Some(dir) => ServerStatsCounter::new(locate_stats_file(dir, profile)),
            None => ServerStatsCounter::default(),
        }
    }
}

fn locate_stats_file(dir: &WorldDir, profile: &Profile) -> PathBuf {
    let stat_folder = dir.player_stats_dir();
    let uuid_stats_file = stat_folder.join(format!("{}.json", format_uuid(profile.uuid)));
    if uuid_stats_file.exists() {
        return uuid_stats_file;
    }
    let player_name_stats_file = format!("{}.json", profile.name);
    if file_util::is_valid_path_segment(&player_name_stats_file) {
        let player_name_stats_path = stat_folder.join(&player_name_stats_file);
        if player_name_stats_path.is_file() {
            return match std::fs::rename(&player_name_stats_path, &uuid_stats_file) {
                Ok(()) => uuid_stats_file,
                Err(_) => {
                    warn!("Failed to copy file {player_name_stats_file} to {}", uuid_stats_file.display());
                    player_name_stats_path
                }
            };
        }
    }
    uuid_stats_file
}

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
