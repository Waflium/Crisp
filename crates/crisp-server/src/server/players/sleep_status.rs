use crate::network::chat::{Arg, Component};
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;

#[derive(Default)]
pub struct SleepStatus {
    active_players: i32,
    sleeping_players: i32,
}

impl SleepStatus {
    pub fn are_enough_sleeping(&self, sleep_percentage_needed: i32) -> bool {
        self.sleeping_players >= self.sleepers_needed(sleep_percentage_needed)
    }

    pub fn sleepers_needed(&self, sleep_percentage_needed: i32) -> i32 {
        1.max(crisp_core::mth::ceil(((self.active_players * sleep_percentage_needed) as f32 / 100.0) as f64))
    }

    pub fn remove_all_sleepers(&mut self) {
        self.sleeping_players = 0;
    }

    pub fn amount_sleeping(&self) -> i32 {
        self.sleeping_players
    }
}

impl ServerLevel {
    pub fn can_sleep_through_nights(&self) -> bool {
        self.game_rules.get_int(crisp_data::game_rule::PLAYERS_SLEEPING_PERCENTAGE) <= 100
    }

    fn announce_sleep_status(&self) {
        if !self.can_sleep_through_nights() {
            return;
        }
        let percentage = self.game_rules.get_int(crisp_data::game_rule::PLAYERS_SLEEPING_PERCENTAGE);
        let message = if self.sleep_status.are_enough_sleeping(percentage) {
            Component::translatable("sleep.skipping_night")
        } else {
            Component::translatable_with(
                "sleep.players_sleeping",
                vec![Arg::Int(self.sleep_status.amount_sleeping()), Arg::Int(self.sleep_status.sleepers_needed(percentage))],
            )
        };
        for &id in self.players.keys() {
            self.send_overlay_message(id, &message);
        }
    }

    pub fn update_sleeping_player_list(&mut self) {
        if !self.players.is_empty() && self.update_sleep_status() {
            self.announce_sleep_status();
        }
    }

    pub fn should_wake_up_all_players(&self) -> bool {
        let percentage = self.game_rules.get_int(crisp_data::game_rule::PLAYERS_SLEEPING_PERCENTAGE);
        self.sleep_status.are_enough_sleeping(percentage) && self.are_enough_deep_sleeping(percentage)
    }

    pub fn wake_up_all_players(&mut self) {
        self.sleep_status.remove_all_sleepers();
        let sleeping: Vec<EntityId> = self.players.keys().copied().filter(|&id| self.is_sleeping(id)).collect();
        for id in sleeping {
            self.stop_sleep_in_bed(id, false, false);
        }
    }

    pub fn are_enough_deep_sleeping(&self, sleep_percentage_needed: i32) -> bool {
        let deep_sleepers = self.players.keys().filter(|&&id| self.is_sleeping_long_enough(id)).count() as i32;
        deep_sleepers >= self.sleep_status.sleepers_needed(sleep_percentage_needed)
    }

    pub fn update_sleep_status(&mut self) -> bool {
        let old_active_players = self.sleep_status.active_players;
        let old_sleeping_players = self.sleep_status.sleeping_players;
        let (mut active_players, mut sleeping_players) = (0, 0);
        let ids: Vec<EntityId> = self.players.keys().copied().collect();
        for id in ids {
            if !self.is_spectator(id) {
                active_players += 1;
                if self.is_sleeping(id) {
                    sleeping_players += 1;
                }
            }
        }
        self.sleep_status.active_players = active_players;
        self.sleep_status.sleeping_players = sleeping_players;
        (old_sleeping_players > 0 || sleeping_players > 0) && (old_active_players != active_players || old_sleeping_players != sleeping_players)
    }
}
