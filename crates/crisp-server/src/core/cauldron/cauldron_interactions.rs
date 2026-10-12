use crisp_core::BlockPos;
use crisp_data::properties::LEVEL_CAULDRON;
use crisp_data::{BlockState, Item, blocks, fluid_tags, item_tags, items, registry, registry_id};
use crisp_protocol::nbt::Nbt;

use crate::server::level::{ServerLevel, SoundSource};
use crate::stats::stats;
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;
use crate::world::item::component as dc;
use crate::world::item::item_utils::{create_filled_result, has_infinite_materials};
use crate::world::level::BlockGetter;
use crate::world::level::update_flags::UPDATE_ALL;
use crate::world::{InteractionHand, InteractionResult};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Dispatcher {
    Empty,
    Water,
    Lava,
    PowderSnow,
}

fn sound(name: &str) -> i32 {
    registry_id(registry::SOUND_EVENT, name)
}

pub fn is_water_potion(item_in_hand: &ItemStack) -> bool {
    match item_in_hand.get(dc::POTION_CONTENTS) {
        Some(Nbt::String(potion)) => potion == "minecraft:water",
        Some(contents) => contents.get("potion").and_then(Nbt::as_str) == Some("minecraft:water"),
        None => false,
    }
}

pub fn water_potion() -> ItemStack {
    let mut stack = ItemStack::new(items::POTION, 1);
    stack.set(dc::POTION_CONTENTS, Some(Nbt::Compound(vec![("potion".into(), Nbt::String("minecraft:water".into()))])));
    stack
}

fn set_item_in_hand(level: &mut ServerLevel, player: EntityId, hand: InteractionHand, stack: ItemStack) {
    level.player_mut(player).set_item_in_hand(hand, Some(stack));
}

pub fn lower_fill_level(state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
    let new_level = state.get(LEVEL_CAULDRON) - 1;
    let new_state = if new_level == 0 { blocks::CAULDRON.default_state() } else { state.set(LEVEL_CAULDRON, new_level) };
    level.set_block(pos, new_state, UPDATE_ALL);
}

