use crisp_core::{BlockPos, Direction, Vec3};
use crisp_data::properties::LIT;
use crisp_data::{BlockState, items};
use crisp_protocol::nbt::Nbt;

use super::base_container_block_entity::{BaseContainerBlockEntity, save_all_items};
use super::{BlockEntity, BlockEntityKind, float_value, int_or, int_value};
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;
use crate::world::item::component as dc;
use crate::world::item::crafting::{self, CookingRecipeType};
use crate::world::level::storage::loot::loot_context::{LootContext, LootContextBuilder};
use crate::world::level::storage::loot::loot_params::{LootParams, SlotProvider};
use crate::world::level::storage::loot::providers::number::floats::resolvable_float::ResolvableFloat;
use crate::world::level::storage::loot::providers::number::ints::resolvable_int::ResolvableInt;
use crate::world::level::update_flags::UPDATE_ALL;

pub const SLOT_INPUT: usize = 0;
pub const SLOT_FUEL: usize = 1;
pub const SLOT_RESULT: usize = 2;
const SLOTS_FOR_UP: &[usize] = &[0];
const SLOTS_FOR_DOWN: &[usize] = &[2, 1];
const SLOTS_FOR_SIDES: &[usize] = &[1];
pub const BURN_TIME_STANDARD: i32 = 200;

#[derive(Clone)]
pub struct AbstractFurnaceBlockEntity {
    pub base: BaseContainerBlockEntity,
    pub recipe_type: CookingRecipeType,
    pub lit_time_remaining: i32,
    pub lit_total_time: i32,
    pub cooking_timer: i32,
    pub cooking_total_time: i32,
    pub speed_multiplier: f32,
    pub recipes_used: Vec<(String, i32)>,
    last_recipe: Option<usize>,
}

impl AbstractFurnaceBlockEntity {
    pub fn new(recipe_type: CookingRecipeType) -> AbstractFurnaceBlockEntity {
        AbstractFurnaceBlockEntity {
            base: BaseContainerBlockEntity::with_size(3),
            recipe_type,
            lit_time_remaining: 0,
            lit_total_time: 0,
            cooking_timer: 0,
            cooking_total_time: 0,
            speed_multiplier: 1.0,
            recipes_used: Vec::new(),
            last_recipe: None,
        }
    }

    pub fn load_additional(&mut self, input: &Nbt) {
        self.base.load_additional(input);
        self.base.load_all_items(input, 3);
        self.cooking_timer = int_or(input, "cooking_time_spent", 0);
        self.cooking_total_time = int_or(input, "cooking_total_time", 0);
        self.lit_time_remaining = int_or(input, "lit_time_remaining", 0);
        self.lit_total_time = int_or(input, "lit_total_time", 0);
        self.speed_multiplier = input.get("speed_multiplier").and_then(float_value).unwrap_or(1.0);
        self.recipes_used = match input.get("RecipesUsed") {
            Some(Nbt::Compound(fields)) => fields
                .iter()
                .map(|(k, v)| Some((crate::commands::arguments::parse_identifier(k)?, int_value(v)?)))
                .collect::<Option<Vec<_>>>()
                .unwrap_or_default(),
            _ => Vec::new(),
        };
    }

    pub fn save_additional(&self, output: &mut Vec<(String, Nbt)>) {
        self.base.save_additional(output);
        output.push(("cooking_time_spent".into(), Nbt::Int(self.cooking_timer)));
        output.push(("cooking_total_time".into(), Nbt::Int(self.cooking_total_time)));
        output.push(("lit_time_remaining".into(), Nbt::Int(self.lit_time_remaining)));
        output.push(("lit_total_time".into(), Nbt::Int(self.lit_total_time)));
        output.push(("speed_multiplier".into(), Nbt::Float(self.speed_multiplier)));
        save_all_items(output, &self.base.items, true);
        output.push(("RecipesUsed".into(), Nbt::Compound(self.recipes_used.iter().map(|(k, v)| (k.clone(), Nbt::Int(*v))).collect())));
    }

    pub fn data(&self, id: usize) -> i32 {
        match id {
            0 => self.lit_time_remaining,
            1 => self.lit_total_time,
            2 => self.cooking_timer,
            3 => self.cooking_total_time,
            _ => 0,
        }
    }

    fn get_recipe_for(&mut self, input: &ItemStack) -> Option<usize> {
        let found = crafting::get_recipe_for(self.recipe_type, input, self.last_recipe);
        if found.is_some() {
            self.last_recipe = found;
        }
        found
    }

    fn total_cook_time_of(&self, recipe: usize) -> i32 {
        let time = crafting::cooking_recipes()[recipe].cooking_time;
        if self.speed_multiplier > 0.0 { (time as f32 / self.speed_multiplier).ceil() as i32 } else { time }
    }

