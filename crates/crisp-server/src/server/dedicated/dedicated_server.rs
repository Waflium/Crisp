use std::sync::Arc;

use crisp_data::packets::play::clientbound;

use crate::network::server_connection_listener::ListenerSettings;
use crate::server::Server;
use crate::server::level::game_mode::GameType;
use crate::server::notifications::NotificationManager;
use crate::server::notifications::server_activity_monitor::ServerActivityMonitor;
use crate::server::players::{NameAndId, OpEntry};
use crate::world::difficulty::Difficulty;
use crate::world::entity::EntityId;

impl Server {
    pub fn attach_notification_manager(
        &mut self,
        notification_manager: NotificationManager,
        server_activity_monitor: Arc<ServerActivityMonitor>,
        listener_settings: Arc<ListenerSettings>,
    ) {
        self.ops.set_notification_manager(notification_manager.clone());
        self.whitelist.set_notification_manager(notification_manager.clone());
        self.bans.set_notification_manager(notification_manager.clone());
        self.ip_bans.set_notification_manager(notification_manager.clone());
        self.notification_manager = notification_manager;
        self.server_activity_monitor = server_activity_monitor;
        self.listener_settings = listener_settings;
    }

    pub fn mark_ready(&mut self) {
        if !self.ready {
            self.ready = true;
            if let Some(management_server) = self.notification_manager.management_server() {
                management_server.set_ready();
            }
        }
    }

    pub fn is_ready(&self) -> bool {
        self.ready
    }

    fn update_property(&mut self, key: &str, value: String) {
        if let Some(settings) = &mut self.settings {
            settings.update(key, value);
        }
    }

    pub fn op(&mut self, user: NameAndId, level: Option<i32>, bypasses_player_limit: Option<bool>) {
        let level = level.unwrap_or(self.config.op_permission_level);
        let bypasses_player_limit = bypasses_player_limit.unwrap_or_else(|| self.ops.can_bypass_player_limit(user.id));
        let id = user.id;
        self.ops.add(OpEntry { user, level, bypasses_player_limit });
        if let Some(player) = self.player_by_uuid(id) {
            self.send_player_permission_level(player);
        }
    }

    pub fn deop(&mut self, user: &NameAndId) {
        if self.ops.remove(user)
            && let Some(player) = self.player_by_uuid(user.id)
        {
            self.send_player_permission_level(player);
        }
    }

    pub fn player_by_uuid(&self, uuid: u128) -> Option<EntityId> {
        self.level.players.values().find(|p| p.profile.uuid == uuid).map(|p| p.entity.id)
    }

    pub fn player_by_name(&self, name: &str) -> Option<EntityId> {
        self.level.players.values().find(|p| p.profile.name.eq_ignore_ascii_case(name)).map(|p| p.entity.id)
    }

    pub fn find_player(&self, id: Option<u128>, name: Option<&str>) -> Option<EntityId> {
        match (id, name) {
            (Some(id), _) => self.player_by_uuid(id),
            (None, Some(name)) => self.player_by_name(name),
            (None, None) => None,
        }
    }

    pub fn players_with_address(&self, ip: &str) -> Vec<EntityId> {
        self.level.players.values().filter(|p| p.ip_address() == ip).map(|p| p.entity.id).collect()
    }

    pub fn is_auto_save(&self) -> bool {
        !self.level.no_save
    }

    pub fn set_dedicated_difficulty(&mut self, difficulty: Difficulty) {
        self.config.difficulty = difficulty;
        self.update_property("difficulty", difficulty.key().to_string());
        self.set_difficulty(self.config.difficulty, true);
    }

    pub fn set_enforce_whitelist(&mut self, enforce: bool) {
        self.config.enforce_whitelist = enforce;
        self.update_property("enforce-whitelist", enforce.to_string());
    }

    pub fn set_max_players(&mut self, max_players: i32) {
        self.config.max_players = max_players as u32;
        self.update_property("max-players", max_players.to_string());
    }

