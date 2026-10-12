use std::time::Duration;

use indexmap::{IndexMap, IndexSet};
use crisp_core::fastutil::Int2ObjectOpenHashMap;
use crisp_core::hash_map::FastSet;
use crate::util::FastIndexSet;

use crisp_core::{BlockPos, ChunkPos, mth};
use crisp_data::packets::play::clientbound;

use super::ServerLevel;
use super::server_entity::{Audience, ServerEntity, UpdateInterval};
use crate::network::connection::encode;
use crate::world::entity::{Entity, EntityId, LevelEntity};
use crate::world::level::BlockGetter;
use crate::world::level::light_layer::LightLayer;
use crate::world::level::lighting::{section, write_light_data};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ChunkTrackingView {
    pub center: ChunkPos,
    pub view_distance: i32,
}

impl ChunkTrackingView {
    fn min_x(self) -> i32 {
        self.center.x - self.view_distance - 1
    }
    fn min_z(self) -> i32 {
        self.center.z - self.view_distance - 1
    }
    fn max_x(self) -> i32 {
        self.center.x + self.view_distance + 1
    }
    fn max_z(self) -> i32 {
        self.center.z + self.view_distance + 1
    }

    fn square_intersects(self, o: ChunkTrackingView) -> bool {
        self.min_x() <= o.max_x() && self.max_x() >= o.min_x() && self.min_z() <= o.max_z() && self.max_z() >= o.min_z()
    }

    pub fn contains_with(self, x: i32, z: i32, include_neighbors: bool) -> bool {
        is_within_distance(self.center.x, self.center.z, self.view_distance, x, z, include_neighbors)
    }

    pub fn contains(self, x: i32, z: i32) -> bool {
        self.contains_with(x, z, true)
    }

    fn for_each(self, mut f: impl FnMut(ChunkPos)) {
        for x in self.min_x()..=self.max_x() {
            for z in self.min_z()..=self.max_z() {
                if self.contains(x, z) {
                    f(ChunkPos::new(x, z));
                }
            }
        }
    }

    pub fn difference(from: Option<ChunkTrackingView>, to: Option<ChunkTrackingView>, mut on_enter: impl FnMut(ChunkPos), mut on_leave: impl FnMut(ChunkPos)) {
        if from == to {
            return;
        }
        match (from, to) {
            (Some(last), Some(next)) if last.square_intersects(next) => {
                let (min_x, min_z) = (last.min_x().min(next.min_x()), last.min_z().min(next.min_z()));
                let (max_x, max_z) = (last.max_x().max(next.max_x()), last.max_z().max(next.max_z()));
                for x in min_x..=max_x {
                    for z in min_z..=max_z {
                        let (saw, sees) = (last.contains(x, z), next.contains(x, z));
                        if saw != sees {
                            if sees {
                                on_enter(ChunkPos::new(x, z));
                            } else {
                                on_leave(ChunkPos::new(x, z));
                            }
                        }
                    }
                }
            }
            _ => {
                if let Some(last) = from {
                    last.for_each(&mut on_leave);
                }
                if let Some(next) = to {
                    next.for_each(&mut on_enter);
                }
            }
        }
    }
}

pub fn is_within_distance(center_x: i32, center_z: i32, view_distance: i32, x: i32, z: i32, include_neighbors: bool) -> bool {
    let buffer = if include_neighbors { 2 } else { 1 };
    let dx = 0i64.max((x - center_x).abs() as i64 - buffer);
    let dz = 0i64.max((z - center_z).abs() as i64 - buffer);
    dx * dx + dz * dz < (view_distance * view_distance) as i64
}

pub struct TrackedEntity {
    pub server_entity: ServerEntity,
    range: i32,
    last_section_pos: (i32, i32, i32),
    pub seen_by: FastIndexSet<EntityId>,
}

#[derive(Default)]
struct ChangedBlocks {
    sections: IndexMap<i32, IndexSet<BlockPos>>,
    sky_changed_light_sections: Vec<u64>,
    block_changed_light_sections: Vec<u64>,
}

#[derive(Default)]
pub struct ChunkMap {
    pub view_distance: i32,
    pub tracked: Int2ObjectOpenHashMap<TrackedEntity>,
    changed: IndexMap<ChunkPos, ChangedBlocks>,
    held: IndexMap<ChunkPos, ChangedBlocks>,
    ignored: FastSet<EntityId>,
}

