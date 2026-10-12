use crisp_core::BlockPos;
use crisp_core::random::{LegacyRandomSource, RandomSource};
use crisp_data::{items, registry, registry_id};

use super::abstract_container_menu::{AbstractContainerMenu, MenuKind};
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;
use crate::world::item::component::codec;
use crate::world::item::enchantment::enchantment_helper::{self, EnchantmentInstance};
use crate::world::level::block::enchanting_table_block::{BOOKSHELF_OFFSETS, is_valid_book_shelf};

const ENCHANT_SLOTS: usize = 0;

pub struct EnchantmentMenu {
    random: LegacyRandomSource,
    enchantment_seed: i32,
    pub costs: [i32; 3],
    pub enchant_clue: [i32; 3],
    pub level_clue: [i32; 3],
}

impl EnchantmentMenu {
    fn new(enchantment_seed: i32) -> EnchantmentMenu {
        EnchantmentMenu { random: LegacyRandomSource::new(0), enchantment_seed, costs: [0; 3], enchant_clue: [-1; 3], level_clue: [-1; 3] }
    }

    pub fn get(&self, id: usize) -> i32 {
        match id {
            0..=2 => self.costs[id],
            3 => self.enchantment_seed,
            4..=6 => self.enchant_clue[id - 4],
            7..=9 => self.level_clue[id - 7],
            _ => 0,
        }
    }

    fn get_enchantment_list(&mut self, item_stack: &ItemStack, slot: usize, enchantment_cost: i32) -> Vec<EnchantmentInstance> {
        self.random.set_seed(self.enchantment_seed.wrapping_add(slot as i32) as i64);
        let names = codec::registry("minecraft:enchantment");
        let tag: Vec<&str> = crisp_data::tag("minecraft:enchantment", "minecraft:in_enchanting_table").iter().map(|&id| names[id as usize]).collect();
        let mut list = enchantment_helper::select_enchantment(&mut self.random, item_stack, enchantment_cost, &tag);
        if item_stack.is(items::BOOK) && list.len() > 1 {
            list.remove(self.random.next_int_bounded(list.len() as i32) as usize);
        }
        list
    }
}

impl AbstractContainerMenu {
    pub fn enchantment(container_id: i32, pos: BlockPos, enchantment_seed: i32) -> AbstractContainerMenu {
        let mut menu = AbstractContainerMenu::new(container_id, MenuKind::Enchantment, None);
        menu.access = Some(pos);
        menu.enchantment = Some(EnchantmentMenu::new(enchantment_seed));
        menu
    }
}

impl ServerLevel {
    pub fn enchantment_menu_slots_changed(&mut self, menu: &mut AbstractContainerMenu, player: EntityId) {
        let item_stack = menu.containers[ENCHANT_SLOTS].get_item(0).clone();
        if !item_stack.is_empty() && item_stack.is_enchantable() {
            let Some(pos) = menu.access else { return };
            let mut bookcases = 0;
            for &offset in BOOKSHELF_OFFSETS.iter() {
                if is_valid_book_shelf(self, pos, offset) {
                    bookcases += 1;
                }
            }
            let Some(state) = menu.enchantment.as_mut() else { return };
            state.random.set_seed(state.enchantment_seed as i64);
            for i in 0..3 {
                state.costs[i] = enchantment_helper::get_enchantment_cost(&mut state.random, i as i32, bookcases, &item_stack);
                state.enchant_clue[i] = -1;
                state.level_clue[i] = -1;
                if state.costs[i] < i as i32 + 1 {
                    state.costs[i] = 0;
                }
            }
            for i in 0..3 {
                if state.costs[i] > 0 {
                    let list = state.get_enchantment_list(&item_stack, i, state.costs[i]);
                    if !list.is_empty() {
                        let ench = &list[state.random.next_int_bounded(list.len() as i32) as usize];
                        state.enchant_clue[i] = codec::registry_id("minecraft:enchantment", &ench.enchantment).unwrap_or(-1);
                        state.level_clue[i] = ench.level;
                    }
                }
            }
            self.menu_broadcast_changes(menu, player);
        } else if let Some(state) = menu.enchantment.as_mut() {
            for i in 0..3 {
                state.costs[i] = 0;
                state.enchant_clue[i] = -1;
                state.level_clue[i] = -1;
            }
        }
    }

    pub fn enchantment_menu_click_menu_button(&mut self, menu: &mut AbstractContainerMenu, player: EntityId, button_id: i32) -> bool {
        if !(0..3).contains(&button_id) {
            error!("{} pressed invalid button id: {button_id}", self.player(player).profile.name);
            return false;
        }
        let button = button_id as usize;
        let item_stack = menu.containers[ENCHANT_SLOTS].get_item(0).clone();
        let mut currency = menu.containers[ENCHANT_SLOTS].get_item(1).clone();
        let enchantment_cost = button_id + 1;
        let has_infinite_materials = self.has_infinite_materials(player);
        if (currency.is_empty() || currency.count < enchantment_cost) && !has_infinite_materials {
            return false;
        }
        let Some(state) = menu.enchantment.as_mut() else { return false };
        let experience_level = self.player(player).experience.level;
        if state.costs[button] <= 0 || item_stack.is_empty() || (experience_level < enchantment_cost || experience_level < state.costs[button]) && !has_infinite_materials {
            return false;
        }
        let Some(pos) = menu.access else { return true };
        let new_enchantment = state.get_enchantment_list(&item_stack, button, state.costs[button]);
        if !new_enchantment.is_empty() {
            self.on_enchantment_performed(player, enchantment_cost);
            let mut enchantment_item = item_stack;
            if enchantment_item.is(items::BOOK) {
                enchantment_item = enchantment_item.transmute_copy(items::ENCHANTED_BOOK, enchantment_item.count);
                self.menu_container_set_item(menu, player, ENCHANT_SLOTS, 0, enchantment_item.clone());
            }
            for enchantment in &new_enchantment {
                enchantment_helper::enchant(&mut enchantment_item, &enchantment.enchantment, enchantment.level);
            }
            *menu.containers[ENCHANT_SLOTS].get_item_mut(0) = enchantment_item;
            currency.consume(enchantment_cost, self, Some(player));
            if currency.is_empty() {
                self.menu_container_set_item(menu, player, ENCHANT_SLOTS, 1, ItemStack::empty());
            } else {
                *menu.containers[ENCHANT_SLOTS].get_item_mut(1) = currency;
            }
            self.award_stat(player, crate::stats::stats::ENCHANT_ITEM, 1);
            self.menu_container_set_changed(menu, player, ENCHANT_SLOTS);
            let enchantment_seed = self.player(player).experience.enchantment_seed;
            if let Some(state) = menu.enchantment.as_mut() {
                state.enchantment_seed = enchantment_seed;
            }
            self.enchantment_menu_slots_changed(menu, player);
            let pitch = self.random.next_float() * 0.1 + 0.9;
            self.play_sound(None, pos, registry_id(registry::SOUND_EVENT, "minecraft:block.enchantment_table.use"), SoundSource::Blocks, 1.0, pitch);
        }
        true
    }

    pub fn enchantment_menu_removed(&mut self, menu: &mut AbstractContainerMenu, player: EntityId) {
        if menu.access.is_some() {
            self.clear_container(menu, player, ENCHANT_SLOTS);
        }
    }
}
