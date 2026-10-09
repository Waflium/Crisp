use std::collections::HashMap;

use crisp_core::fastutil::Object2ObjectOpenHashMap;
use crisp_protocol::nbt::Nbt;

use super::criteria::{ObjectiveCriteria, RenderType};
use super::display_slot::DisplaySlot;
use super::objective::{Objective, ObjectiveId, PackedObjective};
use super::player_team::{PackedTeam, PlayerTeam};
use super::score::{PlayerScores, Score};
use super::score_holder::ScoreHolder;
use super::team::{CollisionRule, Visibility};
use super::team_color::TeamColor;
use crate::network::chat::Component;
use crate::network::chat::numbers::NumberFormat;
use crate::server::server_scoreboard::ServerScoreboard;

pub struct PlayerScoreEntry<'a> {
    pub owner: &'a str,
    pub score: &'a Score,
}

impl PlayerScoreEntry<'_> {
    pub fn is_hidden(&self) -> bool {
        self.owner.starts_with('#')
    }
}

pub struct Scoreboard {
    objectives: Vec<Option<Objective>>,
    objectives_by_name: Object2ObjectOpenHashMap<ObjectiveId>,
    objectives_by_criteria: HashMap<ObjectiveCriteria, Vec<ObjectiveId>>,
    player_scores: Object2ObjectOpenHashMap<PlayerScores>,
    display_objectives: [Option<ObjectiveId>; 19],
    teams_by_name: Object2ObjectOpenHashMap<PlayerTeam>,
    teams_by_player: HashMap<String, String>,
    pub server: ServerScoreboard,
}

impl Default for Scoreboard {
    fn default() -> Scoreboard {
        Scoreboard {
            objectives: Vec::new(),
            objectives_by_name: Object2ObjectOpenHashMap::new(16, 0.5),
            objectives_by_criteria: HashMap::new(),
            player_scores: Object2ObjectOpenHashMap::new(16, 0.5),
            display_objectives: [None; 19],
            teams_by_name: Object2ObjectOpenHashMap::new(16, 0.75),
            teams_by_player: HashMap::new(),
            server: ServerScoreboard::default(),
        }
    }
}

pub struct ScoreAccess {
    pub holder: ScoreHolder,
    pub objective: ObjectiveId,
    can_modify: bool,
    requires_sync: bool,
}

impl Scoreboard {
    pub fn get_objective(&self, name: &str) -> Option<&Objective> {
        self.objectives_by_name.get(name).map(|&id| self.objective(id))
    }

    pub fn objective(&self, id: ObjectiveId) -> &Objective {
        self.objectives[id as usize].as_ref().expect("a removed objective")
    }

    pub fn objective_if_present(&self, id: ObjectiveId) -> Option<&Objective> {
        self.objectives.get(id as usize).and_then(Option::as_ref)
    }

    fn objective_mut(&mut self, id: ObjectiveId) -> &mut Objective {
        self.objectives[id as usize].as_mut().expect("a removed objective")
    }

    pub fn add_objective(
        &mut self,
        name: &str,
        criteria: ObjectiveCriteria,
        display_name: Component,
        render_type: RenderType,
        display_auto_update: bool,
        number_format: Option<NumberFormat>,
    ) -> ObjectiveId {
        assert!(!self.objectives_by_name.contains_key(name), "An objective with the name '{name}' already exists!");
        let id = self.objectives.len() as ObjectiveId;
        self.objectives_by_criteria.entry(criteria.clone()).or_default().push(id);
        self.objectives.push(Some(Objective::new(id, name, criteria, display_name, render_type, display_auto_update, number_format)));
        self.objectives_by_name.insert(name.to_string(), id);
        self.on_objective_added(id);
        id
    }

    pub fn objectives_by_criteria(&self, criteria: &ObjectiveCriteria) -> &[ObjectiveId] {
        self.objectives_by_criteria.get(criteria).map_or(&[], Vec::as_slice)
    }

    pub fn get_or_create_player_score(&mut self, holder: &ScoreHolder, objective: ObjectiveId, force_writable: bool) -> ScoreAccess {
        let can_modify = force_writable || !self.objective(objective).criteria.is_read_only();
        let scores = self.player_scores.get_or_insert_with(holder.scoreboard_name(), PlayerScores::default);
        let (_, new) = scores.get_or_create(objective);
        ScoreAccess { holder: holder.clone(), objective, can_modify, requires_sync: new }
    }

    pub fn get_player_score_info(&self, name: &str, objective: ObjectiveId) -> Option<&Score> {
        self.player_scores.get(name)?.get(objective)
    }

    fn score_mut(&mut self, name: &str, objective: ObjectiveId) -> &mut Score {
        self.player_scores.get_or_insert_with(name, PlayerScores::default).get_or_create(objective).0
    }