fn section_pos(e: &Entity) -> (i32, i32, i32) {
    let p = e.position();
    (mth::floor(p.x) >> 4, mth::floor(p.y) >> 4, mth::floor(p.z) >> 4)
}

impl ChunkMap {
    pub fn new(view_distance: i32) -> ChunkMap {
        ChunkMap { view_distance: view_distance.clamp(2, 32), ..ChunkMap::default() }
    }

    pub fn block_changed(&mut self, pos: BlockPos) {
        self.changed.entry(pos.chunk()).or_default().sections.entry(pos.y >> 4).or_default().insert(pos);
    }

    fn section_light_changed(&mut self, pos: ChunkPos, layer: LightLayer, index: usize) {
        let filter_bit = |filter: &[u64]| filter.get(index / 64).is_some_and(|w| w >> (index % 64) & 1 != 0);
        if self.changed.get(&pos).is_some_and(|c| filter_bit(c.filter(layer))) {
            return;
        }
        let filter = self.changed.entry(pos).or_default().filter_mut(layer);
        if filter.len() <= index / 64 {
            filter.resize(index / 64 + 1, 0);
        }
        filter[index / 64] |= 1 << (index % 64);
    }
}

impl ChangedBlocks {
    fn filter(&self, layer: LightLayer) -> &[u64] {
        match layer {
            LightLayer::Sky => &self.sky_changed_light_sections,
            LightLayer::Block => &self.block_changed_light_sections,
        }
    }

    fn filter_mut(&mut self, layer: LightLayer) -> &mut Vec<u64> {
        match layer {
            LightLayer::Sky => &mut self.sky_changed_light_sections,
            LightLayer::Block => &mut self.block_changed_light_sections,
        }
    }

    fn light_changed(&self) -> bool {
        self.sky_changed_light_sections.iter().chain(&self.block_changed_light_sections).any(|&w| w != 0)
    }
}

pub struct TrackingConfig {
    pub range: i32,
    pub update_interval: UpdateInterval,
    pub track_deltas: bool,
    pub stepped: bool,
    pub is_item: bool,
}

impl ServerLevel {
    pub fn level_players(&self) -> Vec<EntityId> {
        self.players.values().filter(|p| !p.entity.is_removed()).map(|p| p.entity.id).collect()
    }

    pub fn player_view_distance(&self, player: EntityId) -> i32 {
        mth::clamp_i(self.player(player).requested_view_distance, 2, self.chunk_map.view_distance)
    }

    pub fn is_chunk_tracked(&self, player: EntityId, x: i32, z: i32) -> bool {
        let p = self.player(player);
        p.chunk_tracking_view.is_some_and(|v| v.contains(x, z)) && !p.chunk_sender.is_pending(ChunkPos::new(x, z))
    }

    fn is_chunk_on_tracked_border(&self, player: EntityId, x: i32, z: i32) -> bool {
        if !self.is_chunk_tracked(player, x, z) {
            return false;
        }
        for dx in -1..=1 {
            for dz in -1..=1 {
                if (dx != 0 || dz != 0) && !self.is_chunk_tracked(player, x + dx, z + dz) {
                    return true;
                }
            }
        }
        false
    }

    pub fn players_tracking_chunk(&self, pos: ChunkPos, border_only: bool) -> Vec<EntityId> {
        self.players
            .keys()
            .copied()
            .filter(|&p| if border_only { self.is_chunk_on_tracked_border(p, pos.x, pos.z) } else { self.is_chunk_tracked(p, pos.x, pos.z) })
            .collect()
    }

    pub fn update_chunk_tracking(&mut self, player: EntityId) {
        let center = self.player(player).entity.chunk_position();
        let view_distance = self.player_view_distance(player);
        let next = ChunkTrackingView { center, view_distance };
        if self.player(player).chunk_tracking_view != Some(next) {
            self.apply_chunk_tracking_view(player, Some(next));
        }
    }