    pub fn total_cook_time(&mut self) -> i32 {
        let input = self.base.items[SLOT_INPUT].clone();
        match self.get_recipe_for(&input) {
            Some(recipe) => self.total_cook_time_of(recipe),
            None => BURN_TIME_STANDARD,
        }
    }

    pub fn set_item(&mut self, slot: usize, mut stack: ItemStack) -> bool {
        let old = &self.base.items[slot];
        let same = !stack.is_empty() && ItemStack::is_same_item_same_components(old, &stack);
        stack.limit_size(stack.max_stack_size().min(99));
        self.base.items[slot] = stack;
        if slot == SLOT_INPUT && !same {
            self.cooking_total_time = self.total_cook_time();
            self.cooking_timer = 0;
            return true;
        }
        false
    }

    pub fn can_place_item(&self, slot: usize, stack: &ItemStack) -> bool {
        match slot {
            SLOT_RESULT => false,
            SLOT_FUEL => {
                let fuel_slot = &self.base.items[SLOT_FUEL];
                stack.has(dc::COOKING_FUEL) || stack.is(items::BUCKET) && !fuel_slot.is(items::BUCKET)
            }
            _ => true,
        }
    }

    pub fn slots_for_face(direction: Direction) -> &'static [usize] {
        match direction {
            Direction::Down => SLOTS_FOR_DOWN,
            Direction::Up => SLOTS_FOR_UP,
            _ => SLOTS_FOR_SIDES,
        }
    }

    pub fn can_take_item_through_face(slot: usize, stack: &ItemStack, direction: Direction) -> bool {
        if direction == Direction::Down && slot == SLOT_FUEL { stack.get_item().is(&crisp_data::item_tags::FURNACE_FUEL_BOTTOM_TAKEABLE) } else { true }
    }

    fn set_recipe_used(&mut self, recipe: usize) {
        let id = crafting::cooking_recipes()[recipe].id;
        match self.recipes_used.iter_mut().find(|(k, _)| k == id) {
            Some((_, count)) => *count += 1,
            None => self.recipes_used.push((id.to_string(), 1)),
        }
    }
}

fn can_burn(items: &[ItemStack], max_stack_size: i32, burn_result: &ItemStack) -> bool {
    let result = &items[SLOT_RESULT];
    if result.is_empty() {
        return true;
    }
    if !ItemStack::is_same_item_same_components(result, burn_result) {
        return false;
    }
    result.count + burn_result.count <= max_stack_size.min(burn_result.max_stack_size())
}

fn burn(items: &mut [ItemStack], result: &ItemStack) {
    if items[SLOT_RESULT].is_empty() {
        items[SLOT_RESULT] = result.copy();
    } else {
        items[SLOT_RESULT].grow(result.count);
    }
    if items[SLOT_INPUT].is(items::WET_SPONGE) && !items[SLOT_FUEL].is_empty() && items[SLOT_FUEL].is(items::BUCKET) {
        items[SLOT_FUEL] = ItemStack::new(items::WATER_BUCKET, 1);
    }
    items[SLOT_INPUT].shrink(1);
}

impl ServerLevel {
    fn furnace(&mut self, pos: BlockPos) -> Option<&mut AbstractFurnaceBlockEntity> {
        match &mut self.get_block_entity(pos)?.kind {
            BlockEntityKind::Furnace(furnace) => Some(furnace),
            _ => None,
        }
    }

    fn get_loot_context<'a>(&'a mut self, pos: BlockPos, state: BlockState, block_entity: Option<&'a BlockEntity>) -> LootContext<'a> {
        let mut params = LootParams::new(self);
        params.block_state = Some(state);
        params.block_entity = block_entity;
        params.origin = Some(Vec3::at_center_of(pos));
        params.container = block_entity.map(SlotProvider::BlockEntity);
        LootContextBuilder::new(params).create(None)
    }

    fn get_burn_duration(&mut self, pos: BlockPos, state: BlockState, fuel_item: &ItemStack) -> i32 {
        let block_entity = self.get_block_entity(pos).cloned();
        let mut context = self.get_loot_context(pos, state, block_entity.as_ref());
        ResolvableInt::get_from_item(fuel_item, dc::COOKING_FUEL, |fuel| fuel.get("burn_time"), &mut context, 0)
    }

    fn get_speed_multiplier(&mut self, pos: BlockPos, state: BlockState, fuel_item: &ItemStack) -> f32 {
        let block_entity = self.get_block_entity(pos).cloned();
        let mut context = self.get_loot_context(pos, state, block_entity.as_ref());
        ResolvableFloat::get_from_item(fuel_item, dc::COOKING_FUEL, |fuel| fuel.get("speed_multiplier"), &mut context, 1.0)
    }

