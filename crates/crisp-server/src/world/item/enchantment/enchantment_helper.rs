use crisp_core::random::RandomSource;
use crisp_data::EquipmentSlot;

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