    pub fn apply_chunk_tracking_view(&mut self, player: EntityId, next: Option<ChunkTrackingView>) {
        let previous = self.player(player).chunk_tracking_view;
        if let Some(to) = next
            && previous.is_none_or(|from| from.center != to.center)
        {
            self.player(player).connection.send(clientbound::SET_CHUNK_CACHE_CENTER, |p| {
                p.varint(to.center.x);
                p.varint(to.center.z);
            });
        }
        let changes = std::cell::RefCell::new(Vec::new());
        ChunkTrackingView::difference(previous, next, |p| changes.borrow_mut().push((true, p)), |p| changes.borrow_mut().push((false, p)));
        for (entered, pos) in changes.into_inner() {
            if entered {
                if self.chunks.chunk_to_send(pos) {
                    self.player_mut(player).chunk_sender.mark_chunk_pending_to_send(pos);
                }
                continue;
            }
            let alive = self.is_alive(player);
            let p = self.player_mut(player);
            if !p.chunk_sender.drop_chunk(pos) && alive {
                p.connection.send(clientbound::FORGET_LEVEL_CHUNK, |w| {
                    w.i32(pos.z);
                    w.i32(pos.x);
                });
            }
        }
        self.player_mut(player).chunk_tracking_view = next;
    }

    fn skip_player(&self, player: EntityId) -> bool {
        self.is_spectator(player) && !self.game_rules.get_bool(crisp_data::game_rule::SPECTATORS_GENERATE_CHUNKS)
    }

    fn update_player_status(&mut self, player: EntityId, added: bool) {
        if added {
            let ignored = self.skip_player(player);
            if ignored {
                self.chunk_map.ignored.insert(player);
            }
            let section = section_pos(&self.player(player).entity);
            self.player_mut(player).last_section_pos = section;
            if !ignored {
                self.chunks.add_player(ChunkPos::new(section.0, section.2), player);
            }
            self.player_mut(player).chunk_tracking_view = None;
            self.update_chunk_tracking(player);
        } else {
            let last = self.player(player).last_section_pos;
            let was_ignored = self.chunk_map.ignored.remove(&player);
            if !was_ignored {
                self.chunks.remove_player(ChunkPos::new(last.0, last.2), player);
            }
            self.apply_chunk_tracking_view(player, None);
        }
    }

    pub fn process_chunk_events(&mut self) {
        for (pos, starts) in self.chunks.take_structure_starts_available() {
            self.structure_check.on_structure_load(pos, starts);
        }
        for pos in self.chunks.take_ticking_ready() {
            self.post_process_generation(pos);
            self.chunks.start_ticking_chunk(pos);
        }
        while let Some((pos, status)) = self.chunks.pop_full_status_change() {
            self.update_chunk_status(pos, status);
        }
        for pos in self.chunks.take_ready_to_send() {
            let players: Vec<EntityId> = self.players.keys().copied().collect();
            for player in players {
                if self.player(player).chunk_tracking_view.is_some_and(|v| v.contains(pos.x, pos.z)) {
                    self.player_mut(player).chunk_sender.mark_chunk_pending_to_send(pos);
                }
            }
            if let Some(changes) = self.chunk_map.held.shift_remove(&pos) {
                self.chunk_map.changed.entry(pos).or_insert(changes);
            }
        }
    }

    pub fn poll_chunk_task(&mut self) -> bool {
        let worked = self.chunks.poll_task();
        self.process_chunk_events();
        worked
    }

    pub fn wait_for_chunk_work(&mut self, timeout: Duration) {
        self.chunks.wait_for_work(timeout);
        self.process_chunk_events();
    }

    pub fn finish_chunk_work(&mut self) {
        self.chunks.run_until_idle();
        self.process_chunk_events();
        self.wait_for_entities();
    }

    pub fn track_entity(&mut self, id: EntityId, config: TrackingConfig) {
        let entity = self.entity_ref(id).expect("tracked entity exists");
        let server_entity = ServerEntity::new(entity, config.update_interval, config.track_deltas, config.stepped, config.is_item);
        let tracked = TrackedEntity { server_entity, range: config.range, last_section_pos: section_pos(entity), seen_by: FastIndexSet::default() };
        self.chunk_map.tracked.insert(id, tracked);
        let players = self.level_players();
        self.update_tracked_players(id, &players);
        if self.players.contains_key(&id) {
            self.update_player_status(id, true);
            let others: Vec<EntityId> = self.chunk_map.tracked.keys().filter(|&e| e != id).collect();
            for other in others {
                self.update_tracked_player(other, id);
            }
        }
    }

