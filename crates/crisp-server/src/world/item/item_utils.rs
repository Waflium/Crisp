use crate::server::level::ServerLevel;
use crate::server::level::game_mode::GameType;
use crate::world::InteractionHand;
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;

pub fn has_infinite_materials(level: &ServerLevel, player: EntityId) -> bool {
    level.player(player).game_mode.game_type == GameType::Creative
}

pub fn create_filled_result(
    level: &mut ServerLevel,
    player: EntityId,
    hand: InteractionHand,
    mut item_stack: ItemStack,
    new_item_stack: ItemStack,
    limit_creative_stack_size: bool,
) -> ItemStack {
    let is_creative = has_infinite_materials(level, player);
    if limit_creative_stack_size && is_creative {
        let p = level.player_mut(player);
        if !p.inventory.slots.iter().flatten().any(|s| !s.is_empty() && ItemStack::is_same_item_same_components(s, &new_item_stack)) {
            p.pick_up(new_item_stack);
        }
        return item_stack;
    }
    if !is_creative {
        item_stack.count -= 1;
    }
    let p = level.player_mut(player);
    p.set_item_in_hand(hand, Some(item_stack.clone()));
    if item_stack.is_empty() {
        return new_item_stack;
    }
    let left = p.pick_up(new_item_stack.clone());
    if left > 0 {
        level.player_drop(player, new_item_stack.copy_with_count(left), false, crate::util::Prediction::Predicted);
    }
    item_stack
}
