pub mod criteria;
pub mod display_slot;
pub mod objective;
pub mod player_team;
pub mod score;
pub mod score_holder;
pub mod scoreboard;
pub mod scoreboard_save_data;
pub mod team;
pub mod team_color;
#[cfg(test)]
mod tests;

use std::cell::Ref;

use self::criteria::ObjectiveCriteria;
use self::player_team::PlayerTeam;
use self::score_holder::ScoreHolder;
use self::scoreboard::{ReadOnlyScore, ScoreAccess};
use crate::network::chat::Component;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::level::storage::format_uuid;

impl ScoreAccess {
    pub fn get(&self, level: &mut ServerLevel) -> i32 {
        level.scoreboard.borrow_mut().score_value(self)
    }

    pub fn set(&mut self, level: &mut ServerLevel, value: i32) -> Result<(), ReadOnlyScore> {
        let auto_update = level.scoreboard.borrow().objective_if_present(self.objective).is_some_and(|objective| objective.display_auto_update());
        let display = if auto_update { self.holder.display_name(level) } else { None };
        level.scoreboard.borrow_mut().set_score(self, value, display)
    }

    pub fn add(&mut self, level: &mut ServerLevel, count: i32) -> Result<i32, ReadOnlyScore> {
        let value = self.get(level).wrapping_add(count);
        self.set(level, value)?;
        Ok(value)
    }

    pub fn increment(&mut self, level: &mut ServerLevel) -> Result<i32, ReadOnlyScore> {
        self.add(level, 1)
    }

    pub fn locked(&self, level: &ServerLevel) -> bool {
        level.scoreboard.borrow().score_locked(self)
    }

    pub fn lock(&mut self, level: &mut ServerLevel) {
        level.scoreboard.borrow_mut().set_score_locked(self, true);
    }

    pub fn unlock(&mut self, level: &mut ServerLevel) {
        level.scoreboard.borrow_mut().set_score_locked(self, false);
    }

    pub fn set_display(&mut self, level: &mut ServerLevel, display: Option<Component>) {
        level.scoreboard.borrow_mut().set_score_display(self, display);
    }

    pub fn number_format_override(&mut self, level: &mut ServerLevel, number_format: Option<crate::network::chat::numbers::NumberFormat>) {
        level.scoreboard.borrow_mut().set_score_number_format(self, number_format);
    }
}

impl ServerLevel {
    pub fn scoreboard_name(&self, id: EntityId) -> String {
        match self.players.get(&id) {
            Some(p) => p.profile.name.clone(),
            None => self.entities.get(&id).map_or_else(String::new, |e| format_uuid(e.entity().uuid)),
        }
    }

    pub fn team_of(&self, id: EntityId) -> Option<Ref<'_, PlayerTeam>> {
        let scoreboard = self.scoreboard.borrow();
        if scoreboard.team_count() == 0 {
            return None;
        }
        let name = self.scoreboard_name(id);
        Ref::filter_map(scoreboard, |scoreboard| scoreboard.get_players_team(&name)).ok()
    }

    pub fn for_all_objectives(&mut self, criteria: &ObjectiveCriteria, holder: &ScoreHolder, mut operation: impl FnMut(&mut ServerLevel, &mut ScoreAccess)) {
        let objectives = self.scoreboard.borrow().objectives_by_criteria(criteria).to_vec();
        for objective in objectives {
            let mut access = self.scoreboard.borrow_mut().get_or_create_player_score(holder, objective, true);
            operation(self, &mut access);
        }
    }

    pub fn update_score_for_criteria(&mut self, player: EntityId, criteria: &ObjectiveCriteria, value: i32) {
        if self.scoreboard.borrow().objectives_by_criteria(criteria).is_empty() {
            return;
        }
        let holder = ScoreHolder::for_entity(self, player);
        self.for_all_objectives(criteria, &holder, |level, score| {
            let _ = score.set(level, value);
        });
    }

    pub fn do_teams_allow_damage(&self, id: EntityId, other: EntityId) -> bool {
        match self.team_of(id) {
            None => true,
            Some(team) => !team.is_allied_to(self.team_of(other).as_deref()) || team.is_allow_friendly_fire(),
        }
    }
}
