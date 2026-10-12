use std::collections::HashSet;

use crisp_data::packets::play::clientbound;
use crisp_protocol::Writer;
use indexmap::IndexMap;

use crate::network::chat::Component;
use crate::network::chat::numbers::NumberFormat;
use crate::network::connection::{Outbound, encode};
use crate::world::entity::EntityId;
use crate::world::scores::display_slot::DisplaySlot;
use crate::world::scores::objective::{Objective, ObjectiveId};
use crate::world::scores::player_team::PlayerTeam;
use crate::world::scores::score::Score;
use crate::world::scores::scoreboard::Scoreboard;
use crate::world::scores::scoreboard_save_data::ScoreboardSaveData;

pub const METHOD_ADD: u8 = 0;
pub const METHOD_REMOVE: u8 = 1;
pub const METHOD_CHANGE: u8 = 2;

#[derive(Default)]
pub struct ServerScoreboard {
    tracked_objectives: HashSet<ObjectiveId>,
    dirty: bool,
    connections: IndexMap<EntityId, Outbound>,
}

impl ServerScoreboard {
    pub fn add_connection(&mut self, player: EntityId, connection: Outbound) {
        self.connections.insert(player, connection);
    }

    pub fn remove_connection(&mut self, player: EntityId) {
        self.connections.shift_remove(&player);
    }

    fn broadcast_all(&self, packet: Vec<u8>) {
        for connection in self.connections.values() {
            connection.send_raw(packet.clone());
        }
    }
}

impl Scoreboard {
    pub(crate) fn on_objective_added(&mut self, _objective: ObjectiveId) {
        self.set_dirty();
    }

    pub(crate) fn on_objective_changed(&mut self, objective: ObjectiveId) {
        if self.server.tracked_objectives.contains(&objective) {
            self.server.broadcast_all(set_objective_packet(self.objective(objective), METHOD_CHANGE));
        }
        self.set_dirty();
    }

    pub(crate) fn on_objective_removed(&mut self, objective: ObjectiveId) {
        if self.server.tracked_objectives.contains(&objective) {
            self.stop_tracking_objective(objective);
        }
        self.set_dirty();
    }

    pub(crate) fn on_score_changed(&mut self, owner: &str, objective: ObjectiveId, score: &Score) {
        if self.server.tracked_objectives.contains(&objective) {
            let name = &self.objective(objective).name().to_string();
            self.server.broadcast_all(set_score_packet(owner, name, score.value, score.display.as_ref(), score.number_format.as_ref()));
        }
        self.set_dirty();
    }

    pub(crate) fn on_score_lock_changed(&mut self) {
        self.set_dirty();
    }

    pub(crate) fn on_player_removed(&mut self, player: &str) {
        if !self.server.tracked_objectives.is_empty() {
            self.server.broadcast_all(reset_score_packet(player, None));
        }
        self.set_dirty();
    }

    pub(crate) fn on_player_score_removed(&mut self, player: &str, objective: ObjectiveId) {
        if self.server.tracked_objectives.contains(&objective) {
            self.server.broadcast_all(reset_score_packet(player, Some(self.objective(objective).name())));
        }
        self.set_dirty();
    }

    pub fn set_display_objective(&mut self, slot: DisplaySlot, objective: Option<ObjectiveId>) {
        let old = self.get_display_objective(slot);
        self.put_display_objective(slot, objective);
        if let Some(old) = old
            && Some(old) != objective
        {
            if self.objective_display_slot_count(old) > 0 {
                self.server.broadcast_all(set_display_objective_packet(slot, objective.map(|o| self.objective(o))));
            } else {
                self.stop_tracking_objective(old);
            }
        }
        if let Some(objective) = objective {
            if self.server.tracked_objectives.contains(&objective) {
                self.server.broadcast_all(set_display_objective_packet(slot, Some(self.objective(objective))));
            } else {
                self.start_tracking_objective(objective);
            }
        }
        self.set_dirty();
    }

    pub fn add_player_to_team(&mut self, player: &str, team: &str) -> bool {
        if self.add_player_to_team_base(player, team) {
            self.server.broadcast_all(set_player_team_player_packet(team, player, true));
            self.set_dirty();
            true
        } else {
            false
        }
    }

    pub fn remove_player_from_team(&mut self, player: &str, team: &str) {
        self.remove_player_from_team_base(player, team);
        self.server.broadcast_all(set_player_team_player_packet(team, player, false));
        self.set_dirty();
    }

    pub(crate) fn on_team_added(&mut self, team: &str) {
        let packet = set_player_team_add_or_modify_packet(self.get_player_team(team).unwrap(), true);
        self.server.broadcast_all(packet);
        self.set_dirty();
    }

    pub(crate) fn on_team_changed(&mut self, team: &str) {
        let packet = set_player_team_add_or_modify_packet(self.get_player_team(team).unwrap(), false);
        self.server.broadcast_all(packet);
        self.set_dirty();
    }

    pub(crate) fn on_team_removed(&mut self, team: &PlayerTeam) {
        self.server.broadcast_all(set_player_team_remove_packet(team));
        self.set_dirty();
    }

