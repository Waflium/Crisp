use crisp_data::{registry, registry_id};

use super::map_item;
use crate::server::level::{ServerLevel, SoundSource};
use crate::util::Prediction;
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;
use crate::world::{InteractionHand, InteractionResult};

pub fn use_item(level: &mut ServerLevel, player: EntityId, hand: InteractionHand) -> (InteractionResult, Option<ItemStack>) {
    let Some(mut item_stack) = level.player(player).item_in_hand(hand) else { return (InteractionResult::Pass, None) };
    let this = item_stack.item;
    item_stack.consume(1, level, Some(player));
    level.player_mut(player).set_item_in_hand(hand, Some(item_stack.clone()));
    level.award_stat(player, crate::stats::stats::ITEM_USED.get(this.0), 1);
    let sound = registry_id(registry::SOUND_EVENT, "minecraft:ui.cartography_table.take_result");
    level.play_sound_from_entity(None, player, sound, SoundSource::Players, 1.0, 1.0);
    let block_pos = level.player(player).entity.block_position();
    let map = map_item::create(level, block_pos.x, block_pos.z, 0, true, false);
    if item_stack.is_empty() {
        return (InteractionResult::SUCCESS, Some(map));
    }
    let infinite_materials = level.has_infinite_materials(player);
    let mut copy = map.copy();
    if !level.player_mut(player).inventory.add_stack(&mut copy, infinite_materials).0 {
        level.player_drop(player, map, false, Prediction::ServerOnly);
    }
    (InteractionResult::SUCCESS, None)
}
