use crisp_core::{BlockPos, Direction};
use crisp_data::{BlockState, Item, items};
use crisp_protocol::nbt::Nbt;

use super::base_container_block_entity::{BaseContainerBlockEntity, save_all_items};
use super::{BlockEntityKind, float_value, int_or};
use crate::server::level::ServerLevel;
use crate::server::level::events::SOUND_BREWING_STAND_BREW;
use crate::world::item::ItemStack;
use crate::world::item::component as dc;
use crate::world::item::crafting::brewing_input::BrewingInput;
use crate::world::item::crafting::potion_ingredient::PotionIngredient;
use crate::world::item::crafting::recipe_holder::RecipeHolder;
use crate::world::item::crafting::recipe_manager::RecipeManager;
use crate::world::item::crafting::recipe_property_set::BREWING_REAGENTS;
use crate::world::level::block::brewing_stand_block::HAS_BOTTLE;
use crate::world::level::storage::loot::providers::number::floats::resolvable_float::ResolvableFloat;
use crate::world::level::storage::loot::providers::number::ints::resolvable_int::ResolvableInt;
use crate::world::level::update_flags::UPDATE_CLIENTS;

const INGREDIENT_SLOT: usize = 3;
const FUEL_SLOT: usize = 4;
const SLOTS_FOR_UP: &[usize] = &[3];
const SLOTS_FOR_DOWN: &[usize] = &[0, 1, 2, 3];
const SLOTS_FOR_SIDES: &[usize] = &[0, 1, 2, 4];
pub const DATA_BREW_TIME: usize = 0;
pub const DATA_FUEL_USES: usize = 1;
pub const DATA_TOTAL_BREW_TIME: usize = 2;
pub const DATA_TOTAL_FUEL_USES: usize = 3;

#[derive(Clone)]
pub struct BrewingStandBlockEntity {
    pub base: BaseContainerBlockEntity,
    brew_time: i32,
    total_brew_time: i32,
    last_potion_count: Option<[bool; 3]>,
    ingredient: Option<Item>,
    fuel: i32,
    total_fuel: i32,
    speed_multiplier: f32,
    last_recipe: Option<&'static str>,
}

impl Default for BrewingStandBlockEntity {
    fn default() -> BrewingStandBlockEntity {
        BrewingStandBlockEntity {
            base: BaseContainerBlockEntity::with_size(5),
            brew_time: 0,
            total_brew_time: 0,
            last_potion_count: None,
            ingredient: None,
            fuel: 0,
            total_fuel: 0,
            speed_multiplier: 1.0,
            last_recipe: None,
        }
    }
}

impl BrewingStandBlockEntity {
    pub fn data(&self, data_id: usize) -> i32 {
        match data_id {
            DATA_BREW_TIME => self.brew_time,
            DATA_FUEL_USES => self.fuel,
            DATA_TOTAL_BREW_TIME => self.total_brew_time,
            DATA_TOTAL_FUEL_USES => self.total_fuel,
            _ => 0,
        }
    }

    fn get_potion_bits(&self) -> [bool; 3] {
        std::array::from_fn(|potion| !self.base.items[potion].is_empty())
    }

    fn get_recipe_for(&mut self, input: &ItemStack, reagent: &ItemStack) -> Option<&'static RecipeHolder> {
        let found = RecipeManager::get().get_brewing_recipe_for(&BrewingInput { input, reagent }, self.last_recipe);
        if let Some(found) = found {
            self.last_recipe = Some(found.id);
        }
        found
    }

    fn is_brewable(&mut self) -> bool {
        let ingredient = self.base.items[INGREDIENT_SLOT].clone();
        if ingredient.is_empty() {
            return false;
        }
        if !RecipeManager::get().property_set(BREWING_REAGENTS).test(&ingredient) {
            return false;
        }
        for dest in 0..3 {
            let item_stack = self.base.items[dest].clone();
            if !item_stack.is_empty() && self.get_recipe_for(&item_stack, &ingredient).is_some() {
                return true;
            }
        }
        false
    }

    pub fn load_additional(&mut self, input: &Nbt) {
        self.base.load_additional(input);
        self.base.load_all_items(input, 5);
        self.brew_time = int_or(input, "BrewTime", 0);
        self.total_brew_time = int_or(input, "total_brew_time", 400);
        if self.brew_time > 0 {
            self.ingredient = Some(self.base.items[INGREDIENT_SLOT].item);
        }
        self.fuel = int_or(input, "Fuel", 0);
        self.total_fuel = int_or(input, "total_fuel", 20);
        self.speed_multiplier = input.get("speed_multiplier").and_then(float_value).unwrap_or(1.0);
    }

    pub fn save_additional(&self, output: &mut Vec<(String, Nbt)>) {
        self.base.save_additional(output);
        output.push(("BrewTime".into(), Nbt::Int(self.brew_time)));
        output.push(("total_brew_time".into(), Nbt::Int(self.total_brew_time)));
        save_all_items(output, &self.base.items, true);
        output.push(("Fuel".into(), Nbt::Int(self.fuel)));
        output.push(("total_fuel".into(), Nbt::Int(self.total_fuel)));
        output.push(("speed_multiplier".into(), Nbt::Float(self.speed_multiplier)));
    }

    pub fn can_place_item(&self, slot: usize, item_stack: &ItemStack) -> bool {
        if slot == FUEL_SLOT {
            return item_stack.has(dc::BREWING_FUEL);
        }
        if slot == INGREDIENT_SLOT {
            RecipeManager::get().property_set(BREWING_REAGENTS).test(item_stack)
        } else {
            PotionIngredient::is_potion_input(item_stack) && self.base.items[slot].is_empty()
        }
    }

    pub fn get_slots_for_face(direction: Direction) -> &'static [usize] {
        match direction {
            Direction::Up => SLOTS_FOR_UP,
            Direction::Down => SLOTS_FOR_DOWN,
            _ => SLOTS_FOR_SIDES,
        }
    }

    pub fn can_take_item_through_face(slot: usize, item_stack: &ItemStack) -> bool {
        if slot == INGREDIENT_SLOT { item_stack.is(items::GLASS_BOTTLE) } else { true }
    }
}