    fn set_dirty(&mut self) {
        self.server.dirty = true;
    }

    pub fn store_to_save_data_if_dirty(&mut self, save_data: &mut ScoreboardSaveData) {
        if self.server.dirty {
            self.server.dirty = false;
            save_data.set_data(self.store());
        }
    }

    pub fn start_tracking_packets(&self, objective: ObjectiveId) -> Vec<Vec<u8>> {
        let o = self.objective(objective);
        let mut packets = vec![set_objective_packet(o, METHOD_ADD)];
        for slot in DisplaySlot::VALUES {
            if self.get_display_objective(slot) == Some(objective) {
                packets.push(set_display_objective_packet(slot, Some(o)));
            }
        }
        for entry in self.list_player_scores(objective) {
            packets.push(set_score_packet(entry.owner, o.name(), entry.score.value, entry.score.display.as_ref(), entry.score.number_format.as_ref()));
        }
        packets
    }

    pub fn start_tracking_objective(&mut self, objective: ObjectiveId) {
        let packets = self.start_tracking_packets(objective);
        for connection in self.server.connections.values() {
            for packet in &packets {
                connection.send_raw(packet.clone());
            }
        }
        self.server.tracked_objectives.insert(objective);
    }

    fn stop_tracking_packets(&self, objective: ObjectiveId) -> Vec<Vec<u8>> {
        let o = self.objective(objective);
        let mut packets = vec![set_objective_packet(o, METHOD_REMOVE)];
        for slot in DisplaySlot::VALUES {
            if self.get_display_objective(slot) == Some(objective) {
                packets.push(set_display_objective_packet(slot, Some(o)));
            }
        }
        packets
    }

    pub fn stop_tracking_objective(&mut self, objective: ObjectiveId) {
        let packets = self.stop_tracking_packets(objective);
        for connection in self.server.connections.values() {
            for packet in &packets {
                connection.send_raw(packet.clone());
            }
        }
        self.server.tracked_objectives.remove(&objective);
    }

    pub fn objective_display_slot_count(&self, objective: ObjectiveId) -> usize {
        DisplaySlot::VALUES.iter().filter(|&&slot| self.get_display_objective(slot) == Some(objective)).count()
    }

    pub fn update_entire_scoreboard(&self, connection: &Outbound) {
        for team in self.player_teams() {
            connection.send_raw(set_player_team_add_or_modify_packet(team, true));
        }
        let mut sent: Vec<ObjectiveId> = Vec::new();
        for slot in DisplaySlot::VALUES {
            if let Some(objective) = self.get_display_objective(slot)
                && !sent.contains(&objective)
            {
                for packet in self.start_tracking_packets(objective) {
                    connection.send_raw(packet);
                }
                sent.push(objective);
            }
        }
    }
}

pub fn set_objective_packet(objective: &Objective, method: u8) -> Vec<u8> {
    encode(clientbound::SET_OBJECTIVE, |w| {
        w.string(objective.name());
        w.u8(method);
        if method == METHOD_ADD || method == METHOD_CHANGE {
            objective.display_name().write(w);
            w.varint(objective.render_type() as i32);
            NumberFormat::write_optional(objective.number_format(), w);
        }
    })
}

pub fn set_display_objective_packet(slot: DisplaySlot, objective: Option<&Objective>) -> Vec<u8> {
    encode(clientbound::SET_DISPLAY_OBJECTIVE, |w| {
        w.varint(slot.id());
        w.string(objective.map_or("", Objective::name));
    })
}

pub fn set_score_packet(owner: &str, objective: &str, score: i32, display: Option<&Component>, number_format: Option<&NumberFormat>) -> Vec<u8> {
    encode(clientbound::SET_SCORE, |w| {
        w.string(owner);
        w.string(objective);
        w.varint(score);
        w.bool(display.is_some());
        if let Some(display) = display {
            display.write(w);
        }
        NumberFormat::write_optional(number_format, w);
    })
}

pub fn reset_score_packet(owner: &str, objective: Option<&str>) -> Vec<u8> {
    encode(clientbound::RESET_SCORE, |w| {
        w.string(owner);
        w.bool(objective.is_some());
        if let Some(objective) = objective {
            w.string(objective);
        }
    })
}

fn write_team_parameters(team: &PlayerTeam, w: &mut Writer) {
    team.display_name().write(w);
    team.player_prefix().write(w);
    team.player_suffix().write(w);
    w.varint(team.name_tag_visibility().id());
    w.varint(team.collision_rule().id());
    w.bool(team.color().is_some());
    if let Some(color) = team.color() {
        w.varint(color.id());
    }
    w.u8(team.pack_options());
}

pub fn set_player_team_add_or_modify_packet(team: &PlayerTeam, create_new: bool) -> Vec<u8> {
    encode(clientbound::SET_PLAYER_TEAM, |w| {
        w.string(team.name());
        w.u8(if create_new { 0 } else { 2 });
        write_team_parameters(team, w);
        if create_new {
            w.varint(team.player_count() as i32);
            team.players().for_each(|p| w.string(p));
        }
    })
}

