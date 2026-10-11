use crisp_core::random::RandomSource;
use crisp_data::{EquipmentSlot, items};

use crate::world::item::component::values::ItemEnchantments;
use crate::world::item::component::{self as dc, DataComponentType};

use super::enchantment::{Enchantment, components};
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;

fn enchantments(stack: &ItemStack) -> impl Iterator<Item = (&'static Enchantment, i32)> {
    stack.enchantments().enchantments.into_iter().filter_map(|(id, level)| Some((Enchantment::by_id(&id)?, level)))
}

fn modify_item_filtered_count(component: &str, stack: &ItemStack, random: &mut impl RandomSource, value: f32) -> f32 {
    enchantments(stack).fold(value, |value, (enchantment, level)| enchantment.modify_value(component, level, random, value))
}

pub fn get_item_enchantment_level(enchantment: &str, piece: &ItemStack) -> i32 {
    piece.enchantments().level(enchantment)
}

pub fn get_component_type(item_stack: &ItemStack) -> DataComponentType {
    if item_stack.is(items::ENCHANTED_BOOK) { dc::STORED_ENCHANTMENTS } else { dc::ENCHANTMENTS }
}

pub fn update_enchantments(item_stack: &mut ItemStack, consumer: impl FnOnce(&mut ItemEnchantments)) -> ItemEnchantments {
    let component_type = get_component_type(item_stack);
    let Some(old_enchantments) = item_stack.get(component_type) else { return ItemEnchantments::default() };
    let mut mutable = ItemEnchantments::from_nbt(old_enchantments);
    consumer(&mut mutable);
    item_stack.set(component_type, Some(mutable.to_nbt()));
    mutable
}

pub fn enchant(item_stack: &mut ItemStack, enchantment: &str, level: i32) {
    update_enchantments(item_stack, |enchantments| enchantments.upgrade(enchantment, level));
}

pub struct EnchantmentInstance {
    pub enchantment: String,
    pub level: i32,
}

impl EnchantmentInstance {
    fn weight(&self) -> i32 {
        Enchantment::by_id(&self.enchantment).map_or(0, Enchantment::weight)
    }
}

fn get_random_item(random: &mut impl RandomSource, items: &[EnchantmentInstance]) -> Option<usize> {
    let total_weight: i64 = items.iter().map(|i| i.weight() as i64).sum();
    if total_weight > i32::MAX as i64 {
        panic!("Sum of weights must be <= 2147483647");
    }
    if total_weight == 0 {
        return None;
    }
    let mut selection = random.next_int_bounded(total_weight as i32);
    for (index, item) in items.iter().enumerate() {
        selection -= item.weight();
        if selection < 0 {
            return Some(index);
        }
    }
    None
}

pub fn enchant_item(random: &mut impl RandomSource, mut item_stack: ItemStack, enchantment_cost: i32, source: &[&str]) -> ItemStack {
    let enchants = select_enchantment(random, &item_stack, enchantment_cost, source);
    if item_stack.is(items::BOOK) {
        item_stack = ItemStack::new(items::ENCHANTED_BOOK, 1);
    }
    for enchant_instance in enchants {
        enchant(&mut item_stack, &enchant_instance.enchantment, enchant_instance.level);
    }
    item_stack
}

pub fn select_enchantment(random: &mut impl RandomSource, item_stack: &ItemStack, mut enchantment_cost: i32, source: &[&str]) -> Vec<EnchantmentInstance> {
    let mut results: Vec<EnchantmentInstance> = Vec::new();
    let Some(enchantable) = item_stack.get(dc::ENCHANTABLE).and_then(|e| e.get("value")).map(crate::world::item::component::codec::as_i32) else { return results };
    enchantment_cost += 1 + random.next_int_bounded(enchantable / 4 + 1) + random.next_int_bounded(enchantable / 4 + 1);
    let random_span = (random.next_float() + random.next_float() - 1.0) * 0.15;
    enchantment_cost = crisp_core::mth::clamp_i(crate::util::java_round_f(enchantment_cost as f32 + enchantment_cost as f32 * random_span), 1, i32::MAX);
    let mut enchantments = get_available_enchantment_results(enchantment_cost, item_stack, source);
    if !enchantments.is_empty() {
        if let Some(index) = get_random_item(random, &enchantments) {
            results.push(EnchantmentInstance { enchantment: enchantments[index].enchantment.clone(), level: enchantments[index].level });
        }
        while random.next_int_bounded(50) <= enchantment_cost {
            if let Some(last) = results.last() {
                filter_compatible_enchantments(&mut enchantments, last);
            }
            if enchantments.is_empty() {
                break;
            }
            if let Some(index) = get_random_item(random, &enchantments) {
                results.push(EnchantmentInstance { enchantment: enchantments[index].enchantment.clone(), level: enchantments[index].level });
            }
            enchantment_cost /= 2;
        }
    }
    results
}

