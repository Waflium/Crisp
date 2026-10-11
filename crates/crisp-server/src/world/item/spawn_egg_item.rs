use crisp_core::Vec3;
use crisp_data::{registry, registry_id};

use crate::server::level::ServerLevel;
use crate::world::entity::{EntityId, LevelEntity};
use crate::world::item::ItemStack;
use crate::world::item::component::{self as dc, values};

pub fn spawns_entity(item_stack: &ItemStack, entity_type: i32) -> bool {
    get_type(item_stack) == Some(entity_type)
}

pub fn get_type(item_stack: &ItemStack) -> Option<i32> {
    let entity_data = item_stack.get(dc::ENTITY_DATA)?;
    Some(registry_id(registry::ENTITY_TYPE, &values::typed_entity_data(entity_data).0))
}

pub fn spawn_offspring_from_spawn_egg(level: &mut ServerLevel, player: EntityId, parent: EntityId, entity_type: i32, pos: Vec3, spawn_egg_stack: &mut ItemStack) -> Option<EntityId> {
    if !spawns_entity(spawn_egg_stack, entity_type) {
        return None;
    }
    let mut offspring = if level.mob(parent).is_some_and(|mob| mob.ageable().is_some()) {
        level.get_breed_offspring(parent, parent)?
    } else {
        let LevelEntity::Mob(mob) = ServerLevel::create_entity(registry::ENTITY_TYPE[entity_type as usize])? else { return None };
        mob
    };
    offspring.set_baby(true);
    if !offspring.is_baby() {
        return None;
    }
    offspring.entity.snap_to(pos.x, pos.y, pos.z, 0.0, 0.0);
    offspring.apply_components_from_item_stack(spawn_egg_stack);
    let id = offspring.entity.id;
    level.add_fresh_entity(LevelEntity::Mob(offspring));
    spawn_egg_stack.consume(1, level, Some(player));
    Some(id)
}
