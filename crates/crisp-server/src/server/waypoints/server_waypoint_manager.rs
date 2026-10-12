use crisp_data::game_rule;

use crate::server::level::ServerLevel;
use crate::util::FastIndexMap;
use crate::util::java_collections::JavaObjectHashSet;
use crate::world::entity::EntityId;
use crate::world::waypoints::waypoint_transmitter::Connection;

#[derive(Default)]
struct Table {
    backing_map: FastIndexMap<EntityId, FastIndexMap<EntityId, Connection>>,
}

impl Table {
    fn contains(&self, player: EntityId, waypoint: EntityId) -> bool {
        self.backing_map.get(&player).is_some_and(|row| row.contains_key(&waypoint))
    }

    fn get_mut(&mut self, player: EntityId, waypoint: EntityId) -> Option<&mut Connection> {
        self.backing_map.get_mut(&player)?.get_mut(&waypoint)
    }

    fn put(&mut self, player: EntityId, waypoint: EntityId, connection: Connection) {
        self.backing_map.entry(player).or_default().insert(waypoint, connection);
    }

    fn remove(&mut self, player: EntityId, waypoint: EntityId) -> Option<Connection> {
        let row = self.backing_map.get_mut(&player)?;
        let connection = row.shift_remove(&waypoint);
        if row.is_empty() {
            self.backing_map.shift_remove(&player);
        }
        connection
    }

    fn row_keys(&self, player: EntityId) -> Vec<EntityId> {
        self.backing_map.get(&player).map_or_else(Vec::new, |row| row.keys().copied().collect())
    }

    fn column_keys(&self, waypoint: EntityId) -> Vec<EntityId> {
        self.backing_map.iter().filter(|(_, row)| row.contains_key(&waypoint)).map(|(&player, _)| player).collect()
    }
}

#[derive(Default)]
pub struct ServerWaypointManager {
    waypoints: JavaObjectHashSet<EntityId>,
    players: JavaObjectHashSet<EntityId>,
    connections: Table,
}

impl ServerWaypointManager {
    pub fn track_waypoint(&mut self, level: &ServerLevel, waypoint: EntityId) {
        self.waypoints.insert(waypoint);
        for player in self.players.iter() {
            create_connection(&mut self.connections, level, player, waypoint);
        }
    }

    pub fn update_waypoint(&mut self, level: &ServerLevel, waypoint: EntityId) {
        if self.waypoints.contains(&waypoint) {
            for player in self.connections.column_keys(waypoint) {
                update_connection(&mut self.connections, level, player, waypoint);
            }
            for player in self.players.iter() {
                if !self.connections.contains(player, waypoint) {
                    create_connection(&mut self.connections, level, player, waypoint);
                }
            }
        }
    }

    pub fn untrack_waypoint(&mut self, level: &ServerLevel, waypoint: EntityId) {
        for player in self.connections.column_keys(waypoint) {
            if let Some(connection) = self.connections.remove(player, waypoint) {
                connection.disconnect(level);
            }
        }
        self.waypoints.remove(&waypoint);
    }

    pub fn add_player(&mut self, level: &ServerLevel, player: EntityId) {
        self.players.insert(player);
        for waypoint in self.waypoints.iter() {
            create_connection(&mut self.connections, level, player, waypoint);
        }
        if level.is_transmitting_waypoint(player) {
            self.track_waypoint(level, player);
        }
    }

    pub fn update_player(&mut self, level: &ServerLevel, player: EntityId) {
        for waypoint in self.connections.row_keys(player) {
            update_connection(&mut self.connections, level, player, waypoint);
        }
        for waypoint in self.waypoints.iter() {
            if !self.connections.contains(player, waypoint) {
                create_connection(&mut self.connections, level, player, waypoint);
            }
        }
    }

    pub fn remove_player(&mut self, level: &ServerLevel, player: EntityId) {
        if let Some(row) = self.connections.backing_map.shift_remove(&player) {
            for connection in row.values() {
                connection.disconnect(level);
            }
        }
        self.untrack_waypoint(level, player);
        self.players.remove(&player);
    }

    pub fn break_all_connections(&mut self, level: &ServerLevel) {
        for row in self.connections.backing_map.values() {
            for connection in row.values() {
                connection.disconnect(level);
            }
        }
        self.connections.backing_map.clear();
    }

    pub fn remake_connections(&mut self, level: &ServerLevel, waypoint: EntityId) {
        for player in self.players.iter() {
            create_connection(&mut self.connections, level, player, waypoint);
        }
    }

    pub fn transmitters(&self) -> &JavaObjectHashSet<EntityId> {
        &self.waypoints
    }
}

fn is_locator_bar_enabled_for(level: &ServerLevel) -> bool {
    level.game_rules.get_bool(game_rule::LOCATOR_BAR)
}

fn create_connection(connections: &mut Table, level: &ServerLevel, player: EntityId, waypoint: EntityId) {
    if player != waypoint && is_locator_bar_enabled_for(level) {
        match level.make_waypoint_connection_with(waypoint, player) {
            Some(connection) => {
                connection.connect(level);
                connections.put(player, waypoint, connection);
            }
            None => {
                if let Some(connection) = connections.remove(player, waypoint) {
                    connection.disconnect(level);
                }
            }
        }
    }
}

fn update_connection(connections: &mut Table, level: &ServerLevel, player: EntityId, waypoint: EntityId) {
    if player != waypoint && is_locator_bar_enabled_for(level) {
        let Some(connection) = connections.get_mut(player, waypoint) else { return };
        if !connection.is_broken(level) {
            connection.update(level);
        } else {
            match level.make_waypoint_connection_with(waypoint, player) {
                Some(new_connection) => {
                    new_connection.connect(level);
                    connections.put(player, waypoint, new_connection);
                }
                None => {
                    connection.disconnect(level);
                    connections.remove(player, waypoint);
                }
            }
        }
    }
}