pub fn filter_compatible_enchantments(enchants: &mut Vec<EnchantmentInstance>, target: &EnchantmentInstance) {
    enchants.retain(|e| Enchantment::are_compatible(&target.enchantment, &e.enchantment));
}

pub fn get_available_enchantment_results(value: i32, item_stack: &ItemStack, source: &[&str]) -> Vec<EnchantmentInstance> {
    let mut results = Vec::new();
    let is_book = item_stack.is(items::BOOK);
    for &id in source {
        let Some(enchantment) = Enchantment::by_id(id) else { continue };
        if !(enchantment.is_primary_item(item_stack) || is_book) {
            continue;
        }
        let mut level = enchantment.max_level();
        while level >= enchantment.min_level() {
            if value >= enchantment.min_cost(level) && value <= enchantment.max_cost(level) {
                results.push(EnchantmentInstance { enchantment: id.to_string(), level });
                break;
            }
            level -= 1;
        }
    }
    results
}

pub fn process_block_experience(random: &mut impl RandomSource, tool: Option<&ItemStack>, amount: i32) -> i32 {
    match tool {
        Some(tool) => modify_item_filtered_count(components::BLOCK_EXPERIENCE, tool, random, amount as f32) as i32,
        None => amount,
    }
}

pub fn modify_durability_to_repair_from_xp(random: &mut impl RandomSource, stack: &ItemStack, durability: i32) -> i32 {
    (modify_item_filtered_count(components::REPAIR_WITH_XP, stack, random, durability as f32) as i32).max(0)
}

impl ServerLevel {
    pub fn get_enchantment_level(&self, enchantment: &str, entity: EntityId) -> i32 {
        let Some(definition) = Enchantment::by_id(enchantment) else { return 0 };
        let mut best_level = 0;
        for slot in EquipmentSlot::VALUES {
            if !definition.matching_slot(slot) {
                continue;
            }
            if let Some(piece) = self.item_by_slot(entity, slot) {
                best_level = best_level.max(get_item_enchantment_level(enchantment, &piece));
            }
        }
        best_level
    }

    pub fn process_mob_experience(&mut self, killer: Option<EntityId>, amount: i32) -> i32 {
        let Some(killer) = killer.filter(|&k| self.is_player(k)) else { return amount };
        let mut value = amount as f32;
        for slot in EquipmentSlot::VALUES {
            let Some(stack) = self.item_by_slot(killer, slot) else { continue };
            for (enchantment, level) in enchantments(&stack).filter(|(e, _)| e.matching_slot(slot)) {
                value = enchantment.modify_value(components::MOB_EXPERIENCE, level, &mut self.random, value);
            }
        }
        value as i32
    }

    pub fn random_item_with(&mut self, component: &str, player: EntityId, predicate: impl Fn(&ItemStack) -> bool) -> Option<EquipmentSlot> {
        let mut items = Vec::new();
        for slot in EquipmentSlot::VALUES {
            let Some(stack) = self.item_by_slot(player, slot).filter(|s| predicate(s)) else { continue };
            for (enchantment, _) in enchantments(&stack) {
                if enchantment.has_effect(component) && enchantment.matching_slot(slot) {
                    items.push(slot);
                }
            }
        }
        if items.is_empty() {
            return None;
        }
        let index = self.entity_mut(player).random.next_int_bounded(items.len() as i32);
        Some(items[index as usize])
    }
}
