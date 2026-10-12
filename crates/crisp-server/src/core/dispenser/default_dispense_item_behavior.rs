use crisp_core::random::RandomSource;
use crisp_core::{Axis, Direction, Vec3};

use super::block_source::BlockSource;
use crate::server::level::ServerLevel;
use crate::server::level::events::{PARTICLES_SHOOT_SMOKE, SOUND_DISPENSER_DISPENSE};
use crate::world::entity::{ItemEntity, LevelEntity};
use crate::world::item::ItemStack;

pub const DEFAULT_ACCURACY: i32 = 6;

pub fn get_dispense_position(source: &BlockSource) -> Vec3 {
    let direction = source.facing();
    source.center().add(0.7 * direction.step_x() as f64, 0.7 * direction.step_y() as f64, 0.7 * direction.step_z() as f64)
}

pub fn execute(level: &mut ServerLevel, source: &BlockSource, mut dispensed: ItemStack) -> ItemStack {
    let direction = source.facing();
    let position = get_dispense_position(source);
    let item_stack = dispensed.split(1);
    spawn_item(level, item_stack, DEFAULT_ACCURACY, direction, position);
    dispensed
}

pub fn spawn_item(level: &mut ServerLevel, item_stack: ItemStack, accuracy: i32, direction: Direction, position: Vec3) {
    let spawn_y = if direction.axis() == Axis::Y { position.y - 0.125 } else { position.y - 0.15625 };
    let random = &mut level.random;
    random.next_double();
    random.next_double();
    let pow = random.next_double() * 0.1 + 0.2;
    let spread = 0.0172275 * accuracy as f64;
    let delta = Vec3::new(
        random.triangle(direction.step_x() as f64 * pow, spread),
        random.triangle(0.2, spread),
        random.triangle(direction.step_z() as f64 * pow, spread),
    );
    let item_entity = ItemEntity::with_movement(Vec3::new(position.x, spawn_y, position.z), item_stack, delta);
    level.add_fresh_entity(LevelEntity::Item(item_entity));
}

pub fn play_default_sound(level: &ServerLevel, source: &BlockSource) {
    level.level_event(None, SOUND_DISPENSER_DISPENSE, source.pos, 0);
}

pub fn play_default_animation(level: &ServerLevel, source: &BlockSource, direction: Direction) {
    level.level_event(None, PARTICLES_SHOOT_SMOKE, source.pos, direction.ordinal() as i32);
}

pub fn consume_with_remainder(level: &mut ServerLevel, source: &BlockSource, mut dispensed: ItemStack, remainder: ItemStack) -> ItemStack {
    dispensed.shrink(1);
    if dispensed.is_empty() {
        return remainder;
    }
    add_to_inventory_or_dispense(level, source, remainder);
    dispensed
}

fn add_to_inventory_or_dispense(level: &mut ServerLevel, source: &BlockSource, item_stack: ItemStack) {
    let remainder = level.dispenser_insert_item(source.pos, item_stack);
    if !remainder.is_empty() {
        let direction = source.facing();
        spawn_item(level, remainder, DEFAULT_ACCURACY, direction, get_dispense_position(source));
        play_default_sound(level, source);
        play_default_animation(level, source, direction);
    }
}
