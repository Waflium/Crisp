use crisp_core::random::RandomSource;
use crisp_core::{AABB, BlockPos, Direction};
use crisp_data::properties::{CHEST_TYPE, ChestType, FACING, OPEN};
use crisp_data::{BlockState, registry, registry_id};

use super::{BlockEntity, BlockEntityKind, chest_block_entity};
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::entity::EntityId;
use crate::world::level::block::chest_block;
use crate::world::level::update_flags::UPDATE_ALL;

pub const CHECK_TICK_DELAY: i32 = 5;

#[derive(Clone, Default)]
pub struct ContainerOpenersCounter {
    pub open_count: i32,
    pub max_interaction_range: f64,
}

fn counter_mut(entity: &mut BlockEntity) -> Option<&mut ContainerOpenersCounter> {
    match &mut entity.kind {
        BlockEntityKind::Chest(chest) => Some(&mut chest.openers_counter),
        BlockEntityKind::Barrel(barrel) => Some(&mut barrel.openers_counter),
        _ => None,
    }
}

impl ServerLevel {
    fn openers_counter(&mut self, pos: BlockPos) -> Option<(&mut ContainerOpenersCounter, BlockState)> {
        let entity = self.get_block_entity(pos)?;
        let state = entity.block_state();
        counter_mut(entity).map(|c| (c, state))
    }

    pub fn increment_openers(&mut self, pos: BlockPos, max_interaction_range: f64) {
        let Some((counter, state)) = self.openers_counter(pos) else { return };
        let previous = counter.open_count;
        counter.open_count += 1;
        let current = counter.open_count;
        if previous == 0 {
            self.openers_on_open(pos, state);
            self.schedule_tick(pos, state.block(), CHECK_TICK_DELAY);
        }
        self.opener_count_changed(pos, state, previous, current);
        if let Some((counter, _)) = self.openers_counter(pos) {
            counter.max_interaction_range = max_interaction_range.max(counter.max_interaction_range);
        }
    }

    pub fn decrement_openers(&mut self, pos: BlockPos) {
        let Some((counter, state)) = self.openers_counter(pos) else { return };
        let previous = counter.open_count;
        counter.open_count -= 1;
        let current = counter.open_count;
        if current == 0 {
            self.openers_on_close(pos, state);
            if let Some((counter, _)) = self.openers_counter(pos) {
                counter.max_interaction_range = 0.0;
            }
        }
        self.opener_count_changed(pos, state, previous, current);
    }

    pub fn get_entities_with_container_open(&mut self, pos: BlockPos) -> Vec<EntityId> {
        let Some((counter, _)) = self.openers_counter(pos) else { return Vec::new() };
        let range = counter.max_interaction_range + 4.0;
        let Some(entity) = self.get_block_entity(pos) else { return Vec::new() };
        let id = entity.id();
        let search = AABB::of_block(pos).inflate(range, range, range);
        let players: Vec<EntityId> = self.players.values().filter(|p| p.entity.bounding_box().intersects(&search)).map(|p| p.entity.id).collect();
        players.into_iter().filter(|&p| !self.is_spectator(p) && self.is_own_container(p, pos, id)).collect()
    }

    fn is_own_container(&self, player: EntityId, pos: BlockPos, id: u64) -> bool {
        let Some(container) = self.player(player).container_menu.as_ref().and_then(|m| m.chest_container()) else { return false };
        match self.chunks.loaded_chunk(pos.chunk()).and_then(|c| c.block_entities.map.get(&pos)).map(|e| &e.kind) {
            Some(BlockEntityKind::Barrel(_)) => container.is_single(pos, id),
            _ => container.contains(pos, id),
        }
    }

