use crisp_core::random::RandomSource;
use crisp_data::{EquipmentSlot, items};

use crate::world::item::component::values::ItemEnchantments;
use crate::world::item::component::{self as dc, DataComponentType};

use super::enchanted_item_in_use::EnchantedItemInUse;
use super::enchantment::Enchantment;
use super::targeted_conditional_effect::EnchantmentTarget;
use crate::server::level::ServerLevel;
use crate::world::damagesource::DamageSource;
use crate::world::entity::EntityId;
use crate::world::entity::attributes::{Attribute, AttributeModifier};
use crate::world::item::ItemStack;

fn enchantments(stack: &ItemStack) -> Vec<(&'static Enchantment, i32)> {
    stack.enchantments().enchantments.into_iter().filter_map(|(id, level)| Some((Enchantment::by_id(&id)?, level))).collect()
}

fn run_iteration_on_item(piece: &ItemStack, mut method: impl FnMut(&'static Enchantment, i32)) {
    for (enchantment, level) in enchantments(piece) {
        method(enchantment, level);
    }
}

pub fn equipment_slot_name(slot: EquipmentSlot) -> &'static str {
    match slot {
        EquipmentSlot::Mainhand => "mainhand",
        EquipmentSlot::Offhand => "offhand",
        EquipmentSlot::Feet => "feet",
        EquipmentSlot::Legs => "legs",
        EquipmentSlot::Chest => "chest",
        EquipmentSlot::Head => "head",
        EquipmentSlot::Body => "body",
        EquipmentSlot::Saddle => "saddle",
    }
}

pub fn for_each_modifier(item_stack: &ItemStack, slot: EquipmentSlot, mut consumer: impl FnMut(Attribute, AttributeModifier)) {
    run_iteration_on_item(item_stack, |enchantment, level| {
        for effect in &enchantment.effects.attributes {
            if enchantment.matching_slot(slot) {
                consumer(effect.attribute, effect.get_modifier(level, equipment_slot_name(slot)));
            }
        }
    });
}

pub fn has_prevent_armor_change(item_stack: &ItemStack) -> bool {
    enchantments(item_stack).iter().any(|(enchantment, _)| enchantment.effects.prevent_armor_change)
}

pub fn get_item_enchantment_level(enchantment: &str, piece: &ItemStack) -> i32 {
    piece.enchantments().level(enchantment)
}

pub fn has_tag(item_stack: &ItemStack, tag: &str) -> bool {
    let tag = crisp_data::tag("minecraft:enchantment", tag);
    item_stack
        .enchantments()
        .enchantments
        .iter()
        .any(|(enchantment, _)| crate::world::item::component::codec::registry_id("minecraft:enchantment", enchantment).is_ok_and(|id| tag.contains(&id)))
}

pub fn can_store_enchantments(item_stack: &ItemStack) -> bool {
    item_stack.has(get_component_type(item_stack))
}

pub fn set_enchantments(item_stack: &mut ItemStack, enchantments: &ItemEnchantments) {
    item_stack.set(get_component_type(item_stack), Some(enchantments.to_nbt()));
}

pub fn get_enchantments_for_crafting(item_stack: &ItemStack) -> ItemEnchantments {
    item_stack.get(get_component_type(item_stack)).map_or_else(ItemEnchantments::default, ItemEnchantments::from_nbt)
}

