use crisp_core::random::RandomSource;
use crisp_core::{Vec3, mth};
use crisp_data::{items, registry, registry_id};

use crate::server::level::ServerLevel;
use crate::server::level::events::PARTICLES_EYE_OF_ENDER_DEATH;
use crate::world::entity::data::{DataValue, entity_defaults};
use crate::world::entity::{Entity, EntityDimensions, EntityId, ItemEntity, LevelEntity, sound_event};
use crate::world::item::ItemStack;

const TOO_FAR_SIGNAL_HEIGHT: f64 = 8.0;
const TOO_FAR_DISTANCE: f64 = 12.0;
const DATA_ITEM_STACK: usize = 8;

pub struct EyeOfEnder {
    pub entity: Entity,
    target: Option<Vec3>,
    life: i32,
    survive_after_death: bool,
}

impl EyeOfEnder {
    pub const DIMENSIONS: EntityDimensions = EntityDimensions { width: 0.25, height: 0.25, eye_height: 0.25 * 0.85 };

    pub fn of_type() -> EyeOfEnder {
        let mut data = entity_defaults();
        data.push(DataValue::ItemStack(Some(get_default_item())));
        let entity = Entity::new(registry_id(registry::ENTITY_TYPE, "minecraft:eye_of_ender"), Self::DIMENSIONS, data);
        EyeOfEnder { entity, target: None, life: 0, survive_after_death: false }
    }

    pub fn new(x: f64, y: f64, z: f64) -> EyeOfEnder {
        let mut eye = EyeOfEnder::of_type();
        eye.entity.set_pos(Vec3::new(x, y, z));
        eye
    }

    pub fn set_item(&mut self, source: &ItemStack) {
        let item = if source.is_empty() { get_default_item() } else { source.copy_with_count(1) };
        self.entity.data.set(DATA_ITEM_STACK, DataValue::ItemStack(Some(item)));
    }

    pub fn get_item(&self) -> ItemStack {
        match self.entity.data.get(DATA_ITEM_STACK) {
            DataValue::ItemStack(Some(item)) => item.clone(),
            _ => ItemStack::empty(),
        }
    }

    pub fn signal_to(&mut self, target: Vec3) {
        let position = self.entity.position();
        let delta = target.subtract(position);
        let horizontal_distance = delta.horizontal_distance();
        self.target = Some(if horizontal_distance > TOO_FAR_DISTANCE {
            position.add(delta.x / horizontal_distance * TOO_FAR_DISTANCE, TOO_FAR_SIGNAL_HEIGHT, delta.z / horizontal_distance * TOO_FAR_DISTANCE)
        } else {
            target
        });
        self.life = 0;
        self.survive_after_death = self.entity.random.next_int_bounded(5) > 0;
    }
}

fn get_default_item() -> ItemStack {
    ItemStack::new(items::ENDER_EYE, 1)
}

fn update_delta_movement(old_movement: Vec3, position: Vec3, target: Vec3) -> Vec3 {
    let horizontal_delta = Vec3::new(target.x - position.x, 0.0, target.z - position.z);
    let horizontal_length = horizontal_delta.length();
    let mut wanted_speed = mth::lerp(0.0025, old_movement.horizontal_distance(), horizontal_length);
    let mut movement_y = old_movement.y;
    if horizontal_length < 1.0 {
        wanted_speed *= 0.8;
        movement_y *= 0.8;
    }
    let wanted_movement_y = if position.y - old_movement.y < target.y { 1.0 } else { -1.0 };
    horizontal_delta.scale(wanted_speed / horizontal_length).add(0.0, movement_y + (wanted_movement_y - movement_y) * 0.015, 0.0)
}

impl ServerLevel {
    fn eye_of_ender(&mut self, id: EntityId) -> &mut EyeOfEnder {
        match self.entities.get_mut(&id) {
            Some(LevelEntity::EyeOfEnder(eye)) => eye,
            _ => unreachable!("not an eye of ender"),
        }
    }

    pub fn tick_eye_of_ender(&mut self, id: EntityId) {
        self.entity_base_tick(id);
        let eye = self.eye_of_ender(id);
        let new_position = eye.entity.position().add_vec(eye.entity.delta_movement);
        if let Some(target) = eye.target {
            eye.entity.delta_movement = update_delta_movement(eye.entity.delta_movement, new_position, target);
        }
        eye.entity.set_pos(new_position);
        self.entity_moved(id);
        let eye = self.eye_of_ender(id);
        eye.life += 1;
        if eye.life > 80 {
            self.entity_play_sound(id, sound_event("minecraft:entity.ender_eye.death"), 1.0, 1.0);
            let eye = self.eye_of_ender(id);
            eye.entity.discard();
            let (position, survive_after_death, item) = (eye.entity.position(), eye.survive_after_death, eye.get_item());
            if survive_after_death {
                self.add_fresh_entity(LevelEntity::Item(ItemEntity::new(position, item)));
            } else {
                let block_position = self.entity(id).block_position();
                self.level_event(None, PARTICLES_EYE_OF_ENDER_DEATH, block_position, 0);
            }
        }
    }
}