    pub fn list_player_scores(&self, objective: ObjectiveId) -> Vec<PlayerScoreEntry<'_>> {
        self.player_scores.iter().filter_map(|(owner, scores)| scores.get(objective).map(|score| PlayerScoreEntry { owner, score })).collect()
    }

    pub fn objectives(&self) -> impl Iterator<Item = &Objective> {
        self.objectives_by_name.iter().map(|(_, &id)| self.objective(id))
    }

    pub fn objective_count(&self) -> usize {
        self.objectives_by_name.len()
    }

    pub fn tracked_players(&self) -> Vec<ScoreHolder> {
        self.player_scores.stream().map(|(name, _)| ScoreHolder::for_name_only(name)).collect()
    }

    pub fn reset_all_player_scores(&mut self, name: &str) {
        if self.player_scores.remove(name).is_some() {
            self.on_player_removed(name);
        }
    }

    pub fn reset_single_player_score(&mut self, name: &str, objective: ObjectiveId) {
        let Some(scores) = self.player_scores.get_mut(name) else { return };
        let removed = scores.remove(objective);
        if !scores.has_scores() {
            if self.player_scores.remove(name).is_some() {
                self.on_player_removed(name);
            }
        } else if removed {
            self.on_player_score_removed(name, objective);
        }
    }

    pub fn list_player_scores_of(&self, name: &str) -> Vec<(ObjectiveId, i32)> {
        self.player_scores.get(name).map(|s| s.iter().map(|(o, score)| (*o, score.value)).collect()).unwrap_or_default()
    }

    pub fn remove_objective(&mut self, id: ObjectiveId) {
        let name = self.objective(id).name.clone();
        self.objectives_by_name.remove(&name);
        for slot in DisplaySlot::VALUES {
            if self.get_display_objective(slot) == Some(id) {
                self.set_display_objective(slot, None);
            }
        }
        let criteria = self.objective(id).criteria.clone();
        if let Some(list) = self.objectives_by_criteria.get_mut(&criteria) {
            list.retain(|&o| o != id);
        }
        for scores in self.player_scores.values_mut() {
            scores.remove(id);
        }
        self.on_objective_removed(id);
        self.objectives[id as usize] = None;
    }

    pub(crate) fn put_display_objective(&mut self, slot: DisplaySlot, objective: Option<ObjectiveId>) {
        self.display_objectives[slot as usize] = objective;
    }

    pub fn get_display_objective(&self, slot: DisplaySlot) -> Option<ObjectiveId> {
        self.display_objectives[slot as usize]
    }

    pub fn set_objective_display_name(&mut self, id: ObjectiveId, name: Component) {
        self.objective_mut(id).set_display_name(name);
        self.on_objective_changed(id);
    }

    pub fn set_render_type(&mut self, id: ObjectiveId, render_type: RenderType) {
        self.objective_mut(id).render_type = render_type;
        self.on_objective_changed(id);
    }

    pub fn set_display_auto_update(&mut self, id: ObjectiveId, display_auto_update: bool) {
        self.objective_mut(id).display_auto_update = display_auto_update;
        self.on_objective_changed(id);
    }

    pub fn set_number_format(&mut self, id: ObjectiveId, number_format: Option<NumberFormat>) {
        self.objective_mut(id).number_format = number_format;
        self.on_objective_changed(id);
    }

    pub fn get_player_team(&self, name: &str) -> Option<&PlayerTeam> {
        self.teams_by_name.get(name)
    }

    fn team_mut(&mut self, name: &str) -> &mut PlayerTeam {
        self.teams_by_name.get_mut(name).expect("a team")
    }

    pub fn add_player_team(&mut self, name: &str) {
        if self.teams_by_name.contains_key(name) {
            warn!("Requested creation of existing team '{name}'");
            return;
        }
        self.teams_by_name.insert(name.to_string(), PlayerTeam::new(name));
        self.on_team_added(name);
    }

    pub fn remove_player_team(&mut self, name: &str) {
        let Some(team) = self.teams_by_name.remove(name) else { return };
        for player in team.players() {
            self.teams_by_player.remove(player);
        }
        self.on_team_removed(&team);
    }

    pub(crate) fn add_player_to_team_base(&mut self, player: &str, team: &str) -> bool {
        if self.get_players_team(player).is_some() {
            self.remove_player_from_team_any(player);
        }
        self.teams_by_player.insert(player.to_string(), team.to_string());
        self.team_mut(team).players.insert(player.to_string())
    }

    pub fn remove_player_from_team_any(&mut self, player: &str) -> bool {
        match self.teams_by_player.get(player).cloned() {
            Some(team) => {
                self.remove_player_from_team(player, &team);
                true
            }
            None => false,
        }
    }

    pub(crate) fn remove_player_from_team_base(&mut self, player: &str, team: &str) {
        assert!(
            self.teams_by_player.get(player).is_some_and(|t| t == team),
            "Player is either on another team or not on any team. Cannot remove from team '{team}'."
        );
        self.teams_by_player.remove(player);
        self.team_mut(team).players.remove(player);
    }

    pub fn player_teams(&self) -> impl Iterator<Item = &PlayerTeam> {
        self.teams_by_name.iter().map(|(_, team)| team)
    }

    pub fn team_count(&self) -> usize {
        self.teams_by_name.len()
    }

    pub fn get_players_team(&self, player: &str) -> Option<&PlayerTeam> {
        self.teams_by_name.get(self.teams_by_player.get(player)?)
    }

    pub fn set_team_display_name(&mut self, team: &str, display_name: Component) {
        self.team_mut(team).display_name = display_name;
        self.on_team_changed(team);
    }

    pub fn set_player_prefix(&mut self, team: &str, prefix: Component) {
        self.team_mut(team).player_prefix = prefix;
        self.on_team_changed(team);
    }

    pub fn set_player_suffix(&mut self, team: &str, suffix: Component) {
        self.team_mut(team).player_suffix = suffix;
        self.on_team_changed(team);
    }

    pub fn set_allow_friendly_fire(&mut self, team: &str, allow: bool) {
        self.team_mut(team).allow_friendly_fire = allow;
        self.on_team_changed(team);
    }

    pub fn set_see_friendly_invisibles(&mut self, team: &str, see: bool) {
        self.team_mut(team).see_friendly_invisibles = see;
        self.on_team_changed(team);
    }

    pub fn set_name_tag_visibility(&mut self, team: &str, visibility: Visibility) {
        self.team_mut(team).name_tag_visibility = visibility;
        self.on_team_changed(team);
    }

    pub fn set_death_message_visibility(&mut self, team: &str, visibility: Visibility) {
        self.team_mut(team).death_message_visibility = visibility;
        self.on_team_changed(team);
    }

    pub fn set_color(&mut self, team: &str, color: Option<TeamColor>) {
        self.team_mut(team).color = color;
        self.on_team_changed(team);
    }

    pub fn set_collision_rule(&mut self, team: &str, rule: CollisionRule) {
        self.team_mut(team).collision_rule = rule;
        self.on_team_changed(team);
    }

    pub fn entity_removed(&mut self, name: &str) {
        self.reset_all_player_scores(name);
        self.remove_player_from_team_any(name);
    }

    pub(crate) fn set_score(&mut self, access: &mut ScoreAccess, value: i32, display: Option<Component>) -> Result<(), ReadOnlyScore> {
        if self.objective_if_present(access.objective).is_none() {
            return Ok(());
        }
        if !access.can_modify {
            return Err(ReadOnlyScore);
        }
        let mut changed = access.requires_sync;
        let score = self.score_mut(access.holder.scoreboard_name(), access.objective);
        if let Some(display) = display
            && score.display.as_ref() != Some(&display)
        {
            score.display = Some(display);
            changed = true;
        }
        if value != score.value {
            score.value = value;
            changed = true;
        }
        if changed {
            self.send_score_to_players(access);
        }
        Ok(())
    }

    pub fn score_value(&mut self, access: &ScoreAccess) -> i32 {
        self.player_scores.get(access.holder.scoreboard_name()).and_then(|s| s.get(access.objective)).map_or(0, |s| s.value)
    }

    pub fn score_locked(&self, access: &ScoreAccess) -> bool {
        self.player_scores.get(access.holder.scoreboard_name()).and_then(|s| s.get(access.objective)).is_none_or(|s| s.locked)
    }

    pub fn set_score_display(&mut self, access: &mut ScoreAccess, display: Option<Component>) {
        if self.objective_if_present(access.objective).is_none() {
            return;
        }
        let requires_sync = access.requires_sync;
        let score = self.score_mut(access.holder.scoreboard_name(), access.objective);
        if requires_sync || score.display != display {
            score.display = display;
            self.send_score_to_players(access);
        }
    }

    pub fn set_score_number_format(&mut self, access: &mut ScoreAccess, number_format: Option<NumberFormat>) {
        if self.objective_if_present(access.objective).is_none() {
            return;
        }
        self.score_mut(access.holder.scoreboard_name(), access.objective).number_format = number_format;
        self.send_score_to_players(access);
    }

    pub fn set_score_locked(&mut self, access: &mut ScoreAccess, locked: bool) {
        if self.objective_if_present(access.objective).is_none() {
            return;
        }
        self.score_mut(access.holder.scoreboard_name(), access.objective).locked = locked;
        if access.requires_sync {
            self.send_score_to_players(access);
        }
        self.on_score_lock_changed();
    }

    fn send_score_to_players(&mut self, access: &mut ScoreAccess) {
        let score = self.score_mut(access.holder.scoreboard_name(), access.objective).clone();
        self.on_score_changed(access.holder.scoreboard_name(), access.objective, &score);
        access.requires_sync = false;
    }

    pub fn store(&self) -> Nbt {
        let mut fields = Vec::new();
        let objectives: Vec<Nbt> = self.objectives_by_name.stream().map(|(_, &id)| self.objective(id).pack()).collect();
        if !objectives.is_empty() {
            fields.push(("Objectives".to_string(), Nbt::List(objectives)));
        }
        let mut scores = Vec::new();
        for (owner, player) in self.player_scores.stream() {
            for (objective, score) in player.iter() {
                let mut entry = vec![("Name".to_string(), Nbt::String(owner.to_string())), ("Objective".into(), Nbt::String(self.objective(*objective).name.clone()))];
                score.pack(&mut entry);
                scores.push(Nbt::Compound(entry));
            }
        }
        if !scores.is_empty() {
            fields.push(("PlayerScores".into(), Nbt::List(scores)));
        }
        let slots: Vec<(String, Nbt)> = DisplaySlot::VALUES
            .iter()
            .filter_map(|&slot| self.get_display_objective(slot).map(|id| (slot.serialized_name().to_string(), Nbt::String(self.objective(id).name.clone()))))
            .collect();
        if !slots.is_empty() {
            fields.push(("DisplaySlots".into(), Nbt::Compound(slots)));
        }
        let teams: Vec<Nbt> = self.teams_by_name.stream().map(|(_, team)| team.pack()).collect();
        if !teams.is_empty() {
            fields.push(("Teams".into(), Nbt::List(teams)));
        }
        Nbt::Compound(fields)
    }

    pub fn load(&mut self, data: &Nbt) {
        for objective in data.get("Objectives").map_or(&[][..], Nbt::as_list) {
            match PackedObjective::from_nbt(objective) {
                Ok(o) => {
                    if self.objectives_by_name.contains_key(&o.name) {
                        error!("An objective with the name '{}' already exists!", o.name);
                        continue;
                    }
                    self.add_objective(&o.name, o.criteria, o.display_name, o.render_type, o.display_auto_update, o.number_format);
                }
                Err(e) => error!("Failed to load objective: {e}"),
            }
        }
        for score in data.get("PlayerScores").map_or(&[][..], Nbt::as_list) {
            let (Some(owner), Some(objective)) = (score.get("Name").and_then(Nbt::as_str), score.get("Objective").and_then(Nbt::as_str)) else { continue };
            let Some(&id) = self.objectives_by_name.get(objective) else {
                error!("Unknown objective {objective} for name {owner}, ignoring");
                continue;
            };
            match Score::unpack(score) {
                Ok(s) => self.player_scores.get_or_insert_with(owner, PlayerScores::default).set_score(id, s),
                Err(e) => error!("Failed to load score: {e}"),
            }
        }
        if let Some(Nbt::Compound(slots)) = data.get("DisplaySlots") {
            for (slot, name) in slots {
                let Some(slot) = DisplaySlot::by_name(slot) else { continue };
                let objective = name.as_str().and_then(|n| self.objectives_by_name.get(n).copied());
                self.set_display_objective(slot, objective);
            }
        }
        for team in data.get("Teams").map_or(&[][..], Nbt::as_list) {
            match PackedTeam::from_nbt(team) {
                Ok(packed) => self.load_player_team(packed),
                Err(e) => error!("Failed to load team: {e}"),
            }
        }
    }

    fn load_player_team(&mut self, packed: PackedTeam) {
        let name = packed.name.as_str();
        self.add_player_team(name);
        if let Some(display_name) = packed.display_name {
            self.set_team_display_name(name, display_name);
        }
        self.set_color(name, packed.color);
        self.set_allow_friendly_fire(name, packed.allow_friendly_fire);
        self.set_see_friendly_invisibles(name, packed.see_friendly_invisibles);
        self.set_player_prefix(name, packed.member_name_prefix);
        self.set_player_suffix(name, packed.member_name_suffix);
        self.set_name_tag_visibility(name, packed.name_tag_visibility);
        self.set_death_message_visibility(name, packed.death_message_visibility);
        self.set_collision_rule(name, packed.collision_rule);
        for player in &packed.players {
            self.add_player_to_team(player, name);
        }
    }
}

#[derive(Debug)]
pub struct ReadOnlyScore;