impl Dispatcher {
    pub fn interact(self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, player: EntityId, hand: InteractionHand, item_in_hand: ItemStack) -> InteractionResult {
        if self == Dispatcher::Water && item_in_hand.item.is(&item_tags::CAULDRON_CAN_REMOVE_DYE) {
            return dyed_item_iteration(state, level, pos, player, hand, item_in_hand);
        }
        let item = item_in_hand.get_item();
        if item == items::LAVA_BUCKET {
            return fill_lava_interaction(level, pos, player, hand, item_in_hand);
        }
        if item == items::WATER_BUCKET {
            let new_state = blocks::WATER_CAULDRON.default_state().set(LEVEL_CAULDRON, 3);
            return empty_bucket(level, pos, player, hand, item_in_hand, new_state, "minecraft:item.bucket.empty");
        }
        if item == items::POWDER_SNOW_BUCKET {
            return fill_powder_snow_interaction(level, pos, player, hand, item_in_hand);
        }
        match self {
            Dispatcher::Empty if item == items::POTION => {
                if !is_water_potion(&item_in_hand) {
                    return InteractionResult::TryWithEmptyHand;
                }
                let used_item = item_in_hand.get_item();
                let result = create_filled_result(level, player, hand, item_in_hand, ItemStack::new(items::GLASS_BOTTLE, 1), true);
                set_item_in_hand(level, player, hand, result);
                level.award_stat(player, stats::USE_CAULDRON, 1);
                level.award_stat(player, stats::ITEM_USED.get(used_item.0), 1);
                level.set_block(pos, blocks::WATER_CAULDRON.default_state(), UPDATE_ALL);
                level.play_sound(None, pos, sound("minecraft:item.bottle.empty"), SoundSource::Blocks, 1.0, 1.0);
                InteractionResult::SUCCESS
            }
            Dispatcher::Water if item == items::BUCKET => {
                let can_fill = state.get(LEVEL_CAULDRON) == 3;
                fill_bucket(level, pos, player, hand, item_in_hand, ItemStack::new(items::WATER_BUCKET, 1), can_fill, "minecraft:item.bucket.fill")
            }
            Dispatcher::Water if item == items::GLASS_BOTTLE => {
                let used_item = item_in_hand.get_item();
                let result = create_filled_result(level, player, hand, item_in_hand, water_potion(), true);
                set_item_in_hand(level, player, hand, result);
                level.award_stat(player, stats::USE_CAULDRON, 1);
                level.award_stat(player, stats::ITEM_USED.get(used_item.0), 1);
                lower_fill_level(state, level, pos);
                level.play_sound(None, pos, sound("minecraft:item.bottle.fill"), SoundSource::Blocks, 1.0, 1.0);
                InteractionResult::SUCCESS
            }
            Dispatcher::Water if item == items::POTION => {
                if state.get(LEVEL_CAULDRON) == 3 || !is_water_potion(&item_in_hand) {
                    return InteractionResult::TryWithEmptyHand;
                }
                let consumed = if has_infinite_materials(level, player) { 0 } else { 1 };
                let item_in_hand_after = if item_in_hand.count - consumed > 0 { item_in_hand.get_item() } else { items::AIR };
                let result = create_filled_result(level, player, hand, item_in_hand, ItemStack::new(items::GLASS_BOTTLE, 1), true);
                set_item_in_hand(level, player, hand, result);
                level.award_stat(player, stats::USE_CAULDRON, 1);
                level.award_stat(player, stats::ITEM_USED.get(item_in_hand_after.0), 1);
                level.set_block(pos, state.cycle(LEVEL_CAULDRON), UPDATE_ALL);
                level.play_sound(None, pos, sound("minecraft:item.bottle.empty"), SoundSource::Blocks, 1.0, 1.0);
                InteractionResult::SUCCESS
            }
            Dispatcher::Water if item.is(&item_tags::BANNERS) => banner_interaction(state, level, pos, player, hand, item_in_hand),
            Dispatcher::Water if is_dyed_shulker_box(item) => shulker_box_interaction(state, level, pos, player, hand, item_in_hand),
            Dispatcher::Lava if item == items::BUCKET => {
                fill_bucket(level, pos, player, hand, item_in_hand, ItemStack::new(items::LAVA_BUCKET, 1), true, "minecraft:item.bucket.fill_lava")
            }
            Dispatcher::PowderSnow if item == items::BUCKET => {
                let can_fill = state.get(LEVEL_CAULDRON) == 3;
                fill_bucket(level, pos, player, hand, item_in_hand, ItemStack::new(items::POWDER_SNOW_BUCKET, 1), can_fill, "minecraft:item.bucket.fill_powder_snow")
            }
            _ => InteractionResult::TryWithEmptyHand,
        }
    }
}

fn is_dyed_shulker_box(item: Item) -> bool {
    item != items::SHULKER_BOX && item.info().block.is_some_and(|block| block.is_instance_of("ShulkerBoxBlock"))
}

#[allow(clippy::too_many_arguments)]
fn fill_bucket(
    level: &mut ServerLevel,
    pos: BlockPos,
    player: EntityId,
    hand: InteractionHand,
    item_in_hand: ItemStack,
    new_item: ItemStack,
    can_fill: bool,
    sound_event: &str,
) -> InteractionResult {
    if !can_fill {
        return InteractionResult::TryWithEmptyHand;
    }
    let item_used = item_in_hand.get_item();
    let result = create_filled_result(level, player, hand, item_in_hand, new_item, true);
    set_item_in_hand(level, player, hand, result);
    level.award_stat(player, stats::USE_CAULDRON, 1);
    level.award_stat(player, stats::ITEM_USED.get(item_used.0), 1);
    level.set_block(pos, blocks::CAULDRON.default_state(), UPDATE_ALL);
    level.play_sound(None, pos, sound(sound_event), SoundSource::Blocks, 1.0, 1.0);
    InteractionResult::SUCCESS
}