pub fn set_player_team_remove_packet(team: &PlayerTeam) -> Vec<u8> {
    encode(clientbound::SET_PLAYER_TEAM, |w| {
        w.string(team.name());
        w.u8(1);
    })
}

pub fn set_player_team_player_packet(team: &str, player: &str, add: bool) -> Vec<u8> {
    encode(clientbound::SET_PLAYER_TEAM, |w| {
        w.string(team);
        w.u8(if add { 3 } else { 4 });
        w.varint(1);
        w.string(player);
    })
}

#[cfg(test)]
mod tests {
    use crisp_data::packets::play::clientbound;
    use crisp_protocol::Reader;

    use super::*;
    use crate::network::configuration::CommonListenerCookie;
    use crate::network::login::Profile;
    use crate::server::Server;
    use crate::world::scores::criteria::{ObjectiveCriteria, RenderType};
    use crate::world::scores::score_holder::ScoreHolder;

    fn packets(rx: &mut tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>) -> Vec<Vec<u8>> {
        std::iter::from_fn(|| rx.try_recv().ok()).collect()
    }

    fn id(packet: &[u8]) -> i32 {
        Reader::new(packet).varint().unwrap()
    }

    #[test]
    fn joining_player_gets_teams_then_shown_objectives() {
        let mut server = Server::new(crate::ServerConfig { level_type: crate::LevelType::Flat, ..crate::ServerConfig::default() }).unwrap();
        let level = &mut server.level;
        let foo = level.scoreboard.borrow_mut().add_objective("foo", ObjectiveCriteria::DUMMY, Component::literal("Foo"), RenderType::Integer, false, None);
        let hidden = level.scoreboard.borrow_mut().add_objective("hidden", ObjectiveCriteria::DUMMY, Component::literal("h"), RenderType::Integer, false, None);
        for (holder, objective) in [("a", foo), ("a", hidden), ("b", foo)] {
            let mut score = level.scoreboard.borrow_mut().get_or_create_player_score(&ScoreHolder::for_name_only(holder), objective, false);
            score.set(level, 3).unwrap();
        }
        level.scoreboard.borrow_mut().add_player_team("red");
        level.scoreboard.borrow_mut().add_player_to_team("Alice", "red");
        level.scoreboard.borrow_mut().set_display_objective(DisplaySlot::List, Some(foo));
        level.scoreboard.borrow_mut().set_display_objective(DisplaySlot::Sidebar, Some(foo));

        let (out, mut rx) = Outbound::capture();
        let profile = Profile { uuid: 7, name: "Alice".into(), properties: Vec::new() };
        server.place_new_player(1, profile, CommonListenerCookie::create_initial(false), out, "127.0.0.1:25565".parse().unwrap());
        let sent = packets(&mut rx);
        let commands = sent.iter().position(|p| id(p) == clientbound::COMMANDS).unwrap();
        assert_eq!(id(&sent[commands + 1]), clientbound::RECIPE_BOOK_SETTINGS);
        assert_eq!(id(&sent[commands + 2]), clientbound::RECIPE_BOOK_ADD);
        let commands = commands + 2;
        {
            let scoreboard = server.scoreboard.borrow();
            let objective = scoreboard.objective(foo);
            let mut expected = vec![set_player_team_add_or_modify_packet(scoreboard.get_player_team("red").unwrap(), true), set_objective_packet(objective, METHOD_ADD)];
            expected.push(set_display_objective_packet(DisplaySlot::List, Some(objective)));
            expected.push(set_display_objective_packet(DisplaySlot::Sidebar, Some(objective)));
            for entry in scoreboard.list_player_scores(foo) {
                expected.push(set_score_packet(entry.owner, "foo", 3, None, None));
            }
            assert_eq!(sent[commands + 1..commands + 1 + expected.len()], expected[..]);
            assert_eq!(id(&sent[commands + 1 + expected.len()]), clientbound::PLAYER_POSITION);
        }

        let player = server.connections[&1];
        let level = &mut server.level;
        let mut score = level.scoreboard.borrow_mut().get_or_create_player_score(&ScoreHolder::for_entity(level, player), hidden, false);
        score.set(level, 1).unwrap();
        assert_eq!(packets(&mut rx), Vec::<Vec<u8>>::new(), "hidden objectives are not tracked");
        level.scoreboard.borrow_mut().set_display_objective(DisplaySlot::Sidebar, Some(hidden));
        {
            let scoreboard = level.scoreboard.borrow();
            let mut expected = vec![set_display_objective_packet(DisplaySlot::Sidebar, Some(scoreboard.objective(hidden)))];
            expected.extend(scoreboard.start_tracking_packets(hidden));
            assert_eq!(packets(&mut rx), expected);
        }
        level.scoreboard.borrow_mut().remove_objective(hidden);
        let ids: Vec<i32> = packets(&mut rx).iter().map(|p| id(p)).collect();
        assert_eq!(ids, [clientbound::SET_OBJECTIVE]);
        level.scoreboard.borrow_mut().reset_all_player_scores("b");
        assert_eq!(packets(&mut rx), [reset_score_packet("b", None)]);
    }
}