    pub fn untrack_entity(&mut self, id: EntityId) {
        if self.players.contains_key(&id) {
            self.update_player_status(id, false);
            let others: Vec<EntityId> = self.chunk_map.tracked.keys().collect();
            for other in others {
                self.remove_tracking_player(other, id);
            }
        }
        if let Some(tracked) = self.chunk_map.tracked.remove(id) {
            for viewer in tracked.seen_by {
                if let Some(p) = self.players.get(&viewer) {
                    p.connection.send_raw(remove_entities(&[id]));
                }
            }
        }
    }

    fn remove_tracking_player(&mut self, tracked: EntityId, player: EntityId) {
        if let Some(t) = self.chunk_map.tracked.get_mut(tracked)
            && t.seen_by.shift_remove(&player)
        {
            self.player(player).connection.send_raw(remove_entities(&[tracked]));
        }
    }

    fn update_tracked_players(&mut self, tracked: EntityId, players: &[EntityId]) {
        for &player in players {
            self.update_tracked_player(tracked, player);
        }
    }

    fn update_tracked_player(&mut self, tracked: EntityId, player: EntityId) {
        if tracked == player {
            return;
        }
        let Some(entity) = self.entity_ref(tracked) else { return };
        let (entity_pos, entity_chunk) = (entity.position(), entity.chunk_position());
        let player_pos = self.player(player).entity.position();
        let range = self.chunk_map.tracked.get(tracked).unwrap().range;
        let effective_range = self.entity_broadcast_range_percentage * range / 100;
        let visible_range = (effective_range.min(self.player_view_distance(player) * 16)) as f64;
        let (dx, dz) = (player_pos.x - entity_pos.x, player_pos.z - entity_pos.z);
        let visible = dx * dx + dz * dz <= visible_range * visible_range && self.is_chunk_tracked(player, entity_chunk.x, entity_chunk.z);
        if visible {
            if self.chunk_map.tracked.get_mut(tracked).unwrap().seen_by.insert(player) {
                if self.is_living(tracked) {
                    self.update_dirty_effects(tracked);
                }
                let entity = self.entity_ref(tracked).unwrap();
                let mut packets = vec![encode(clientbound::BUNDLE_DELIMITER, |_| {})];
                packets.extend(self.chunk_map.tracked.get(tracked).unwrap().server_entity.pairing_packets(entity));
                packets.extend(self.extra_pairing_packets(tracked));
                packets.push(encode(clientbound::BUNDLE_DELIMITER, |_| {}));
                let connection = &self.player(player).connection;
                packets.into_iter().for_each(|p| connection.send_raw(p));
            }
        } else {
            self.remove_tracking_player(tracked, player);
        }
    }

    fn extra_pairing_packets(&self, tracked: EntityId) -> Vec<Vec<u8>> {
        if !self.is_living(tracked) {
            return Vec::new();
        }
        let mut packets = Vec::new();
        let attributes = &self.living(tracked).attributes;
        if attributes.syncable_attributes().next().is_some() {
            packets.push(encode(clientbound::UPDATE_ATTRIBUTES, |w| attributes.write_syncable(w, tracked)));
        }
        let equipment: Vec<_> =
            crisp_data::EquipmentSlot::VALUES.into_iter().filter_map(|slot| self.item_by_slot(tracked, slot).map(|s| (slot, Some(s)))).collect();
        if !equipment.is_empty() {
            packets.push(crate::world::entity::living_entity::set_equipment(tracked, &equipment));
        }
        packets
    }

    pub fn move_player(&mut self, player: EntityId) {
        let tracked: Vec<EntityId> = self.chunk_map.tracked.keys().collect();
        let players = self.level_players();
        for id in tracked {
            if id == player {
                self.update_tracked_players(id, &players);
            } else {
                self.update_tracked_player(id, player);
            }
        }
        let p = self.player(player);
        let (old, new) = (p.last_section_pos, section_pos(&p.entity));
        let was_ignored = self.chunk_map.ignored.contains(&player);
        let ignored = self.skip_player(player);
        if old != new || was_ignored != ignored {
            self.player_mut(player).last_section_pos = new;
            if !was_ignored {
                self.chunks.remove_player(ChunkPos::new(old.0, old.2), player);
            }
            if !ignored {
                self.chunks.add_player(ChunkPos::new(new.0, new.2), player);
            }
            if ignored {
                self.chunk_map.ignored.insert(player);
            } else {
                self.chunk_map.ignored.remove(&player);
            }
            self.update_chunk_tracking(player);
        }
    }

