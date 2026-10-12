use crisp_core::BlockPos;
use crisp_core::random::RandomSource;
use crisp_data::{block_tags, items};
use crisp_protocol::nbt::Nbt;

use super::abstract_container_menu::{AbstractContainerMenu, MenuKind};
use super::item_combiner_menu::{INPUT_SLOTS, RESULT_SLOTS};
use crate::server::level::ServerLevel;
use crate::server::level::events::{SOUND_ANVIL_BROKEN, SOUND_ANVIL_USED};
use crate::util::string_util::{filter_text, is_blank};
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;
use crate::world::item::component as dc;
use crate::world::item::component::values::ItemEnchantments;
use crate::world::item::enchantment::enchantment::Enchantment;
use crate::world::item::enchantment::enchantment_helper::get_component_type;
use crate::world::level::BlockGetter;
use crate::world::level::block::anvil_block::damage;
use crate::world::level::update_flags::UPDATE_CLIENTS;

pub const INPUT_SLOT: usize = 0;
pub const ADDITIONAL_SLOT: usize = 1;
pub const RESULT_SLOT: usize = 2;
pub const MAX_NAME_LENGTH: usize = 50;

#[derive(Default)]
pub struct AnvilMenu {
    repair_item_count_cost: i32,
    item_name: Option<String>,
    cost: i32,
    only_renaming: bool,
}

impl AnvilMenu {
    pub fn get_cost(&self) -> i32 {
        self.cost
    }
}

pub fn calculate_increased_repair_cost(base_cost: i32) -> i32 {
    (base_cost as i64 * 2 + 1).min(i32::MAX as i64) as i32
}

fn validate_name(name: &str) -> Option<String> {
    let filtered_name = filter_text(name);
    (filtered_name.encode_utf16().count() <= MAX_NAME_LENGTH).then_some(filtered_name)
}

fn can_store_enchantments(item_stack: &ItemStack) -> bool {
    item_stack.has(get_component_type(item_stack))
}

fn get_enchantments_for_crafting(item_stack: &ItemStack) -> ItemEnchantments {
    item_stack.get(get_component_type(item_stack)).map_or_else(ItemEnchantments::default, ItemEnchantments::from_nbt)
}

fn set_enchantments(item_stack: &mut ItemStack, enchantments: &ItemEnchantments) {
    item_stack.set(get_component_type(item_stack), Some(enchantments.to_nbt()));
}

fn repair_cost(item_stack: &ItemStack) -> i32 {
    item_stack.get_int(dc::REPAIR_COST, 0)
}

impl AbstractContainerMenu {
    pub fn anvil(container_id: i32, pos: BlockPos) -> AbstractContainerMenu {
        let mut menu = AbstractContainerMenu::new(container_id, MenuKind::Anvil, None);
        menu.access = Some(pos);
        menu.anvil = Some(AnvilMenu::default());
        menu
    }

    fn anvil_state(&mut self) -> &mut AnvilMenu {
        self.anvil.as_mut().expect("an anvil menu")
    }

    fn set_result(&mut self, result: ItemStack) {
        *self.containers[RESULT_SLOTS].get_item_mut(0) = result;
    }
}

impl ServerLevel {
    pub fn anvil_menu_may_pickup(&mut self, menu: &AbstractContainerMenu, player: EntityId, _has_item: bool) -> bool {
        let cost = menu.anvil.as_ref().map_or(0, AnvilMenu::get_cost);
        (self.player(player).abilities.instabuild || self.player(player).experience.level >= cost) && cost > 0
    }

