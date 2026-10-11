use crate::server::level::ServerLevel;
use crate::world::InteractionResult;
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;
use crate::world::item::component as dc;

pub fn interact_living_entity(level: &mut ServerLevel, item_stack: &mut ItemStack, _player: EntityId, target: EntityId) -> InteractionResult {
    let Some(custom_name) = item_stack.get(dc::CUSTOM_NAME).cloned() else { return InteractionResult::Pass };
    if level.is_player(target) {
        return InteractionResult::Pass;
    }
    if level.is_alive(target) {
        level.entity_mut(target).set_custom_name(Some(custom_name));
        if let Some(mob) = level.mob_mut(target) {
            mob.persistence_required = true;
        }
        item_stack.shrink(1);
    }
    InteractionResult::SUCCESS
}