    pub fn tick_chunk_map(&mut self) {
        let players = self.level_players();
        for &player in &players {
            self.update_chunk_tracking(player);
        }
        let mut moved_players = Vec::new();
        let tracked: Vec<EntityId> = self.chunk_map.tracked.keys().collect();
        for id in tracked {
            let Some(entity) = self.entity_ref(id) else { continue };
            let new_pos = section_pos(entity);
            let changed = self.chunk_map.tracked.get(id).unwrap().last_section_pos != new_pos;
            if changed {
                self.update_tracked_players(id, &players);
                if self.players.contains_key(&id) {
                    moved_players.push(id);
                }
                self.chunk_map.tracked.get_mut(id).unwrap().last_section_pos = new_pos;
            }
            let needs_sync = self.entity_ref(id).is_some_and(|e| e.needs_sync);
            if changed || needs_sync || self.in_entity_ticking_range(ChunkPos::new(new_pos.0, new_pos.2)) {
                self.send_entity_changes(id);
            }
        }
        if !moved_players.is_empty() {
            let tracked: Vec<EntityId> = self.chunk_map.tracked.keys().collect();
            for id in tracked {
                self.update_tracked_players(id, &moved_players);
            }
        }
    }

    fn send_entity_changes(&mut self, id: EntityId) {
        if self.is_living(id) {
            self.update_dirty_effects(id);
        }
        let Some(tracked) = self.chunk_map.tracked.get_mut(id) else { return };
        let (entity, attributes) = match self.players.get_mut(&id) {
            Some(p) => (&mut p.entity, Some(&mut p.living.attributes)),
            None => match self.entities.get_mut(&id) {
                Some(LevelEntity::Mob(mob)) => (&mut mob.entity, Some(&mut mob.living.attributes)),
                Some(e) => (e.entity_mut(), None),
                None => return,
            },
        };
        let packets = tracked.server_entity.send_changes(entity, attributes);
        for (audience, packet) in packets {
            for viewer in &tracked.seen_by {
                if let Some(p) = self.players.get(viewer) {
                    p.connection.send_raw(packet.clone());
                }
            }
            if audience == Audience::TrackingAndSelf
                && let Some(p) = self.players.get(&id)
            {
                p.connection.send_raw(packet);
            }
        }
    }

    pub fn send_to_tracking_players(&self, id: EntityId, packet: &[u8]) {
        if let Some(tracked) = self.chunk_map.tracked.get(id) {
            for viewer in &tracked.seen_by {
                if let Some(p) = self.players.get(viewer) {
                    p.connection.send_raw(packet.to_vec());
                }
            }
        }
    }

    pub fn send_to_tracking_players_and_self(&self, id: EntityId, packet: &[u8]) {
        self.send_to_tracking_players(id, packet);
        if let Some(p) = self.players.get(&id) {
            p.connection.send_raw(packet.to_vec());
        }
    }

    pub fn apply_light_changes(&mut self) {
        let min_light_section = self.chunks.light_engine.engine.min_light_section();
        let max_light_section = self.chunks.light_engine.engine.max_light_section();
        for (layer, section_node) in self.chunks.take_light_changes() {
            let pos = ChunkPos::new(section::x(section_node), section::z(section_node));
            let section_y = section::y(section_node);
            if (min_light_section..=max_light_section).contains(&section_y) {
                self.chunk_map.section_light_changed(pos, layer, (section_y - min_light_section) as usize);
            }
        }
    }

