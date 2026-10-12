use std::path::PathBuf;

use crisp_data::packets::play::clientbound;

use crate::commands::arguments::selector::entity_selector::java_equals_ignore_case;
use crate::network::chat::Component;
use crate::network::configuration::ChatVisibility;
use crate::network::login::Profile;
use crate::server::Server;
use crate::server::level::ServerLevel;
use crate::server::player_advancements::PlayerAdvancements;
use crate::server::server_advancement_manager::ServerAdvancementManager;
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

impl Server {
    pub fn get_player_advancements(&self, uuid: u128) -> PlayerAdvancements {
        PlayerAdvancements::new(ServerAdvancementManager::get_bound(), self.world_dir.as_ref().map(|dir| dir.player_advancements(uuid)))
    }

    pub fn reload_player_advancements(&mut self) {
        let manager = ServerAdvancementManager::get_bound();
        for id in self.player_ids() {
            self.player_mut(id).advancements.reload(manager);
        }
    }

    pub fn deliver_level_broadcasts(&mut self) {
        for index in 0..self.levels.len() {
            if self.levels[index].pending_system_broadcasts.is_empty() {
                continue;
            }
            for message in std::mem::take(&mut self.levels[index].pending_system_broadcasts) {
                for &id in &self.player_list {
                    if self.level_of(id).is_some_and(|level| level != index) {
                        self.send_player_system_message(id, &message, false);
                    }
                }
            }
        }
    }
}

impl ServerLevel {
    pub fn broadcast_system_message(&mut self, message: &Component) {
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
        self.pending_system_broadcasts.push(message.clone());
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

    pub fn broadcast_system_to_all_except_team(&mut self, player: EntityId, message: &Component) {
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
