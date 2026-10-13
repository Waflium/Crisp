use super::block_source::BlockSource;
use crate::server::level::ServerLevel;
use crate::server::level::events::SOUND_DISPENSER_PROJECTILE_LAUNCH;
use crate::world::item::ItemStack;
use crate::world::item::projectile_item;

pub fn execute(level: &mut ServerLevel, source: &BlockSource, mut dispensed: ItemStack) -> ItemStack {
    let dispense_config = projectile_item::create_dispense_config(dispensed.get_item());
    let direction = source.facing();
    let position = dispense_config.position_function.get_dispense_position(source, direction);
    let projectile = projectile_item::as_projectile(level, position, &dispensed, direction);
    level.spawn_projectile_using_shoot(
        projectile,
        &dispensed,
        direction.step_x() as f64,
        direction.step_y() as f64,
        direction.step_z() as f64,
        dispense_config.power,
        dispense_config.uncertainty,
    );
    dispensed.shrink(1);
    dispensed
}

pub fn play_sound(level: &ServerLevel, source: &BlockSource, dispensed: &ItemStack) {
    let dispense_config = projectile_item::create_dispense_config(dispensed.get_item());
    level.level_event(None, dispense_config.override_dispense_event.unwrap_or(SOUND_DISPENSER_PROJECTILE_LAUNCH), source.pos, 0);
}