    pub fn anvil_menu_on_take(&mut self, menu: &mut AbstractContainerMenu, player: EntityId, _carried: &ItemStack) {
        let has_infinite_materials = self.player(player).abilities.instabuild;
        if !has_infinite_materials {
            let cost = menu.anvil_state().cost;
            self.give_experience_levels(player, -cost);
        }
        let repair_item_count_cost = menu.anvil_state().repair_item_count_cost;
        if repair_item_count_cost > 0 {
            let mut addition = menu.containers[INPUT_SLOTS].get_item(ADDITIONAL_SLOT).clone();
            if !addition.is_empty() && addition.count > repair_item_count_cost {
                addition.shrink(repair_item_count_cost);
                self.menu_container_set_item(menu, player, INPUT_SLOTS, ADDITIONAL_SLOT, addition);
            } else {
                self.menu_container_set_item(menu, player, INPUT_SLOTS, ADDITIONAL_SLOT, ItemStack::empty());
            }
        } else if !menu.anvil_state().only_renaming {
            self.menu_container_set_item(menu, player, INPUT_SLOTS, ADDITIONAL_SLOT, ItemStack::empty());
        }
        menu.anvil_state().cost = 0;
        self.menu_container_set_item(menu, player, INPUT_SLOTS, INPUT_SLOT, ItemStack::empty());
        if let Some(pos) = menu.access {
            let state = self.block_state(pos);
            if !has_infinite_materials && state.block().is(&block_tags::ANVIL) && self.entity_mut(player).random.next_float() < 0.12 {
                match damage(state) {
                    None => {
                        self.remove_block(pos, false);
                        self.level_event(None, SOUND_ANVIL_BROKEN, pos, 0);
                    }
                    Some(new_block_state) => {
                        self.set_block(pos, new_block_state, UPDATE_CLIENTS);
                        self.level_event(None, SOUND_ANVIL_USED, pos, 0);
                    }
                }
            } else {
                self.level_event(None, SOUND_ANVIL_USED, pos, 0);
            }
        }
    }