    pub fn furnace_server_tick(&mut self, pos: BlockPos, mut state: BlockState) {
        let Some(mut entity) = self.furnace(pos) else { return };
        let mut changed = false;
        let (was_lit, mut is_lit) = if entity.lit_time_remaining > 0 {
            entity.lit_time_remaining -= 1;
            (true, entity.lit_time_remaining > 0)
        } else {
            (false, false)
        };
        let fuel = entity.base.items[SLOT_FUEL].clone();
        let ingredient = entity.base.items[SLOT_INPUT].clone();
        let has_ingredient = !ingredient.is_empty();
        let has_fuel = !fuel.is_empty();
        let mut remainder_to_drop = None;
        if is_lit || has_fuel && has_ingredient {
            if has_ingredient {
                if let Some(recipe) = entity.get_recipe_for(&ingredient) {
                    let burn_result = crafting::cooking_recipes()[recipe].assemble();
                    if !burn_result.is_empty() && can_burn(&entity.base.items, 99, &burn_result) {
                        if !is_lit {
                            let new_lit_time = self.get_burn_duration(pos, state, &fuel);
                            let new_speed_multiplier = self.get_speed_multiplier(pos, state, &fuel);
                            entity = self.furnace(pos).expect("a furnace");
                            entity.lit_time_remaining = new_lit_time;
                            entity.lit_total_time = new_lit_time;
                            entity.speed_multiplier = new_speed_multiplier;
                            if entity.cooking_total_time > 0 && entity.cooking_timer < entity.cooking_total_time {
                                let completion_ratio = entity.cooking_timer as f32 / entity.cooking_total_time as f32;
                                entity.cooking_total_time = entity.total_cook_time_of(recipe);
                                entity.cooking_timer = (completion_ratio * entity.cooking_total_time as f32).ceil() as i32;
                            }
                            if new_lit_time > 0 {
                                let mut new_fuel = fuel.clone();
                                new_fuel.shrink(1);
                                if let Some(remainder) = fuel.item.crafting_remainder().map(|item| ItemStack::new(item, 1)) {
                                    if new_fuel.is_empty() {
                                        new_fuel = remainder;
                                    } else {
                                        remainder_to_drop = Some(remainder);
                                    }
                                }
                                entity.base.items[SLOT_FUEL] = new_fuel;
                                is_lit = true;
                                changed = true;
                            }
                        }
                        if is_lit {
                            entity.cooking_timer += 1;
                            if entity.cooking_timer >= entity.cooking_total_time {
                                entity.cooking_timer = 0;
                                entity.cooking_total_time = entity.total_cook_time_of(recipe);
                                burn(&mut entity.base.items, &burn_result);
                                entity.set_recipe_used(recipe);
                                changed = true;
                            }
                        } else {
                            entity.cooking_timer = 0;
                        }
                    } else {
                        entity.cooking_timer = 0;
                    }
                }
            } else {
                entity.cooking_timer = 0;
            }
        } else if entity.cooking_timer > 0 {
            entity.cooking_timer = (entity.cooking_timer - 2).clamp(0, entity.cooking_total_time.max(0));
        }
        if let Some(remainder) = remainder_to_drop {
            crate::world::containers::drop_item_stack(self, pos.x as f64, pos.y as f64, pos.z as f64, remainder);
        }
        if was_lit != is_lit {
            changed = true;
            state = state.set(LIT, is_lit);
            self.set_block(pos, state, UPDATE_ALL);
        }
        if changed {
            self.block_entity_set_changed_static(pos, state);
        }
    }

    pub fn get_recipes_to_award_and_pop_experience(&mut self, pos: BlockPos, position: Vec3) {
        let Some(furnace) = self.furnace(pos) else { return };
        let used = furnace.recipes_used.clone();
        for (id, amount) in used {
            if let Some(recipe) = crafting::by_key(&id) {
                let total = amount as f32 * recipe.experience;
                let mut xp_reward = crisp_core::mth::floor(total as f64);
                let fraction = crisp_core::mth::frac_f(total);
                if fraction != 0.0 && crisp_core::random::RandomSource::next_float(&mut self.random) < fraction {
                    xp_reward += 1;
                }
                self.award_experience(position, xp_reward);
            }
        }
    }

    pub fn award_used_recipes_and_pop_experience(&mut self, pos: BlockPos, player: EntityId) {
        let position = self.player(player).entity.position();
        self.get_recipes_to_award_and_pop_experience(pos, position);
        if let Some(furnace) = self.furnace(pos) {
            furnace.recipes_used.clear();
        }
    }
}
