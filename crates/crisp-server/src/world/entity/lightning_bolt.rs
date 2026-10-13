use std::collections::HashSet;

use crisp_core::random::RandomSource;
use crisp_core::{AABB, BlockPos, Vec3};
use crisp_data::properties::{FACING, POWERED};
use crisp_data::{BlockState, WAXABLES, damage_types, registry, registry_id};

use super::mob::MobKind;
use super::{Entity, EntityDimensions, EntityId, LevelEntity};
use crate::server::level::ServerLevel;
use crate::world::damagesource::DamageSource;
use crate::world::difficulty::Difficulty;
use crate::world::level::BlockGetter;
use crate::world::level::block::{self, base_fire_block, weathering_copper};
use crate::world::level::update_flags::UPDATE_ALL;

const START_LIFE: i32 = 2;
const PARTICLES_SCRAPE: i32 = 3002;

pub struct LightningBolt {
    pub entity: Entity,
    life: i32,
    pub seed: i64,
    flashes: i32,
    pub visual_only: bool,
    pub cause: Option<EntityId>,
    hit_entities: HashSet<EntityId>,
    pub blocks_set_on_fire: i32,
}

impl LightningBolt {
    pub const DIMENSIONS: EntityDimensions = EntityDimensions { width: 0.0, height: 0.0, eye_height: 0.0 };

    pub fn new(pos: Vec3) -> LightningBolt {
        let mut entity = Entity::new(registry_id(registry::ENTITY_TYPE, "minecraft:lightning_bolt"), Self::DIMENSIONS, super::data::entity_defaults());
        let seed = entity.random.next_long();
        let flashes = entity.random.next_int_bounded(3) + 1;
        entity.set_pos(pos);
        entity.set_old_pos();
        LightningBolt { entity, life: START_LIFE, seed, flashes, visual_only: false, cause: None, hit_entities: HashSet::new(), blocks_set_on_fire: 0 }
    }

    pub fn hit_entities(&self) -> impl Iterator<Item = EntityId> + '_ {
        self.hit_entities.iter().copied()
    }

    fn strike_position(&self) -> BlockPos {
        let pos = self.entity.position();
        BlockPos::containing(pos.x, pos.y - 1.0e-6, pos.z)
    }
}

impl ServerLevel {
    fn lightning_bolt(&mut self, id: EntityId) -> &mut LightningBolt {
        match self.entities.get_mut(&id) {
            Some(LevelEntity::LightningBolt(bolt)) => bolt,
            _ => unreachable!("not a lightning bolt"),
        }
    }

    pub fn tick_lightning_bolt(&mut self, id: EntityId) {
        self.entity_base_tick(id);
        if self.lightning_bolt(id).life == START_LIFE {
            if matches!(self.difficulty, Difficulty::Normal | Difficulty::Hard) {
                self.spawn_lightning_fire(id, 4);
            }
            let strike = self.lightning_bolt(id).strike_position();
            self.power_lightning_rod(strike);
            self.clear_copper_on_lightning_strike(strike);
        }
        let bolt = self.lightning_bolt(id);
        bolt.life -= 1;
        if bolt.life < 0 {
            if bolt.flashes == 0 {
                let pos = bolt.entity.position();
                let area = AABB::new(pos.x - 15.0, pos.y - 15.0, pos.z - 15.0, pos.x + 15.0, pos.y + 6.0 + 15.0, pos.z + 15.0);
                let hit_entities = bolt.hit_entities.clone();
                let viewers: Vec<EntityId> = self
                    .all_entities()
                    .filter(|&e| e != id && self.is_alive(e) && !hit_entities.contains(&e) && self.entity(e).bounding_box().intersects(&area))
                    .collect();
                let players: Vec<EntityId> = self.players.values().filter(|p| (p.entity.position().distance_to_sqr(pos) as f32).sqrt() < 256.0).map(|p| p.entity.id).collect();
                for player in players {
                    crate::advancements::triggers::criteria_triggers::LIGHTNING_STRIKE.trigger(self, player, id, &viewers);
                }
                self.lightning_bolt(id).entity.discard();
            } else if bolt.life < -bolt.entity.random.next_int_bounded(10) {
                bolt.flashes -= 1;
                bolt.life = 1;
                bolt.seed = bolt.entity.random.next_long();
                self.spawn_lightning_fire(id, 0);
            }
        }
        let bolt = self.lightning_bolt(id);
        if bolt.life >= 0 && !bolt.visual_only {
            let pos = bolt.entity.position();
            let area = AABB::new(pos.x - 3.0, pos.y - 3.0, pos.z - 3.0, pos.x + 3.0, pos.y + 6.0 + 3.0, pos.z + 3.0);
            let hit: Vec<EntityId> = self
                .all_entities()
                .filter(|&e| e != id && self.is_alive(e) && self.entity(e).bounding_box().intersects(&area))
                .collect();
            for &entity in &hit {
                self.thunder_hit(entity, id);
            }
            self.lightning_bolt(id).hit_entities.extend(hit.iter().copied());
            if let Some(cause) = self.lightning_bolt(id).cause.filter(|&cause| self.is_player(cause)) {
                crate::advancements::triggers::criteria_triggers::CHANNELED_LIGHTNING.trigger(self, cause, &hit);
            }
        }
    }

