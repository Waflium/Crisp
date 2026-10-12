use crisp_core::BlockPos;
use crisp_data::BlockState;
use crisp_protocol::nbt::Nbt;

use super::BlockEntityKind;
use super::base_container_block_entity::{copy_into, from_items, load_all_items, save_all_items};
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;
use crate::world::item::component::{self as dc, DataComponentType};
use crate::world::item::crafting::{self, CookingRecipeType};
use crate::world::item::item_utils::has_infinite_materials;

const BURN_COOL_SPEED: i32 = 2;
const NUM_SLOTS: usize = 4;

#[derive(Clone)]
pub struct CampfireBlockEntity {
    items: Vec<ItemStack>,
    cooking_progress: [i32; NUM_SLOTS],
    cooking_time: [i32; NUM_SLOTS],
}

impl Default for CampfireBlockEntity {
    fn default() -> CampfireBlockEntity {
        CampfireBlockEntity { items: vec![ItemStack::empty(); NUM_SLOTS], cooking_progress: [0; NUM_SLOTS], cooking_time: [0; NUM_SLOTS] }
    }
}

fn int_array(input: &Nbt, key: &str) -> Option<Vec<i32>> {
    match input.get(key)? {
        Nbt::IntArray(values) => Some(values.clone()),
        _ => None,
    }
}

impl CampfireBlockEntity {
    pub fn get_items(&self) -> &[ItemStack] {
        &self.items
    }

    pub fn load_additional(&mut self, input: &Nbt) {
        self.items = vec![ItemStack::empty(); NUM_SLOTS];
        load_all_items(input, &mut self.items);
        match int_array(input, "CookingTimes") {
            Some(cooking_times) => {
                let n = self.cooking_time.len().min(cooking_times.len());
                self.cooking_progress[..n].copy_from_slice(&cooking_times[..n]);
            }
            None => self.cooking_progress = [0; NUM_SLOTS],
        }
        match int_array(input, "CookingTotalTimes") {
            Some(cooking_times) => {
                let n = self.cooking_time.len().min(cooking_times.len());
                self.cooking_time[..n].copy_from_slice(&cooking_times[..n]);
            }
            None => self.cooking_time = [0; NUM_SLOTS],
        }
    }

    pub fn save_additional(&self, output: &mut Vec<(String, Nbt)>) {
        save_all_items(output, &self.items, true);
        output.push(("CookingTimes".into(), Nbt::IntArray(self.cooking_progress.to_vec())));
        output.push(("CookingTotalTimes".into(), Nbt::IntArray(self.cooking_time.to_vec())));
    }

    pub fn get_update_tag(&self) -> Nbt {
        let mut output = Vec::new();
        save_all_items(&mut output, &self.items, true);
        Nbt::Compound(output)
    }

    pub fn clear_content(&mut self) {
        self.items = vec![ItemStack::empty(); NUM_SLOTS];
    }

    pub fn apply_implicit_components<'a>(&mut self, get: &mut impl FnMut(DataComponentType) -> Option<&'a Nbt>) {
        copy_into(get(dc::CONTAINER), &mut self.items);
    }

    pub fn collect_implicit_components(&self, components: &mut Vec<(DataComponentType, Option<Nbt>)>) {
        components.retain(|(t, _)| *t != dc::CONTAINER);
        components.push((dc::CONTAINER, Some(from_items(&self.items))));
    }
}

impl ServerLevel {
    fn campfire(&mut self, pos: BlockPos) -> Option<&mut CampfireBlockEntity> {
        match &mut self.get_block_entity(pos)?.kind {
            BlockEntityKind::Campfire(campfire) => Some(campfire),
            _ => None,
        }
    }

    pub fn campfire_cook_tick(&mut self, pos: BlockPos, state: BlockState) {
        let mut changed = false;
        for slot in 0..NUM_SLOTS {
            let Some(entity) = self.campfire(pos) else { return };
            let item_stack = entity.items[slot].clone();
            if item_stack.is_empty() {
                continue;
            }
            changed = true;
            entity.cooking_progress[slot] += 1;
            if entity.cooking_progress[slot] >= entity.cooking_time[slot] {
                let result = match crafting::get_recipe_for(CookingRecipeType::CampfireCooking, &item_stack, None) {
                    Some(recipe) => crafting::cooking_recipes()[recipe].assemble(),
                    None => item_stack,
                };
                crate::world::containers::drop_item_stack(self, pos.x as f64, pos.y as f64, pos.z as f64, result);
                if let Some(entity) = self.campfire(pos) {
                    entity.items[slot] = ItemStack::empty();
                }
                self.send_block_updated(pos, state, state);
            }
        }
        if changed {
            self.block_entity_set_changed_static(pos, state);
        }
    }

    pub fn campfire_cooldown_tick(&mut self, pos: BlockPos, state: BlockState) {
        let Some(entity) = self.campfire(pos) else { return };
        let mut changed = false;
        for slot in 0..NUM_SLOTS {
            if entity.cooking_progress[slot] > 0 {
                changed = true;
                entity.cooking_progress[slot] = (entity.cooking_progress[slot] - BURN_COOL_SPEED).max(0).min(entity.cooking_time[slot]);
            }
        }
        if changed {
            self.block_entity_set_changed_static(pos, state);
        }
    }

    pub fn campfire_place_food(&mut self, pos: BlockPos, source_entity: Option<EntityId>, place_item: &mut ItemStack) -> bool {
        for slot in 0..NUM_SLOTS {
            let Some(entity) = self.campfire(pos) else { return false };
            if !entity.items[slot].is_empty() {
                continue;
            }
            let Some(recipe) = crafting::get_recipe_for(CookingRecipeType::CampfireCooking, place_item, None) else { return false };
            let infinite = source_entity.is_some_and(|source| self.is_player(source) && has_infinite_materials(self, source));
            let consumed = if infinite { place_item.copy_with_count(1) } else { place_item.split(1) };
            let Some(entity) = self.campfire(pos) else { return false };
            entity.cooking_time[slot] = crafting::cooking_recipes()[recipe].cooking_time;
            entity.cooking_progress[slot] = 0;
            entity.items[slot] = consumed;
            self.campfire_mark_updated(pos);
            return true;
        }
        false
    }

    fn campfire_mark_updated(&mut self, pos: BlockPos) {
        let Some(state) = self.chunks.block_entity_ref(pos).map(|entity| entity.block_state()) else { return };
        self.block_entity_set_changed(pos);
        self.send_block_updated(pos, state, state);
    }

    pub fn campfire_pre_remove_side_effects(&mut self, pos: BlockPos) {
        let Some(entity) = self.campfire(pos) else { return };
        let items = std::mem::replace(&mut entity.items, vec![ItemStack::empty(); NUM_SLOTS]);
        crate::world::containers::drop_contents_list(self, pos, items);
    }
}