fn empty_bucket(level: &mut ServerLevel, pos: BlockPos, player: EntityId, hand: InteractionHand, item_in_hand: ItemStack, new_state: BlockState, sound_event: &str) -> InteractionResult {
    let item_used = item_in_hand.get_item();
    let result = create_filled_result(level, player, hand, item_in_hand, ItemStack::new(items::BUCKET, 1), true);
    set_item_in_hand(level, player, hand, result);
    level.award_stat(player, stats::FILL_CAULDRON, 1);
    level.award_stat(player, stats::ITEM_USED.get(item_used.0), 1);
    level.set_block(pos, new_state, UPDATE_ALL);
    level.play_sound(None, pos, sound(sound_event), SoundSource::Blocks, 1.0, 1.0);
    InteractionResult::SUCCESS
}

fn fill_lava_interaction(level: &mut ServerLevel, pos: BlockPos, player: EntityId, hand: InteractionHand, item_in_hand: ItemStack) -> InteractionResult {
    if is_under_water(level, pos) {
        InteractionResult::CONSUME
    } else {
        empty_bucket(level, pos, player, hand, item_in_hand, blocks::LAVA_CAULDRON.default_state(), "minecraft:item.bucket.empty_lava")
    }
}

fn fill_powder_snow_interaction(level: &mut ServerLevel, pos: BlockPos, player: EntityId, hand: InteractionHand, item_in_hand: ItemStack) -> InteractionResult {
    if is_under_water(level, pos) {
        InteractionResult::CONSUME
    } else {
        let new_state = blocks::POWDER_SNOW_CAULDRON.default_state().set(LEVEL_CAULDRON, 3);
        empty_bucket(level, pos, player, hand, item_in_hand, new_state, "minecraft:item.bucket.empty_powder_snow")
    }
}

fn shulker_box_interaction(state: BlockState, level: &mut ServerLevel, pos: BlockPos, player: EntityId, hand: InteractionHand, item_in_hand: ItemStack) -> InteractionResult {
    let cleaned_shulker_box = item_in_hand.transmute_copy(blocks::SHULKER_BOX.item(), 1);
    let result = create_filled_result(level, player, hand, item_in_hand, cleaned_shulker_box, false);
    set_item_in_hand(level, player, hand, result);
    level.award_stat(player, stats::CLEAN_SHULKER_BOX, 1);
    lower_fill_level(state, level, pos);
    InteractionResult::SUCCESS
}

fn banner_interaction(state: BlockState, level: &mut ServerLevel, pos: BlockPos, player: EntityId, hand: InteractionHand, item_in_hand: ItemStack) -> InteractionResult {
    let mut layers = match item_in_hand.get(dc::BANNER_PATTERNS) {
        Some(Nbt::List(layers)) => layers.clone(),
        _ => Vec::new(),
    };
    if layers.is_empty() {
        return InteractionResult::TryWithEmptyHand;
    }
    layers.pop();
    let mut cleaned_banner = item_in_hand.copy_with_count(1);
    cleaned_banner.set(dc::BANNER_PATTERNS, Some(Nbt::List(layers)));
    let result = create_filled_result(level, player, hand, item_in_hand, cleaned_banner, false);
    set_item_in_hand(level, player, hand, result);
    level.award_stat(player, stats::CLEAN_BANNER, 1);
    lower_fill_level(state, level, pos);
    InteractionResult::SUCCESS
}

fn dyed_item_iteration(state: BlockState, level: &mut ServerLevel, pos: BlockPos, player: EntityId, hand: InteractionHand, mut item_in_hand: ItemStack) -> InteractionResult {
    if !item_in_hand.has(dc::DYED_COLOR) {
        return InteractionResult::TryWithEmptyHand;
    }
    item_in_hand.remove(dc::DYED_COLOR);
    level.player_mut(player).update_item_in_hand(hand, &item_in_hand);
    level.award_stat(player, stats::CLEAN_ARMOR, 1);
    lower_fill_level(state, level, pos);
    InteractionResult::SUCCESS
}

fn is_under_water(level: &mut ServerLevel, pos: BlockPos) -> bool {
    level.fluid_state(pos.above()).is(&fluid_tags::WATER)
}