    fn thunder_hit(&mut self, id: EntityId, lightning_bolt: EntityId) {
        match self.mob(id).map(|mob| &mob.kind) {
            Some(MobKind::MushroomCow(_)) => self.mushroom_cow_thunder_hit(id, lightning_bolt),
            _ => self.entity_thunder_hit(id, lightning_bolt),
        }
    }

    pub fn entity_thunder_hit(&mut self, id: EntityId, _lightning_bolt: EntityId) {
        let ticks = self.entity(id).remaining_fire_ticks + 1;
        self.set_remaining_fire_ticks(id, ticks);
        if self.entity(id).remaining_fire_ticks == 0 {
            self.ignite_for_ticks(id, 8 * 20);
        }
        self.hurt_server(id, DamageSource::new(damage_types::LIGHTNING_BOLT), 5.0);
    }

    fn spawn_lightning_fire(&mut self, id: EntityId, additional_sources: i32) {
        let bolt = self.lightning_bolt(id);
        if bolt.visual_only {
            return;
        }
        let pos = bolt.entity.block_position();
        if !self.can_spread_fire_around(pos) {
            return;
        }
        self.try_lightning_fire(id, pos);
        for _ in 0..additional_sources {
            let random = &mut self.lightning_bolt(id).entity.random;
            let (dx, dy, dz) = (random.next_int_bounded(3) - 1, random.next_int_bounded(3) - 1, random.next_int_bounded(3) - 1);
            self.try_lightning_fire(id, pos.offset(dx, dy, dz));
        }
    }

    fn try_lightning_fire(&mut self, id: EntityId, pos: BlockPos) {
        let fire = base_fire_block::get_state(self, pos);
        if self.block_state(pos).is_air() && block::of(fire).can_survive(fire, self, pos) {
            self.set_block(pos, fire, UPDATE_ALL);
            self.lightning_bolt(id).blocks_set_on_fire += 1;
        }
    }

    fn power_lightning_rod(&mut self, pos: BlockPos) {
        let state = self.block_state(pos);
        if !state.block().is_instance_of("LightningRodBlock") {
            return;
        }
        self.set_block(pos, state.set(POWERED, true), UPDATE_ALL);
        let front = state.get(FACING).opposite();
        self.update_neighbors_at(pos.relative(front), state.block());
        self.schedule_tick(pos, state.block(), 8);
        self.level_event(None, PARTICLES_SCRAPE, pos, state.get(FACING).axis() as i32);
    }

    fn clear_copper_on_lightning_strike(&mut self, struck_pos: BlockPos) {
        let struck_state = self.block_state(struck_pos);
        let is_waxed = WAXABLES.iter().any(|&(_, waxed)| waxed == struck_state.block());
        let is_weathering_copper = weathering_copper::age(struck_state.block()).is_some();
        if !is_weathering_copper && !is_waxed {
            return;
        }
        if is_weathering_copper {
            let first = weathering_copper::get_first(self.block_state(struck_pos));
            self.set_block(struck_pos, first, UPDATE_ALL);
        }
        let strikes = self.random.next_int_bounded(3) + 3;
        for _ in 0..strikes {
            let steps = self.random.next_int_bounded(8) + 1;
            let mut work_pos = struck_pos;
            for _ in 0..steps {
                match self.random_step_cleaning_copper(work_pos) {
                    Some(pos) => work_pos = pos,
                    None => break,
                }
            }
        }
    }

    fn random_step_cleaning_copper(&mut self, pos: BlockPos) -> Option<BlockPos> {
        for _ in 0..10 {
            let candidate = BlockPos::new(
                pos.x - 1 + self.random.next_int_bounded(3),
                pos.y - 1 + self.random.next_int_bounded(3),
                pos.z - 1 + self.random.next_int_bounded(3),
            );
            let state: BlockState = self.block_state(candidate);
            if weathering_copper::age(state.block()).is_some() {
                if let Some(previous) = weathering_copper::get_previous(state) {
                    self.set_block(candidate, previous, UPDATE_ALL);
                }
                self.level_event(None, PARTICLES_SCRAPE, candidate, -1);
                return Some(candidate);
            }
        }
        None
    }

    pub fn strike_lightning(&mut self, pos: BlockPos, visual_only: bool) -> EntityId {
        let mut bolt = LightningBolt::new(Vec3::at_bottom_center_of(pos));
        bolt.visual_only = visual_only;
        let id = bolt.entity.id;
        self.add_fresh_entity(LevelEntity::LightningBolt(bolt));
        id
    }
}

#[cfg(test)]
mod tests {
    use crisp_core::{BlockPos, Vec3};
    use crisp_data::blocks;

    use crate::server::level::testing;
    use crate::world::difficulty::Difficulty;
    use crate::world::level::BlockGetter;

    #[test]
    fn a_strike_burns_and_hurts_then_goes() {
        let mut level = testing::level();
        let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
        level.handle_player_loaded(player);
        level.difficulty = Difficulty::Normal;
        let bolt = level.strike_lightning(BlockPos::new(0, -60, 0), false);
        level.tick();
        assert!(level.block_state(BlockPos::new(0, -60, 0)).is_block(blocks::FIRE));
        assert!(level.player(player).health < 20.0);
        assert!(level.entity(player).remaining_fire_ticks > 0);
        for _ in 0..60 {
            level.tick();
        }
        assert!(!level.entities.contains_key(&bolt));
    }
}