    pub fn anvil_menu_create_result(&mut self, menu: &mut AbstractContainerMenu, player: EntityId) {
        let input = menu.containers[INPUT_SLOTS].get_item(INPUT_SLOT).clone();
        let has_infinite_materials = self.player(player).abilities.instabuild;
        let anvil = menu.anvil_state();
        anvil.only_renaming = false;
        anvil.cost = 1;
        let mut price = 0i32;
        let mut tax = 0i64;
        let mut naming_cost = 0;
        if input.is_empty() || !can_store_enchantments(&input) {
            menu.set_result(ItemStack::empty());
            menu.anvil_state().cost = 0;
            return;
        }
        let mut result = input.copy();
        let addition = menu.containers[INPUT_SLOTS].get_item(ADDITIONAL_SLOT).clone();
        let mut enchantments = get_enchantments_for_crafting(&result);
        tax += repair_cost(&input) as i64 + repair_cost(&addition) as i64;
        let anvil = menu.anvil_state();
        anvil.repair_item_count_cost = 0;
        if !addition.is_empty() {
            let using_book = addition.has(dc::STORED_ENCHANTMENTS);
            if result.is_damageable_item() && input.is_valid_repair_item(&addition) {
                let mut repair_amount = result.damage_value().min(result.max_damage() / 4);
                if repair_amount <= 0 {
                    menu.set_result(ItemStack::empty());
                    menu.anvil_state().cost = 0;
                    return;
                }
                let mut count = 0;
                while repair_amount > 0 && count < addition.count {
                    let result_damage = result.damage_value() - repair_amount;
                    result.set_damage_value(result_damage);
                    price += 1;
                    repair_amount = result.damage_value().min(result.max_damage() / 4);
                    count += 1;
                }
                anvil.repair_item_count_cost = count;
            } else {
                if !using_book && (!result.is(addition.get_item()) || !result.is_damageable_item()) {
                    menu.set_result(ItemStack::empty());
                    menu.anvil_state().cost = 0;
                    return;
                }
                if result.is_damageable_item() && !using_book {
                    let remaining1 = input.max_damage() - input.damage_value();
                    let remaining2 = addition.max_damage() - addition.damage_value();
                    let additional = remaining2 + result.max_damage() * 12 / 100;
                    let remaining = remaining1 + additional;
                    let result_damage = (result.max_damage() - remaining).max(0);
                    if result_damage < result.damage_value() {
                        result.set_damage_value(result_damage);
                        price += 2;
                    }
                }
                let additional_enchantments = get_enchantments_for_crafting(&addition);
                let mut is_any_enchantment_compatible = false;
                let mut is_any_enchantment_not_compatible = false;
                for (enchantment_holder, level) in &additional_enchantments.enchantments {
                    let current = enchantments.level(enchantment_holder);
                    let mut level = if current == *level { level + 1 } else { (*level).max(current) };
                    let Some(enchantment) = Enchantment::by_id(enchantment_holder) else { continue };
                    let mut compatible = enchantment.can_enchant(&input);
                    if has_infinite_materials || input.is(items::ENCHANTED_BOOK) {
                        compatible = true;
                    }
                    for (other, _) in &enchantments.enchantments {
                        if other != enchantment_holder && !Enchantment::are_compatible(enchantment_holder, other) {
                            compatible = false;
                            price += 1;
                        }
                    }
                    if !compatible {
                        is_any_enchantment_not_compatible = true;
                    } else {
                        is_any_enchantment_compatible = true;
                        if level > enchantment.max_level() {
                            level = enchantment.max_level();
                        }
                        enchantments.set(enchantment_holder, level);
                        let mut fee = enchantment.get_anvil_cost();
                        if using_book {
                            fee = 1.max(fee / 2);
                        }
                        price += fee * level;
                        if input.count > 1 {
                            price = 40;
                        }
                    }
                }
                if is_any_enchantment_not_compatible && !is_any_enchantment_compatible {
                    menu.set_result(ItemStack::empty());
                    menu.anvil_state().cost = 0;
                    return;
                }
            }
        }
        let anvil = menu.anvil_state();
        if let Some(item_name) = anvil.item_name.as_ref().filter(|item_name| !is_blank(Some(item_name))) {
            if *item_name != input.hover_name().get_string() {
                naming_cost = 1;
                price += naming_cost;
                result.set(dc::CUSTOM_NAME, Some(Nbt::String(item_name.clone())));
            }
        } else if input.has(dc::CUSTOM_NAME) {
            naming_cost = 1;
            price += naming_cost;
            result.remove(dc::CUSTOM_NAME);
        }
        let final_price = if price <= 0 { 0 } else { (tax + price as i64).clamp(0, i32::MAX as i64) as i32 };
        anvil.cost = final_price;
        if price <= 0 {
            result = ItemStack::empty();
        }
        if naming_cost == price && naming_cost > 0 {
            if anvil.cost >= 40 {
                anvil.cost = 39;
            }
            anvil.only_renaming = true;
        }
        if anvil.cost >= 40 && !has_infinite_materials {
            result = ItemStack::empty();
        }
        if !result.is_empty() {
            let base_cost = repair_cost(&result).max(repair_cost(&addition));
            let base_cost = if naming_cost != price || naming_cost == 0 { calculate_increased_repair_cost(base_cost) } else { base_cost };
            result.set(dc::REPAIR_COST, Some(Nbt::Int(base_cost)));
            set_enchantments(&mut result, &enchantments);
        }
        menu.set_result(result);
        self.menu_broadcast_changes(menu, player);
    }

    pub fn anvil_menu_set_item_name(&mut self, menu: &mut AbstractContainerMenu, player: EntityId, name: &str) -> bool {
        let Some(validated_name) = validate_name(name) else { return false };
        if menu.anvil_state().item_name.as_ref() == Some(&validated_name) {
            return false;
        }
        menu.anvil_state().item_name = Some(validated_name.clone());
        if self.slot_has_item(menu, player, RESULT_SLOT) {
            let mut item_stack = self.slot_get(menu, player, RESULT_SLOT);
            if is_blank(Some(&validated_name)) {
                item_stack.remove(dc::CUSTOM_NAME);
            } else {
                item_stack.set(dc::CUSTOM_NAME, Some(Nbt::String(validated_name)));
            }
            self.slot_put(menu, player, RESULT_SLOT, item_stack);
        }
        self.anvil_menu_create_result(menu, player);
        true
    }
}
