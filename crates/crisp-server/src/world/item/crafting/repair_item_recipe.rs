use super::crafting_input::CraftingInput;
use crate::world::item::ItemStack;
use crate::world::item::component as dc;
use crate::world::item::component::codec::registry_id;
use crate::world::item::component::values::ItemEnchantments;
use crate::world::item::enchantment::enchantment_helper::{get_component_type, update_enchantments};

pub struct RepairItemRecipe;

fn get_items_to_combine(input: &CraftingInput) -> Option<(&ItemStack, &ItemStack)> {
    if input.ingredient_count() != 2 {
        return None;
    }
    let mut first: Option<&ItemStack> = None;
    for i in 0..input.size() {
        let item_stack = input.get_item(i);
        if item_stack.is_empty() {
            continue;
        }
        match first {
            Some(first) => return can_combine(first, item_stack).then_some((first, item_stack)),
            None => first = Some(item_stack),
        }
    }
    None
}

fn can_combine(first: &ItemStack, second: &ItemStack) -> bool {
    second.is(first.get_item())
        && first.count == 1
        && second.count == 1
        && first.has(dc::MAX_DAMAGE)
        && second.has(dc::MAX_DAMAGE)
        && first.has(dc::DAMAGE)
        && second.has(dc::DAMAGE)
}

fn get_enchantments_for_crafting(item_stack: &ItemStack) -> ItemEnchantments {
    item_stack.get(get_component_type(item_stack)).map(ItemEnchantments::from_nbt).unwrap_or_default()
}

fn is_curse(enchantment: &str) -> bool {
    let curses = crisp_data::tag("minecraft:enchantment", "minecraft:curse");
    registry_id("minecraft:enchantment", enchantment).is_ok_and(|id| curses.contains(&id))
}

impl RepairItemRecipe {
    pub fn matches(&self, input: &CraftingInput) -> bool {
        get_items_to_combine(input).is_some()
    }

    pub fn assemble(&self, input: &CraftingInput) -> ItemStack {
        let Some((first, second)) = get_items_to_combine(input) else { return ItemStack::empty() };
        let durability = first.max_damage().max(second.max_damage());
        let remaining1 = first.max_damage() - first.damage_value();
        let remaining2 = second.max_damage() - second.damage_value();
        let remaining = remaining1 + remaining2 + durability * 5 / 100;
        let mut item_stack = ItemStack::new(first.get_item(), 1);
        item_stack.set_value(dc::MAX_DAMAGE, crisp_protocol::nbt::Nbt::Int(durability));
        item_stack.set_damage_value((durability - remaining).max(0));
        let first_enchants = get_enchantments_for_crafting(first);
        let second_enchants = get_enchantments_for_crafting(second);
        update_enchantments(&mut item_stack, |new_enchantments| {
            let union = first_enchants.enchantments.iter().map(|(id, _)| id).chain(second_enchants.enchantments.iter().map(|(id, _)| id).filter(|id| first_enchants.level(id) == 0));
            for enchantment in union {
                if is_curse(enchantment) {
                    let enchant_level = first_enchants.level(enchantment).max(second_enchants.level(enchantment));
                    new_enchantments.set(enchantment, enchant_level);
                }
            }
        });
        item_stack
    }
}