pub fn has_any_enchantments(item_stack: &ItemStack) -> bool {
    item_stack.get(dc::ENCHANTMENTS).is_some_and(|enchantments| !ItemEnchantments::from_nbt(enchantments).is_empty())
        || item_stack.get(dc::STORED_ENCHANTMENTS).is_some_and(|enchantments| !ItemEnchantments::from_nbt(enchantments).is_empty())
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

pub fn is_enchantment_compatible<'a>(enchants: impl IntoIterator<Item = &'a str>, target: &str) -> bool {
    enchants.into_iter().all(|existing| Enchantment::are_compatible(existing, target))
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

pub fn get_enchantment_cost(random: &mut impl RandomSource, slot: i32, mut bookcases: i32, item_stack: &ItemStack) -> i32 {
    if !item_stack.has(dc::ENCHANTABLE) {
        return 0;
    }
    if bookcases > 15 {
        bookcases = 15;
    }
    let selected = random.next_int_bounded(8) + 1 + (bookcases >> 1) + random.next_int_bounded(bookcases + 1);
    if slot == 0 {
        (selected / 3).max(1)
    } else if slot == 1 {
        selected * 2 / 3 + 1
    } else {
        selected.max(bookcases * 2)
    }
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

impl ServerLevel {
    fn run_iteration_on_equipment(&self, owner: EntityId) -> Vec<(&'static Enchantment, i32, EnchantedItemInUse)> {
        let mut entries = Vec::new();
        for slot in EquipmentSlot::VALUES {
            let Some(piece) = self.item_by_slot(owner, slot) else { continue };
            entries.extend(self.run_iteration_on_item_in_slot(&piece, slot, owner));
        }
        entries
    }

    fn run_iteration_on_item_in_slot(&self, piece: &ItemStack, slot: EquipmentSlot, owner: EntityId) -> Vec<(&'static Enchantment, i32, EnchantedItemInUse)> {
        if piece.is_empty() {
            return Vec::new();
        }
        enchantments(piece)
            .into_iter()
            .filter(|(enchantment, _)| enchantment.matching_slot(slot))
            .map(|(enchantment, level)| (enchantment, level, EnchantedItemInUse::new(piece.clone(), slot, owner)))
            .collect()
    }

    pub fn process_durability_change(&mut self, item_stack: &ItemStack, amount: i32) -> i32 {
        let mut modified_amount = amount as f32;
        run_iteration_on_item(item_stack, |enchantment, level| enchantment.modify_durability_change(self, level, item_stack, &mut modified_amount));
        modified_amount as i32
    }

    pub fn process_block_experience(&mut self, item_stack: &ItemStack, amount: i32) -> i32 {
        let mut modified_amount = amount as f32;
        run_iteration_on_item(item_stack, |enchantment, level| enchantment.modify_block_experience(self, level, item_stack, &mut modified_amount));
        modified_amount as i32
    }

    pub fn process_mob_experience(&mut self, killer: Option<EntityId>, amount: i32) -> i32 {
        let Some(killer) = killer.filter(|&k| self.is_living(k)) else { return amount };
        let mut modified_amount = amount as f32;
        for (enchantment, level, _) in self.run_iteration_on_equipment(killer) {
            enchantment.modify_mob_experience(self, level, killer, &mut modified_amount);
        }
        modified_amount as i32
    }

    pub fn modify_durability_to_repair_from_xp(&mut self, item_stack: &ItemStack, durability: i32) -> i32 {
        let mut modified_durability = durability as f32;
        run_iteration_on_item(item_stack, |enchantment, level| enchantment.modify_durability_to_repair_from_xp(self, level, item_stack, &mut modified_durability));
        (modified_durability as i32).max(0)
    }

    pub fn is_immune_to_damage(&mut self, victim: EntityId, source: &DamageSource) -> bool {
        let mut result = false;
        for (enchantment, level, _) in self.run_iteration_on_equipment(victim) {
            result = result || enchantment.is_immune_to_damage(self, level, victim, source);
        }
        result
    }

    pub fn get_damage_protection(&mut self, victim: EntityId, source: &DamageSource) -> f32 {
        let mut result = 0.0;
        for (enchantment, level, _) in self.run_iteration_on_equipment(victim) {
            enchantment.modify_damage_protection(self, level, victim, source, &mut result);
        }
        result
    }

    pub fn modify_damage(&mut self, item_stack: &ItemStack, victim: EntityId, source: &DamageSource, damage: f32) -> f32 {
        let mut result = damage;
        run_iteration_on_item(item_stack, |enchantment, level| enchantment.modify_damage(self, level, victim, source, &mut result));
        result
    }

    pub fn modify_fall_based_damage(&mut self, item_stack: &ItemStack, victim: EntityId, source: &DamageSource, damage: f32) -> f32 {
        let mut result = damage;
        run_iteration_on_item(item_stack, |enchantment, level| enchantment.modify_fall_based_damage(self, level, victim, source, &mut result));
        result
    }

    pub fn modify_armor_effectiveness(&mut self, item_stack: &ItemStack, victim: EntityId, source: &DamageSource, armor_fraction: f32) -> f32 {
        let mut result = armor_fraction;
        run_iteration_on_item(item_stack, |enchantment, level| enchantment.modify_armor_effectiveness(self, level, victim, source, &mut result));
        result
    }

    pub fn modify_knockback(&mut self, item_stack: &ItemStack, victim: EntityId, source: &DamageSource, knockback: f32) -> f32 {
        let mut result = knockback;
        run_iteration_on_item(item_stack, |enchantment, level| enchantment.modify_knockback(self, level, victim, source, &mut result));
        result
    }

    pub fn do_post_attack_effects(&mut self, victim: EntityId, source: &DamageSource) {
        let weapon = source.causing_entity.filter(|&attacker| self.is_living(attacker)).map(|attacker| self.get_weapon_item(attacker));
        self.do_post_attack_effects_with_item_source(victim, source, weapon.as_ref());
    }

    pub fn do_post_piercing_attack_effects(&mut self, user: EntityId) {
        let weapon = self.get_weapon_item(user);
        for (enchantment, level, item) in self.run_iteration_on_item_in_slot(&weapon, EquipmentSlot::Mainhand, user) {
            enchantment.do_post_piercing_attack(self, level, &item, user);
        }
    }

    pub fn do_post_attack_effects_with_item_source(&mut self, victim: EntityId, source: &DamageSource, item_source: Option<&ItemStack>) {
        if self.is_living(victim) {
            for (enchantment, level, item) in self.run_iteration_on_equipment(victim) {
                enchantment.do_post_attack(self, level, &item, EnchantmentTarget::Victim, victim, source);
            }
        }
        if let Some(item_source) = item_source
            && let Some(attacker) = source.causing_entity.filter(|&attacker| self.is_living(attacker))
        {
            for (enchantment, level, item) in self.run_iteration_on_item_in_slot(item_source, EquipmentSlot::Mainhand, attacker) {
                enchantment.do_post_attack(self, level, &item, EnchantmentTarget::Attacker, victim, source);
            }
        }
    }

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

    pub fn get_random_item_with(&mut self, component: &str, source: EntityId, predicate: impl Fn(&ItemStack) -> bool) -> Option<EnchantedItemInUse> {
        let mut items = Vec::new();
        for slot in EquipmentSlot::VALUES {
            let Some(item) = self.item_by_slot(source, slot).filter(|s| predicate(s)) else { continue };
            for (enchantment, _) in enchantments(&item) {
                if enchantment.effects.has(component) && enchantment.matching_slot(slot) {
                    items.push(EnchantedItemInUse::new(item.clone(), slot, source));
                }
            }
        }
        if items.is_empty() {
            return None;
        }
        let index = self.entity_mut(source).random.next_int_bounded(items.len() as i32);
        Some(items.swap_remove(index as usize))
    }
}
