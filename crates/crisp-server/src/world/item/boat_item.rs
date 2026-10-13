use crate::server::level::ServerLevel;
use crate::world::entity::entity_save::apply_default_stack_config;
use crate::world::entity::{EntityId, LevelEntity};
use crate::world::item::ItemStack;
use crate::world::item::bucket_item::player_pov_hit_result;
use crate::world::level::block::BlockHitResult;
use crate::world::level::clip::ClipFluid;
use crate::world::{InteractionHand, InteractionResult};

pub fn use_item(level: &mut ServerLevel, player: EntityId, hand: InteractionHand) -> (InteractionResult, Option<ItemStack>) {
    let Some(mut item_stack) = level.player(player).item_in_hand(hand) else { return (InteractionResult::Pass, None) };
    let Some(hit_result) = player_pov_hit_result(level, player, ClipFluid::Any) else { return (InteractionResult::Pass, None) };
    let p = level.entity(player);
    let view_vector = p.view_vector();
    let area = p.bounding_box().expand_towards_vec(view_vector.scale(5.0)).inflate_all(1.0);
    let from = p.eye_position();
    for entity in level.get_entities(Some(player), &area) {
        if level.is_pickable(entity) && level.entity(entity).bounding_box().inflate_all(level.pick_radius(entity) as f64).contains_vec(from) {
            return (InteractionResult::Pass, None);
        }
    }
    let Some(mut boat) = get_boat(&hit_result, &item_stack) else { return (InteractionResult::Fail, None) };
    let y_rot = level.entity(player).y_rot;
    boat.entity_mut().set_y_rot(y_rot);
    let boat_box = boat.entity().bounding_box();
    if !level.no_collision_for_new(&boat, &boat_box) {
        return (InteractionResult::Fail, None);
    }
    let this = item_stack.item;
    level.add_fresh_entity(boat);
    item_stack.consume(1, level, Some(player));
    level.award_stat(player, crate::stats::stats::ITEM_USED.get(this.0), 1);
    (InteractionResult::SUCCESS, Some(item_stack))
}

fn get_boat(hit_result: &BlockHitResult, item_stack: &ItemStack) -> Option<LevelEntity> {
    let mut entity = ServerLevel::create_entity(item_stack.item.name())?;
    let LevelEntity::Boat(boat) = &mut entity else { return None };
    let location = hit_result.location;
    boat.set_initial_pos(location.x, location.y, location.z);
    apply_default_stack_config(&mut entity, item_stack);
    Some(entity)
}