impl ServerLevel {
    fn brewing_stand(&mut self, pos: BlockPos) -> Option<&mut BrewingStandBlockEntity> {
        match &mut self.get_block_entity(pos)?.kind {
            BlockEntityKind::BrewingStand(brewing_stand) => Some(brewing_stand),
            _ => None,
        }
    }

    fn brewing_stand_get_uses(&mut self, pos: BlockPos, state: BlockState, fuel: &ItemStack) -> i32 {
        let block_entity = self.get_block_entity(pos).cloned();
        let mut context = self.get_loot_context(pos, state, block_entity.as_ref());
        ResolvableInt::get_from_item(fuel, dc::BREWING_FUEL, |brewing_fuel| brewing_fuel.get("uses"), &mut context, 0)
    }

    fn brewing_stand_get_speed_multiplier(&mut self, pos: BlockPos, state: BlockState, fuel: &ItemStack) -> f32 {
        let block_entity = self.get_block_entity(pos).cloned();
        let mut context = self.get_loot_context(pos, state, block_entity.as_ref());
        ResolvableFloat::get_from_item(fuel, dc::BREWING_FUEL, |brewing_fuel| brewing_fuel.get("speed_multiplier"), &mut context, 1.0)
    }

    pub fn brewing_stand_server_tick(&mut self, pos: BlockPos, self_state: BlockState) {
        let Some(entity) = self.brewing_stand(pos) else { return };
        let mut fuel = entity.base.items[FUEL_SLOT].clone();
        if entity.fuel <= 0 && fuel.has(dc::BREWING_FUEL) {
            let uses = self.brewing_stand_get_uses(pos, self_state, &fuel);
            let speed_multiplier = self.brewing_stand_get_speed_multiplier(pos, self_state, &fuel);
            let fuel_remainder = fuel.item.crafting_remainder().map(|item| ItemStack::new(item, 1));
            fuel.shrink(1);
            let mut new_fuel = fuel.clone();
            if let Some(fuel_remainder) = fuel_remainder {
                if fuel.is_empty() {
                    new_fuel = fuel_remainder;
                } else {
                    crate::world::containers::drop_item_stack(self, pos.x as f64, pos.y as f64, pos.z as f64, fuel_remainder);
                }
            }
            let Some(entity) = self.brewing_stand(pos) else { return };
            entity.fuel = uses;
            entity.total_fuel = uses;
            entity.speed_multiplier = speed_multiplier;
            entity.base.items[FUEL_SLOT] = new_fuel;
            self.block_entity_set_changed_static(pos, self_state);
        }
        let Some(entity) = self.brewing_stand(pos) else { return };
        let brewable = entity.is_brewable();
        let is_brewing = entity.brew_time > 0;
        let ingredient = entity.base.items[INGREDIENT_SLOT].clone();
        if is_brewing {
            entity.brew_time -= 1;
            let is_done_brewing = entity.brew_time == 0;
            if is_done_brewing && brewable {
                self.brewing_stand_do_brew(pos);
            } else if !brewable || Some(ingredient.item) != entity.ingredient {
                entity.brew_time = 0;
            }
            self.block_entity_set_changed_static(pos, self_state);
        } else if brewable && entity.fuel > 0 {
            let speed_multiplier = if entity.speed_multiplier > 0.0 { entity.speed_multiplier } else { 1.0 };
            entity.fuel -= 1;
            entity.brew_time = (400.0f32 / speed_multiplier).ceil() as i32;
            entity.total_brew_time = entity.brew_time;
            entity.ingredient = Some(ingredient.item);
            self.block_entity_set_changed_static(pos, self_state);
        }
        let Some(entity) = self.brewing_stand(pos) else { return };
        let new_count = entity.get_potion_bits();
        if entity.last_potion_count != Some(new_count) {
            entity.last_potion_count = Some(new_count);
            let mut state = self_state;
            if !state.block().is_instance_of("BrewingStandBlock") {
                return;
            }
            for (i, &has_bottle) in HAS_BOTTLE.iter().enumerate() {
                state = state.set(has_bottle, new_count[i]);
            }
            self.set_block(pos, state, UPDATE_CLIENTS);
        }
    }

    fn brewing_stand_do_brew(&mut self, pos: BlockPos) {
        let Some(entity) = self.brewing_stand(pos) else { return };
        let mut ingredient = entity.base.items[INGREDIENT_SLOT].clone();
        for dest in 0..3 {
            let container = entity.base.items[dest].clone();
            let recipe = entity.get_recipe_for(&container, &ingredient);
            entity.base.items[dest] = match recipe.and_then(RecipeHolder::brewing) {
                Some(recipe) => recipe.assemble(),
                None => container,
            };
        }
        let remainder = ingredient.item.crafting_remainder().map(|item| ItemStack::new(item, 1));
        ingredient.shrink(1);
        if let Some(remainder) = remainder {
            if ingredient.is_empty() {
                ingredient = remainder;
            } else {
                crate::world::containers::drop_item_stack(self, pos.x as f64, pos.y as f64, pos.z as f64, remainder);
            }
        }
        if let Some(entity) = self.brewing_stand(pos) {
            entity.base.items[INGREDIENT_SLOT] = ingredient;
        }
        self.level_event(None, SOUND_BREWING_STAND_BREW, pos, 0);
    }
}
