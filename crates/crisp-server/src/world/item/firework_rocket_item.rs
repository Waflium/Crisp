use crisp_core::{Direction, Vec3};

use super::ItemStack;
use crate::core::dispenser::block_source::BlockSource;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::projectile::firework_rocket_entity::FireworkRocketEntity;
use crate::world::entity::projectile::projectile::Projectile;
use crate::world::level::block::BlockHitResult;
use crate::world::{InteractionHand, InteractionResult};

pub const ROCKET_PLACEMENT_OFFSET: f64 = 0.15;

pub fn use_on(level: &mut ServerLevel, player: EntityId, hand: InteractionHand, hit: BlockHitResult, mut item_stack: ItemStack) -> InteractionResult {
    if level.entity(player).is_fall_flying() {
        return InteractionResult::Pass;
    }
    let click_location = hit.location;
    let direction = hit.direction;
    let owner = level.entity(player).uuid;
    let projectile = FireworkRocketEntity::with_owner(
        Some(owner),
        click_location.x + direction.step_x() as f64 * ROCKET_PLACEMENT_OFFSET,
        click_location.y + direction.step_y() as f64 * ROCKET_PLACEMENT_OFFSET,
        click_location.z + direction.step_z() as f64 * ROCKET_PLACEMENT_OFFSET,
        &item_stack,
    );
    level.spawn_projectile(projectile, &item_stack, |_| {});
    item_stack.shrink(1);
    if !level.has_infinite_materials(player) {
        level.player_mut(player).update_item_in_hand(hand, &item_stack);
    }
    InteractionResult::SUCCESS
}

pub fn use_item(level: &mut ServerLevel, player: EntityId, hand: InteractionHand) -> (InteractionResult, Option<ItemStack>) {
    if !level.entity(player).is_fall_flying() {
        return (InteractionResult::Pass, None);
    }
    let Some(mut item_stack) = level.player(player).item_in_hand(hand) else { return (InteractionResult::Pass, None) };
    let item = item_stack.get_item();
    let projectile = FireworkRocketEntity::attached(level, &item_stack, player);
    level.spawn_projectile(projectile, &item_stack, |_| {});
    item_stack.consume(1, level, Some(player));
    level.award_stat(player, crate::stats::stats::ITEM_USED.get(item.0), 1);
    (InteractionResult::SUCCESS, Some(item_stack))
}

pub fn as_projectile(position: Vec3, item_stack: &ItemStack, _direction: Direction) -> Projectile {
    FireworkRocketEntity::shot_at_angle(&item_stack.copy_with_count(1), position.x, position.y, position.z, true)
}

pub fn get_entity_just_outside_of_block_pos(source: &BlockSource, direction: Direction) -> Vec3 {
    source.center().add(
        direction.step_x() as f64 * 0.5000099999997474,
        direction.step_y() as f64 * 0.5000099999997474,
        direction.step_z() as f64 * 0.5000099999997474,
    )
}