    pub fn broadcast_changed_chunks(&mut self) {
        self.apply_light_changes();
        let changed = std::mem::take(&mut self.chunk_map.changed);
        for (chunk_pos, changes) in changed {
            if !self.chunks.is_ticking(chunk_pos) {
                self.chunk_map.held.insert(chunk_pos, changes);
                continue;
            }
            if changes.light_changed() {
                let border_players = self.players_tracking_chunk(chunk_pos, true);
                if !border_players.is_empty() {
                    let packet = encode(clientbound::LIGHT_UPDATE, |w| {
                        w.varint(chunk_pos.x);
                        w.varint(chunk_pos.z);
                        let (sky, block) = (&changes.sky_changed_light_sections, &changes.block_changed_light_sections);
                        write_light_data(w, chunk_pos, &self.chunks.light_engine.engine, Some(sky), Some(block));
                    });
                    for &player in &border_players {
                        self.player(player).connection.send_raw(packet.clone());
                    }
                }
            }
            if changes.sections.is_empty() {
                continue;
            }
            let players = self.players_tracking_chunk(chunk_pos, false);
            if players.is_empty() {
                continue;
            }
            for (section_y, positions) in changes.sections {
                let positions: Vec<BlockPos> = positions.into_iter().collect();
                let packet = if positions.len() == 1 {
                    let pos = *positions.first().unwrap();
                    let state = self.block_state(pos);
                    encode(clientbound::BLOCK_UPDATE, |p| {
                        p.position(pos.x, pos.y, pos.z);
                        p.varint(state.id() as i32);
                    })
                } else {
                    let section = (chunk_pos.x as i64 & 0x3F_FFFF) << 42 | (section_y as i64 & 0xF_FFFF) | (chunk_pos.z as i64 & 0x3F_FFFF) << 20;
                    let entries: Vec<i64> = positions
                        .iter()
                        .map(|&pos| {
                            let local = ((pos.x & 15) << 8 | (pos.z & 15) << 4 | (pos.y & 15)) as i64;
                            (self.block_state(pos).id() as i64) << 12 | local
                        })
                        .collect();
                    encode(clientbound::SECTION_BLOCKS_UPDATE, |p| {
                        p.i64(section);
                        p.varint(entries.len() as i32);
                        entries.iter().for_each(|&e| p.varlong(e));
                    })
                };
                for &player in &players {
                    self.player(player).connection.send_raw(packet.clone());
                }
                for pos in positions {
                    if self.block_state(pos).has_block_entity()
                        && let Some(packet) = self.block_entity_update_packet(pos)
                    {
                        for &player in &players {
                            self.player(player).connection.send_raw(packet.clone());
                        }
                    }
                }
            }
        }
    }
}

