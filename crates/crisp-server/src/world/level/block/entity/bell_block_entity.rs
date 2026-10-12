use crisp_core::{AABB, BlockPos, Direction};
use crisp_data::{entity_type_tags, mob_effects, registry, registry_id};

use super::BlockEntityKind;
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::effect::MobEffectInstance;
use crate::world::entity::EntityId;
use crate::world::level::block::bell_block::EVENT_BELL_RING;

const DURATION: i32 = 50;
const GLOW_DURATION: i32 = 60;
const MIN_TICKS_BETWEEN_SEARCHES: i64 = 60;
const MAX_RESONATION_TICKS: i32 = 40;
const TICKS_BEFORE_RESONATION: i32 = 5;
const SEARCH_RADIUS: f64 = 48.0;
const HEAR_BELL_RADIUS: f64 = 32.0;
const HIGHLIGHT_RAIDERS_RADIUS: f64 = 48.0;

#[derive(Clone, Default)]
pub struct BellBlockEntity {
    last_ring_timestamp: i64,
    pub ticks: i32,
    pub shaking: bool,
    pub click_direction: Option<Direction>,
    nearby_entities: Option<Vec<EntityId>>,
    resonating: bool,
    resonation_ticks: i32,
}

fn closer_to_center_than(level: &ServerLevel, pos: BlockPos, entity: EntityId, distance: f64) -> bool {
    let position = level.entity(entity).position();
    pos.dist_to_center_sqr(position.x, position.y, position.z) < distance * distance
}

fn is_alive_and_present(level: &ServerLevel, entity: EntityId) -> bool {
    level.entity_ref(entity).is_some_and(|e| !e.is_removed()) && level.is_alive(entity)
}

fn is_raider(level: &ServerLevel, entity: EntityId) -> bool {
    entity_type_tags::RAIDERS.contains(level.entity(entity).entity_type as usize)
}

fn are_raiders_nearby(level: &ServerLevel, bell_pos: BlockPos, nearby_entities: &[EntityId]) -> bool {
    nearby_entities
        .iter()
        .any(|&entity| is_alive_and_present(level, entity) && closer_to_center_than(level, bell_pos, entity, HEAR_BELL_RADIUS) && is_raider(level, entity))
}

fn is_raider_within_range(level: &ServerLevel, block_pos: BlockPos, entity: EntityId) -> bool {
    is_alive_and_present(level, entity) && closer_to_center_than(level, block_pos, entity, HIGHLIGHT_RAIDERS_RADIUS) && is_raider(level, entity)
}

impl ServerLevel {
    fn bell(&mut self, pos: BlockPos) -> Option<&mut BellBlockEntity> {
        match &mut self.get_block_entity(pos)?.kind {
            BlockEntityKind::Bell(bell) => Some(bell),
            _ => None,
        }
    }

    pub fn bell_trigger_event(&mut self, pos: BlockPos, b0: i32, b1: i32) -> bool {
        if b0 != 1 || self.bell(pos).is_none() {
            return false;
        }
        self.bell_update_entities(pos);
        let Some(bell) = self.bell(pos) else { return false };
        bell.resonation_ticks = 0;
        bell.click_direction = Some(Direction::from_3d_data_value(b1));
        bell.ticks = 0;
        bell.shaking = true;
        true
    }

    pub fn bell_server_tick(&mut self, pos: BlockPos) {
        let Some(bell) = self.bell(pos) else { return };
        if bell.shaking {
            bell.ticks += 1;
        }
        if bell.ticks >= DURATION {
            bell.shaking = false;
            bell.ticks = 0;
        }
        let (ticks, resonation_ticks) = (bell.ticks, bell.resonation_ticks);
        let nearby_entities = bell.nearby_entities.clone().unwrap_or_default();
        if ticks >= TICKS_BEFORE_RESONATION && resonation_ticks == 0 && are_raiders_nearby(self, pos, &nearby_entities) {
            if let Some(bell) = self.bell(pos) {
                bell.resonating = true;
            }
            self.play_sound(None, pos, registry_id(registry::SOUND_EVENT, "minecraft:block.bell.resonate"), SoundSource::Blocks, 1.0, 1.0);
        }
        let Some(bell) = self.bell(pos) else { return };
        if bell.resonating {
            if bell.resonation_ticks < MAX_RESONATION_TICKS {
                bell.resonation_ticks += 1;
            } else {
                self.bell_make_raiders_glow(pos, &nearby_entities);
                if let Some(bell) = self.bell(pos) {
                    bell.resonating = false;
                }
            }
        }
    }

    pub fn bell_on_hit(&mut self, pos: BlockPos, click_direction: Direction) {
        let Some(entity) = self.get_block_entity(pos) else { return };
        let block = entity.block_state().block();
        let BlockEntityKind::Bell(bell) = &mut entity.kind else { return };
        bell.click_direction = Some(click_direction);
        if bell.shaking {
            bell.ticks = 0;
        } else {
            bell.shaking = true;
        }
        self.block_event(pos, block, EVENT_BELL_RING, click_direction.ordinal() as i32);
    }

    fn bell_update_entities(&mut self, block_pos: BlockPos) {
        let game_time = self.game_time;
        let Some(bell) = self.bell(block_pos) else { return };
        if game_time > bell.last_ring_timestamp + MIN_TICKS_BETWEEN_SEARCHES || bell.nearby_entities.is_none() {
            bell.last_ring_timestamp = game_time;
            let aabb = AABB::of_block(block_pos).inflate_all(SEARCH_RADIUS);
            let nearby_entities: Vec<EntityId> = self.get_entities(None, &aabb).into_iter().filter(|&id| self.is_living(id)).collect();
            if let Some(bell) = self.bell(block_pos) {
                bell.nearby_entities = Some(nearby_entities);
            }
        }
    }

    fn bell_make_raiders_glow(&mut self, block_pos: BlockPos, nearby_entities: &[EntityId]) {
        for &entity in nearby_entities {
            if is_raider_within_range(self, block_pos, entity) {
                self.add_effect(entity, MobEffectInstance::new(mob_effects::GLOWING, GLOW_DURATION, 0), None);
            }
        }
    }
}