    pub fn set_pause_when_empty_seconds(&mut self, seconds: i32) {
        self.config.pause_when_empty_seconds = seconds;
        self.update_property("pause-when-empty-seconds", seconds.to_string());
    }

    pub fn set_allow_flight(&mut self, allowed: bool) {
        self.config.allow_flight = allowed;
        self.level.allow_flight = allowed;
        self.update_property("allow-flight", allowed.to_string());
    }

    pub fn set_spawn_protection_radius(&mut self, radius: i32) {
        self.config.spawn_protection = radius;
        self.update_property("spawn-protection", radius.to_string());
    }

    pub fn set_motd(&mut self, motd: String) {
        self.update_property("motd", motd.clone());
        self.config.motd = motd;
    }

    pub fn set_force_game_mode(&mut self, force: bool) {
        self.config.force_game_mode = force;
        self.update_property("force-gamemode", force.to_string());
        self.enforce_game_type_for_players(self.forced_game_type());
    }

    pub fn set_game_mode(&mut self, game_mode: GameType) {
        self.config.game_mode = game_mode;
        self.update_property("gamemode", game_mode.name().to_string());
        self.default_game_type = game_mode;
        self.enforce_game_type_for_players(self.forced_game_type());
    }

    pub fn set_view_distance(&mut self, view_distance: i32) {
        self.config.view_distance = view_distance;
        self.update_property("view-distance", view_distance.to_string());
        for player in self.level.players.values() {
            player.connection.send(clientbound::SET_CHUNK_CACHE_RADIUS, |p| p.varint(view_distance));
        }
        self.level.chunks.set_view_distance(view_distance);
    }

    pub fn set_simulation_distance(&mut self, simulation_distance: i32) {
        self.config.simulation_distance = simulation_distance;
        self.update_property("simulation-distance", simulation_distance.to_string());
        for player in self.level.players.values() {
            player.connection.send(clientbound::SET_SIMULATION_DISTANCE, |p| p.varint(simulation_distance));
        }
        self.level.set_simulation_distance(simulation_distance);
    }

    pub fn accepts_transfers(&self) -> bool {
        self.listener_settings.accepts_transfers()
    }

    pub fn set_accepts_transfers(&mut self, accept: bool) {
        self.config.accepts_transfers = accept;
        self.listener_settings.set_accepts_transfers(accept);
        self.update_property("accepts-transfers", accept.to_string());
    }

    pub fn status_heartbeat_interval(&self) -> i32 {
        self.settings.as_ref().map_or(0, |s| s.properties().status_heartbeat_interval)
    }

    pub fn set_status_heartbeat_interval(&mut self, seconds: i32) -> bool {
        let Some(management_server) = self.notification_manager.management_server().cloned() else { return false };
        if !management_server.schedule_heartbeat(seconds) {
            return false;
        }
        self.update_property("status-heartbeat-interval", seconds.to_string());
        true
    }

    pub fn set_operator_user_permission_level(&mut self, level: i32) {
        self.config.op_permission_level = level;
        self.update_property("op-permission-level", level.to_string());
    }

    pub fn set_hides_online_players(&mut self, hide: bool) {
        self.config.hides_online_players = hide;
        self.update_property("hide-online-players", hide.to_string());
    }

    pub fn replies_to_status(&self) -> bool {
        self.listener_settings.replies_to_status()
    }

    pub fn set_replies_to_status(&mut self, enable: bool) {
        self.config.replies_to_status = enable;
        self.listener_settings.set_replies_to_status(enable);
        self.update_property("enable-status", enable.to_string());
    }

    pub fn set_entity_broadcast_range_percentage(&mut self, percentage: i32) {
        self.config.entity_broadcast_range_percentage = percentage;
        self.level.entity_broadcast_range_percentage = percentage;
        self.update_property("entity-broadcast-range-percentage", percentage.to_string());
    }
}
