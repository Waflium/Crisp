use crisp_core::Direction;
use crisp_data::registry;

use super::block_source::BlockSource;
use crate::server::level::ServerLevel;
use crate::world::item::ItemStack;
use crate::world::item::spawn_egg_item;

pub fn execute(level: &mut ServerLevel, source: &BlockSource, mut dispensed: ItemStack) -> ItemStack {
    let direction = source.facing();
    let Some(entity_type) = spawn_egg_item::get_type(&dispensed) else { return dispensed };
    let name = registry::ENTITY_TYPE[entity_type as usize];
    if level.spawn_entity_type_from_stack(name, Some(&dispensed), source.pos.relative(direction), direction != Direction::Up, false).is_none() {
        return dispensed;
    }
    dispensed.shrink(1);
    dispensed
}