pub fn remove_entities(ids: &[EntityId]) -> Vec<u8> {
    encode(clientbound::REMOVE_ENTITIES, |p| {
        p.varint(ids.len() as i32);
        ids.iter().for_each(|&id| p.varint(id));
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn count(view_distance: i32) -> usize {
        let mut n = 0;
        ChunkTrackingView::difference(None, Some(ChunkTrackingView { center: ChunkPos::new(0, 0), view_distance }), |_| n += 1, |_| {});
        n
    }

    #[test]
    fn view_matches_vanilla_chunk_counts() {
        assert_eq!(count(2), 49);
        assert_eq!(count(5), 157);
        assert_eq!(count(10), 473);
    }

    use crisp_core::Vec3;
    use crisp_data::blocks;
    use crisp_protocol::Reader;
    use tokio::sync::mpsc::UnboundedReceiver;

    use crate::network::configuration::ClientInfo;
    use crate::network::connection::Outbound;
    use crate::network::login::Profile;
    use crate::server::level::server_player::ServerPlayer;
    use crate::server::level::testing;
    use crate::world::level::update_flags::UPDATE_ALL;

    fn level_with_player() -> (ServerLevel, UnboundedReceiver<Vec<u8>>) {
        let mut level = testing::level();
        level.set_simulation_distance(2);
        let (out, mut packets) = Outbound::capture();
        let client = ClientInfo::default();
        let mut player = ServerPlayer::new(Profile { uuid: 1, name: "Tester".into(), properties: Vec::new() }, client, out, BlockPos::ZERO);
        player.entity.set_pos(Vec3::new(8.5, -60.0, 8.5));
        let id = player.entity.id;
        level.add_player(player);
        level.finish_chunk_work();
        for _ in 0..10 {
            level.send_next_chunks(id);
            level.player_mut(id).chunk_sender.on_chunk_batch_received(64.0);
        }
        assert!(level.is_chunk_tracked(id, 3, 0));
        while packets.try_recv().is_ok() {}
        (level, packets)
    }

    #[derive(Debug, PartialEq)]
    enum Update {
        Light(i32, i32),
        Block(BlockPos),
    }

    fn updates(packets: &mut UnboundedReceiver<Vec<u8>>) -> Vec<(Update, Vec<u8>)> {
        let mut out = Vec::new();
        while let Ok(packet) = packets.try_recv() {
            let mut r = Reader::new(&packet);
            let id = r.varint().unwrap();
            if id == clientbound::LIGHT_UPDATE {
                let (x, z) = (r.varint().unwrap(), r.varint().unwrap());
                out.push((Update::Light(x, z), packet[packet.len() - r.remaining()..].to_vec()));
            } else if id == clientbound::BLOCK_UPDATE {
                let (x, y, z) = r.position().unwrap();
                out.push((Update::Block(BlockPos::new(x, y, z)), Vec::new()));
            }
        }
        out
    }

    fn sent_block_light(data: &[u8], index: usize, x: usize, y: usize, z: usize) -> Option<u8> {
        let mut r = Reader::new(data);
        let mut masks = Vec::new();
        for _ in 0..4 {
            let words: Vec<u64> = (0..r.varint().unwrap()).map(|_| r.varlong().unwrap() as u64).collect();
            masks.push(words);
        }
        let bit = |mask: &Vec<u64>, i: usize| mask.get(i / 64).is_some_and(|w| w >> (i % 64) & 1 != 0);
        let skip = r.varint().unwrap();
        for _ in 0..skip {
            let len = r.varint().unwrap() as usize;
            (0..len).for_each(|_| drop(r.u8()));
        }
        let mut arrays = Vec::new();
        for _ in 0..r.varint().unwrap() {
            let len = r.varint().unwrap() as usize;
            arrays.push((0..len).map(|_| r.u8().unwrap()).collect::<Vec<u8>>());
        }
        if !bit(&masks[1], index) {
            return None;
        }
        let array = &arrays[(0..index).filter(|&i| bit(&masks[1], i)).count()];
        let i = y << 8 | z << 4 | x;
        Some(array[i / 2] >> (4 * (i % 2)) & 15)
    }

    #[test]
    fn light_updates_reach_border_players_before_the_block_update() {
        let (mut level, mut packets) = level_with_player();
        let torch = BlockPos::new(3 * 16 + 8, -60, 8);
        level.set_block(torch, blocks::TORCH.default_state(), UPDATE_ALL);
        level.chunks.run_light_updates();
        level.tick();
        let sent = updates(&mut packets);
        let order: Vec<&Update> = sent.iter().map(|(u, _)| u).collect();
        let light_at = order.iter().position(|u| **u == Update::Light(3, 0)).expect("light update for the torch's chunk");
        let block_at = order.iter().position(|u| **u == Update::Block(torch)).expect("block update");
        assert!(light_at < block_at, "{order:?}");
        let mut lit: Vec<(i32, i32)> = order.iter().filter_map(|u| if let Update::Light(x, z) = u { Some((*x, *z)) } else { None }).collect();
        lit.sort();
        assert_eq!(lit, [(3, -1), (3, 0), (3, 1)]);
        let data = &sent[light_at].1;
        assert_eq!(sent_block_light(data, 1, 8, 4, 8), Some(14));
        assert_eq!(sent_block_light(data, 1, 10, 4, 8), Some(12));

        level.set_block(torch, blocks::AIR.default_state(), UPDATE_ALL);
        level.tick();
        assert_eq!(updates(&mut packets).into_iter().map(|(u, _)| u).collect::<Vec<_>>(), [Update::Block(torch)]);
        level.chunks.run_light_updates();
        level.tick();
        let sent = updates(&mut packets);
        let (_, data) = sent.iter().find(|(u, _)| *u == Update::Light(3, 0)).expect("light update after removal");
        assert_eq!(sent_block_light(data, 1, 8, 4, 8), Some(0));
        assert!(sent.iter().all(|(u, _)| matches!(u, Update::Light(..))));
    }
}
