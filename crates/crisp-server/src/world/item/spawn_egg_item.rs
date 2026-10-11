use crisp_core::{BlockPos, Direction, Vec3};
use crisp_data::{registry, registry_id};

use super::bucket_item::{may_use_item_at, player_pov_hit_result};
use crate::server::level::ServerLevel;
use crate::world::InteractionHand;
use crate::world::InteractionResult;
use crate::world::entity::{EntityId, LevelEntity};
use crate::world::item::ItemStack;
use crate::world::item::component::{self as dc, values};
use crate::world::level::BlockGetter;
use crate::world::level::block::BlockHitResult;
use crate::world::level::clip::ClipFluid;

pub fn use_on(level: &mut ServerLevel, player: EntityId, hand: InteractionHand, hit: BlockHitResult, mut item_stack: ItemStack) -> InteractionResult {
    let Some(entity_type) = get_type(&item_stack) else { return InteractionResult::Fail };
    let pos = hit.block_pos;
    let clicked_face = hit.direction;
    let block_state = level.block_state(pos);
    let spawn_pos = if block_state.collision_voxel_shape().is_empty() { pos } else { pos.relative(clicked_face) };
    let moved_up = pos != spawn_pos && clicked_face == Direction::Up;
    let result = spawn_mob(level, entity_type, Some(player), &mut item_stack, spawn_pos, true, moved_up);
    level.player_mut(player).update_item_in_hand(hand, &item_stack);
    result
}

fn spawn_mob(level: &mut ServerLevel, entity_type: i32, user: Option<EntityId>, item_stack: &mut ItemStack, spawn_pos: BlockPos, try_move_down: bool, moved_up: bool) -> InteractionResult {
    let name = registry::ENTITY_TYPE[entity_type as usize];
    if level.spawn_entity_type_from_stack(name, Some(item_stack), spawn_pos, try_move_down, moved_up).is_some() {
        item_stack.consume(1, level, user);
        InteractionResult::SUCCESS
    } else {
        InteractionResult::Fail
    }
}

pub fn use_item(level: &mut ServerLevel, player: EntityId, hand: InteractionHand) -> (InteractionResult, Option<ItemStack>) {
    let Some(mut item_stack) = level.player(player).item_in_hand(hand) else { return (InteractionResult::Pass, None) };
    let Some(hit) = player_pov_hit_result(level, player, ClipFluid::SourceOnly) else { return (InteractionResult::Pass, None) };
    let Some(entity_type) = get_type(&item_stack) else { return (InteractionResult::Fail, None) };
    let pos = hit.block_pos;
    if !level.block_state(pos).block().is_instance_of("LiquidBlock") {
        return (InteractionResult::Pass, None);
    }
    if level.may_interact(player, pos) && may_use_item_at(level, player) {
        let result = spawn_mob(level, entity_type, Some(player), &mut item_stack, pos, false, false);
        (result, Some(item_stack))
    } else {
        (InteractionResult::Fail, None)
    }
}

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
