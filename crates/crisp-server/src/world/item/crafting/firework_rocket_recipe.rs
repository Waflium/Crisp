use crisp_protocol::nbt::Nbt;

use super::crafting_input::CraftingInput;
use super::ingredient::Ingredient;
use crate::util::codec::{self, DataResult};
use crate::world::item::ItemStack;
use crate::world::item::component::values::ItemStackTemplate;
use crate::world::item::component::{self as dc, DataComponentPatch};

pub struct FireworkRocketRecipe {
    shell: Ingredient,
    fuel: Ingredient,
    star: Ingredient,
    result: ItemStackTemplate,
}

impl FireworkRocketRecipe {
    pub fn decode(tag: &Nbt) -> DataResult<FireworkRocketRecipe> {
        Ok(FireworkRocketRecipe {
            shell: codec::decode_field(tag, "shell", Ingredient::decode)?,
            fuel: codec::decode_field(tag, "fuel", Ingredient::decode)?,
            star: codec::decode_field(tag, "star", Ingredient::decode)?,
            result: codec::decode_field(tag, "result", ItemStackTemplate::decode)?,
        })
    }

    pub fn matches(&self, input: &CraftingInput) -> bool {
        if input.ingredient_count() < 2 {
            return false;
        }
        let mut has_shell = false;
        let mut fuel_count = 0;
        for slot in 0..input.size() {
            let item_stack = input.get_item(slot);
            if item_stack.is_empty() {
                continue;
            }
            if self.shell.test(item_stack) {
                if has_shell {
                    return false;
                }
                has_shell = true;
            } else if self.fuel.test(item_stack) {
                fuel_count += 1;
                if fuel_count > 3 {
                    return false;
                }
            } else if !self.star.test(item_stack) {
                return false;
            }
        }
        has_shell && fuel_count >= 1
    }

    pub fn assemble(&self, input: &CraftingInput) -> ItemStack {
        let mut explosions = Vec::new();
        let mut fuel_count = 0;
        for slot in 0..input.size() {
            let item_stack = input.get_item(slot);
            if item_stack.is_empty() {
                continue;
            }
            if self.fuel.test(item_stack) {
                fuel_count += 1;
            } else if self.star.test(item_stack)
                && let Some(explosion) = item_stack.get(dc::FIREWORK_EXPLOSION)
            {
                explosions.push(explosion.clone());
            }
        }
        let fireworks = Nbt::Compound(vec![("flight_duration".into(), Nbt::Byte(fuel_count as i8)), ("explosions".into(), Nbt::List(explosions))]);
        let mut components = DataComponentPatch::default();
        components.put(dc::FIREWORKS, Some(dc::FIREWORKS.decode(&fireworks).unwrap_or(fireworks)));
        self.result.apply(self.result.count, &components)
    }
}
