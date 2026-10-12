use crisp_core::Vec3;
use crisp_data::items;

use super::abstract_container_menu::{PICKUP, QUICK_MOVE};
use crate::network::hashed_stack::HashedStack;
use crate::server::level::{ServerLevel, testing};
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;

fn level_with_player() -> (ServerLevel, EntityId) {
    let mut level = testing::level();
    let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 3.5), 180.0, 30.0);
    (level, player)
}

fn click(level: &mut ServerLevel, player: EntityId, slot: i32, button: i32, input: i32) {
    let state_id = level.player(player).inventory_menu.as_ref().unwrap().state_id();
    level.handle_container_menu_click(player, state_id, slot, button, input, Vec::new(), HashedStack::Empty).unwrap();
}

fn menu_slot(level: &mut ServerLevel, player: EntityId, slot: usize) -> ItemStack {
    let menu = level.player_mut(player).inventory_menu.take().unwrap();
    let stack = level.slot_get(&menu, player, slot);
    level.player_mut(player).inventory_menu = Some(menu);
    stack
}

#[test]
fn taking_the_result_consumes_the_grid() {
    let (mut level, player) = level_with_player();
    level.player_mut(player).inventory.slots[36] = Some(ItemStack::new(items::OAK_LOG, 2));
    click(&mut level, player, 36, 0, PICKUP);
    click(&mut level, player, 1, 0, PICKUP);
    assert_eq!(menu_slot(&mut level, player, 0), ItemStack::new(items::OAK_PLANKS, 4));
    click(&mut level, player, 0, 0, PICKUP);
    assert_eq!(level.player(player).carried, Some(ItemStack::new(items::OAK_PLANKS, 4)));
    assert_eq!(menu_slot(&mut level, player, 1), ItemStack::new(items::OAK_LOG, 1));
    level.player_mut(player).carried = None;
    click(&mut level, player, 0, 0, QUICK_MOVE);
    assert!(menu_slot(&mut level, player, 1).is_empty());
    assert_eq!(menu_slot(&mut level, player, 44), ItemStack::new(items::OAK_PLANKS, 4));
}

#[test]
fn shift_click_equips_helmet() {
    let (mut level, player) = level_with_player();
    level.player_mut(player).inventory.slots[36] = Some(ItemStack::new(items::IRON_HELMET, 1));
    click(&mut level, player, 36, 0, QUICK_MOVE);
    assert_eq!((menu_slot(&mut level, player, 5), menu_slot(&mut level, player, 36)), (ItemStack::new(items::IRON_HELMET, 1), ItemStack::empty()));
}

#[test]
fn right_click_splits_and_places_one() {
    let (mut level, player) = level_with_player();
    level.player_mut(player).inventory.slots[36] = Some(ItemStack::new(items::DIRT, 5));
    click(&mut level, player, 36, 1, PICKUP);
    assert_eq!((level.player(player).carried.clone(), menu_slot(&mut level, player, 36)), (Some(ItemStack::new(items::DIRT, 3)), ItemStack::new(items::DIRT, 2)));
    click(&mut level, player, 37, 1, PICKUP);
    assert_eq!((level.player(player).carried.clone(), menu_slot(&mut level, player, 37)), (Some(ItemStack::new(items::DIRT, 2)), ItemStack::new(items::DIRT, 1)));
    click(&mut level, player, 5, 0, PICKUP);
    assert!(menu_slot(&mut level, player, 5).is_empty());
}

#[test]
fn closing_the_inventory_returns_the_grid() {
    let (mut level, player) = level_with_player();
    level.player_mut(player).inventory.slots[36] = Some(ItemStack::new(items::DIRT, 1));
    click(&mut level, player, 36, 0, PICKUP);
    click(&mut level, player, 2, 0, PICKUP);
    assert_eq!(menu_slot(&mut level, player, 2), ItemStack::new(items::DIRT, 1));
    level.do_close_container(player);
    assert!(menu_slot(&mut level, player, 2).is_empty());
    assert!(level.player(player).inventory.slots.iter().flatten().any(|stack| *stack == ItemStack::new(items::DIRT, 1)));
}