    pub fn recheck_openers(&mut self, pos: BlockPos) {
        let Some((_, state)) = self.openers_counter(pos) else { return };
        let users = self.get_entities_with_container_open(pos);
        let open_count = users.len() as i32;
        let Some((counter, _)) = self.openers_counter(pos) else { return };
        counter.max_interaction_range = if users.is_empty() { 0.0 } else { crate::server::level::game_mode::BLOCK_INTERACTION_RANGE };
        let previous = counter.open_count;
        if previous != open_count {
            let is_open = open_count != 0;
            let was_open = previous != 0;
            if is_open && !was_open {
                self.openers_on_open(pos, state);
            } else if !is_open {
                self.openers_on_close(pos, state);
            }
            if let Some((counter, _)) = self.openers_counter(pos) {
                counter.open_count = open_count;
            }
        }
        self.opener_count_changed(pos, state, previous, open_count);
        if open_count > 0 {
            self.schedule_tick(pos, state.block(), CHECK_TICK_DELAY);
        }
    }

    fn openers_on_open(&mut self, pos: BlockPos, state: BlockState) {
        if state.block().is_instance_of("ChestBlock") {
            let sound = chest_block::open_sound(state.block());
            self.play_chest_sound(pos, state, sound);
        } else if state.block().is_instance_of("BarrelBlock") {
            self.play_barrel_sound(pos, state, "minecraft:block.barrel.open");
            self.set_block(pos, state.set(OPEN, true), UPDATE_ALL);
        }
    }

    fn openers_on_close(&mut self, pos: BlockPos, state: BlockState) {
        if state.block().is_instance_of("ChestBlock") {
            let sound = chest_block::close_sound(state.block());
            self.play_chest_sound(pos, state, sound);
        } else if state.block().is_instance_of("BarrelBlock") {
            self.play_barrel_sound(pos, state, "minecraft:block.barrel.close");
            self.set_block(pos, state.set(OPEN, false), UPDATE_ALL);
        }
    }

    fn opener_count_changed(&mut self, pos: BlockPos, state: BlockState, previous: i32, current: i32) {
        let is_chest = matches!(self.get_block_entity(pos).map(|e| &e.kind), Some(BlockEntityKind::Chest(_)));
        if !is_chest {
            return;
        }
        let block = state.block();
        self.block_event(pos, block, chest_block_entity::EVENT_SET_OPEN_COUNT, current);
        let trapped = self.get_block_entity(pos).is_some_and(|e| e.ty() == crisp_data::block_entity_types::TRAPPED_CHEST);
        if trapped && previous != current {
            self.update_neighbors_at(pos, block);
            self.update_neighbors_at(pos.below(), block);
        }
    }

    fn play_chest_sound(&mut self, pos: BlockPos, state: BlockState, sound: i32) {
        let ty = state.get(CHEST_TYPE);
        if ty == ChestType::Left {
            return;
        }
        let (mut x, y, mut z) = (pos.x as f64 + 0.5, pos.y as f64 + 0.5, pos.z as f64 + 0.5);
        if ty == ChestType::Right {
            let direction = chest_block::get_connected_direction(state);
            x += direction.step_x() as f64 * 0.5;
            z += direction.step_z() as f64 * 0.5;
        }
        let pitch = self.random.next_float() * 0.1 + 0.9;
        self.play_sound_at(None, x, y, z, sound, SoundSource::Blocks, 0.5, pitch);
    }

    fn play_barrel_sound(&mut self, pos: BlockPos, state: BlockState, sound: &str) {
        let direction: Direction = state.get(FACING);
        let x = pos.x as f64 + 0.5 + direction.step_x() as f64 / 2.0;
        let y = pos.y as f64 + 0.5 + direction.step_y() as f64 / 2.0;
        let z = pos.z as f64 + 0.5 + direction.step_z() as f64 / 2.0;
        let pitch = self.random.next_float() * 0.1 + 0.9;
        self.play_sound_at(None, x, y, z, registry_id(registry::SOUND_EVENT, sound), SoundSource::Blocks, 0.5, pitch);
    }
}
