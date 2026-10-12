pub mod abstract_container_menu;
pub mod lectern_menu;
pub mod menu_provider;
pub mod slot;
pub mod slot_ranges;

use std::sync::OnceLock;

use crisp_core::BlockPos;
use crisp_data::{EquipmentSlot, Item, RECIPES, tag};
use serde_json::Value;

use crate::world::item::component::{DataComponentPatch, json_to_nbt};
use crate::world::item::{Inventory, ItemStack, OFFHAND};

pub const PICKUP: i32 = 0;
pub const QUICK_MOVE: i32 = 1;
pub const SWAP: i32 = 2;
pub const THROW: i32 = 4;
pub const QUICK_CRAFT: i32 = 5;
pub const PICKUP_ALL: i32 = 6;
pub const OUTSIDE: i16 = -999;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Inventory,
    CraftingTable { pos: BlockPos },
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Loc {
    Inv(usize),
    Table(usize),
}

pub struct Window {
    pub id: i32,
    pub kind: Kind,
    table: [Option<ItemStack>; 10],
}

#[derive(Default)]
pub struct Drag {
    button: Option<u8>,
    slots: Vec<usize>,
}

pub const SLOT_COUNT: usize = 46;

impl Window {
    pub fn inventory() -> Window {
        Window { id: 0, kind: Kind::Inventory, table: Default::default() }
    }

    pub fn crafting_table(id: i32, pos: BlockPos) -> Window {
        Window { id, kind: Kind::CraftingTable { pos }, table: Default::default() }
    }

    pub fn slot(&self, inventory: &Inventory, slot: usize) -> Option<ItemStack> {
        match self.loc(slot)? {
            Loc::Inv(i) => inventory.slots[i].clone(),
            Loc::Table(i) => self.table[i].clone(),
        }
    }

    pub fn window_slot(&self, inventory_slot: usize) -> Option<usize> {
        match self.kind {
            Kind::Inventory => Some(inventory_slot),
            Kind::CraftingTable { .. } => (9..=44).contains(&inventory_slot).then_some(inventory_slot + 1),
        }
    }

    fn loc(&self, slot: usize) -> Option<Loc> {
        match self.kind {
            Kind::Inventory => (slot < SLOT_COUNT).then_some(Loc::Inv(slot)),
            Kind::CraftingTable { .. } => match slot {
                0..=9 => Some(Loc::Table(slot)),
                10..=36 => Some(Loc::Inv(slot - 1)),
                37..=45 => Some(Loc::Inv(slot - 1)),
                _ => None,
            },
        }
    }

    fn grid(&self) -> (std::ops::RangeInclusive<usize>, usize) {
        match self.kind {
            Kind::Inventory => (1..=4, 2),
            Kind::CraftingTable { .. } => (1..=9, 3),
        }
    }

    fn storage(&self) -> (usize, usize, usize) {
        match self.kind {
            Kind::Inventory => (9, 36, 45),
            Kind::CraftingTable { .. } => (10, 37, 46),
        }
    }
}

pub struct Menu<'a> {
    pub window: &'a mut Window,
    pub inventory: &'a mut Inventory,
    pub carried: &'a mut Option<ItemStack>,
}

impl Menu<'_> {
    pub fn get(&self, slot: usize) -> Option<ItemStack> {
        self.window.slot(self.inventory, slot)
    }

    fn set(&mut self, slot: usize, stack: Option<ItemStack>) {
        let stack = stack.filter(|s| s.count > 0);
        match self.window.loc(slot) {
            Some(Loc::Inv(i)) => self.inventory.slots[i] = stack,
            Some(Loc::Table(i)) => self.window.table[i] = stack,
            None => {}
        }
    }

    fn is_result(&self, slot: usize) -> bool {
        slot == 0
    }

    fn equipment_slot(&self, slot: usize) -> Option<EquipmentSlot> {
        match (self.window.kind, slot) {
            (Kind::Inventory, 5) => Some(EquipmentSlot::Head),
            (Kind::Inventory, 6) => Some(EquipmentSlot::Chest),
            (Kind::Inventory, 7) => Some(EquipmentSlot::Legs),
            (Kind::Inventory, 8) => Some(EquipmentSlot::Feet),
            _ => None,
        }
    }

    fn may_place(&self, slot: usize, stack: &ItemStack) -> bool {
        if self.is_result(slot) || self.window.loc(slot).is_none() {
            return false;
        }
        self.equipment_slot(slot).is_none_or(|s| stack.item.info().equipment_slot == Some(s))
    }

    fn max_for(&self, slot: usize, stack: &ItemStack) -> i32 {
        if self.equipment_slot(slot).is_some() { 1 } else { stack.max_stack_size() }
    }

    pub fn update_result(&mut self) {
        let (grid, width) = self.window.grid();
        let items: Vec<Option<ItemStack>> = grid.map(|s| self.get(s)).collect();
        let result = craft(&items, width);
        self.set(0, result);
    }

    fn consume_grid(&mut self) {
        for slot in self.window.grid().0 {
            if let Some(mut s) = self.get(slot) {
                s.count -= 1;
                self.set(slot, Some(s));
            }
        }
        self.update_result();
    }

    fn move_to(&mut self, mut stack: ItemStack, start: usize, end: usize, reverse: bool) -> ItemStack {
        let order: Vec<usize> = if reverse { (start..end).rev().collect() } else { (start..end).collect() };
        for &slot in &order {
            if stack.count == 0 {
                break;
            }
            if let Some(mut existing) = self.get(slot).filter(|e| ItemStack::is_same_item_same_components(e, &stack)) {
                let room = self.max_for(slot, &existing) - existing.count;
                let moved = stack.count.min(room.max(0));
                existing.count += moved;
                stack.count -= moved;
                self.set(slot, Some(existing));
            }
        }
        for &slot in &order {
            if stack.count == 0 {
                break;
            }
            if self.get(slot).is_none() && self.may_place(slot, &stack) {
                let moved = stack.count.min(self.max_for(slot, &stack));
                self.set(slot, Some(stack.copy_with_count(moved)));
                stack.count -= moved;
            }
        }
        stack
    }

    fn quick_move(&mut self, slot: usize) -> bool {
        let Some(stack) = self.get(slot) else { return false };
        let original_count = stack.count;
        let (storage, hotbar, end) = self.window.storage();
        let (grid, _) = self.window.grid();
        let left = match self.window.kind {
            Kind::Inventory => {
                let equip = stack.item.info().equipment_slot;
                let armor = (5..=8).find(|&s| equip.is_some() && self.equipment_slot(s) == equip);
                if slot == 0 {
                    self.move_to(stack, storage, end, true)
                } else if grid.contains(&slot) || (5..=8).contains(&slot) || slot == OFFHAND {
                    self.move_to(stack, storage, end, false)
                } else if let Some(armor) = armor.filter(|&a| self.get(a).is_none()) {
                    self.move_to(stack, armor, armor + 1, false)
                } else if equip == Some(EquipmentSlot::Offhand) && self.get(OFFHAND).is_none() {
                    self.move_to(stack, OFFHAND, OFFHAND + 1, false)
                } else if slot < hotbar {
                    self.move_to(stack, hotbar, end, false)
                } else {
                    self.move_to(stack, storage, hotbar, false)
                }
            }
            Kind::CraftingTable { .. } => {
                if slot == 0 {
                    self.move_to(stack, storage, end, true)
                } else if grid.contains(&slot) {
                    self.move_to(stack, storage, end, false)
                } else {
                    let rest = self.move_to(stack, *grid.start(), *grid.end() + 1, false);
                    if rest.count == 0 {
                        rest
                    } else if slot < hotbar {
                        self.move_to(rest, hotbar, end, false)
                    } else {
                        self.move_to(rest, storage, hotbar, false)
                    }
                }
            }
        };
        if left.count == original_count {
            return false;
        }
        if self.is_result(slot) {
            if left.count == 0 {
                self.consume_grid();
                return true;
            }
            return false;
        }
        self.set(slot, Some(left));
        if grid.contains(&slot) {
            self.update_result();
        }
        true
    }

    pub fn click(&mut self, slot: i16, button: u8, mode: i32, drag: &mut Drag) -> Vec<ItemStack> {
        let mut drops = Vec::new();
        if mode != QUICK_CRAFT {
            drag.button = None;
            drag.slots.clear();
        }
        let grid_before: Vec<Option<ItemStack>> = self.window.grid().0.map(|s| self.get(s)).collect();
        match mode {
            PICKUP if slot == OUTSIDE => {
                if let Some(mut c) = self.carried.take() {
                    if button == 1 {
                        drops.push(c.copy_with_count(1));
                        c.count -= 1;
                        *self.carried = Some(c).filter(|c| c.count > 0);
                    } else {
                        drops.push(c);
                    }
                }
            }
            PICKUP if slot >= 0 => self.pickup(slot as usize, button),
            QUICK_MOVE if slot >= 0 => {
                if slot == 0 {
                    let item = self.get(0).map(|s| s.item);
                    while self.get(0).map(|s| s.item) == item && item.is_some() && self.quick_move(0) {}
                } else {
                    self.quick_move(slot as usize);
                }
            }
            SWAP if slot >= 0 => self.swap(slot as usize, button),
            THROW if slot >= 0 && self.carried.is_none() => {
                let slot = slot as usize;
                if let Some(mut s) = self.get(slot) {
                    if self.is_result(slot) {
                        drops.push(s);
                        self.consume_grid();
                    } else {
                        let n = if button == 1 { s.count } else { 1 };
                        drops.push(s.copy_with_count(n));
                        s.count -= n;
                        self.set(slot, Some(s));
                    }
                }
            }
            QUICK_CRAFT => self.drag(slot, button, drag),
            PICKUP_ALL if slot >= 0 => self.pickup_all(slot as usize),
            _ => {}
        }
        let grid_after: Vec<Option<ItemStack>> = self.window.grid().0.map(|s| self.get(s)).collect();
        if grid_before != grid_after {
            self.update_result();
        }
        drops
    }

    fn pickup(&mut self, slot: usize, button: u8) {
        if self.window.loc(slot).is_none() {
            return;
        }
        if self.is_result(slot) {
            let Some(result) = self.get(0) else { return };
            match self.carried {
                None => *self.carried = Some(result),
                Some(c) if ItemStack::is_same_item_same_components(c, &result) && c.count + result.count <= c.max_stack_size() => c.count += result.count,
                Some(_) => return,
            }
            self.consume_grid();
            return;
        }
        let current = self.get(slot);
        match (current, self.carried.clone()) {
            (None, None) => {}
            (Some(s), None) => {
                let take = if button == 1 { (s.count + 1) / 2 } else { s.count };
                *self.carried = Some(s.copy_with_count(take));
                self.set(slot, Some(s.copy_with_count(s.count - take)));
            }
            (None, Some(c)) if self.may_place(slot, &c) => {
                let n = if button == 1 { 1 } else { c.count.min(self.max_for(slot, &c)) };
                self.set(slot, Some(c.copy_with_count(n)));
                *self.carried = Some(c.copy_with_count(c.count - n)).filter(|c| c.count > 0);
            }
            (Some(s), Some(c)) if ItemStack::is_same_item_same_components(&s, &c) && self.may_place(slot, &c) => {
                let room = self.max_for(slot, &s) - s.count;
                let n = if button == 1 { 1.min(room) } else { c.count.min(room) }.max(0);
                self.set(slot, Some(s.copy_with_count(s.count + n)));
                *self.carried = Some(c.copy_with_count(c.count - n)).filter(|c| c.count > 0);
            }
            (Some(s), Some(c)) if self.may_place(slot, &c) && c.count <= self.max_for(slot, &c) => {
                self.set(slot, Some(c));
                *self.carried = Some(s);
            }
            _ => {}
        }
    }

    fn swap(&mut self, slot: usize, button: u8) {
        let target = match button {
            0..=8 => 36 + button as usize,
            40 => OFFHAND,
            _ => return,
        };
        let hotbar = self.inventory.slots[target].clone();
        let current = self.get(slot);
        if self.is_result(slot) {
            if let (Some(result), None) = (current, hotbar) {
                self.inventory.slots[target] = Some(result);
                self.consume_grid();
            }
            return;
        }
        if let Some(h) = &hotbar {
            if !self.may_place(slot, h) || h.count > self.max_for(slot, h) {
                return;
            }
        }
        self.set(slot, hotbar);
        self.inventory.slots[target] = current;
    }

    fn drag(&mut self, slot: i16, button: u8, drag: &mut Drag) {
        let kind = button >> 2;
        match button & 3 {
            0 => {
                drag.button = Some(kind);
                drag.slots.clear();
            }
            1 if drag.button == Some(kind) && slot >= 0 => {
                let slot = slot as usize;
                let fits = self.carried.as_ref().is_some_and(|c| {
                    self.may_place(slot, c) && self.get(slot).is_none_or(|s| ItemStack::is_same_item_same_components(&s, c) && s.count < self.max_for(slot, &s))
                });
                if fits && !drag.slots.contains(&slot) {
                    drag.slots.push(slot);
                }
            }
            2 if drag.button == Some(kind) => {
                let slots = std::mem::take(&mut drag.slots);
                drag.button = None;
                let Some(mut c) = self.carried.clone() else { return };
                if slots.len() == 1 {
                    self.pickup(slots[0], if kind == 1 { 1 } else { 0 });
                    return;
                }
                let each = if kind == 1 { 1 } else { c.count / slots.len().max(1) as i32 };
                for slot in slots {
                    if c.count == 0 || each == 0 {
                        break;
                    }
                    let existing = self.get(slot).map_or(0, |s| s.count);
                    let n = each.min(self.max_for(slot, &c) - existing).min(c.count).max(0);
                    self.set(slot, Some(c.copy_with_count(existing + n)));
                    c.count -= n;
                }
                *self.carried = Some(c).filter(|c| c.count > 0);
            }
            _ => {}
        }
    }

    fn pickup_all(&mut self, clicked: usize) {
        let Some(mut c) = self.carried.clone() else { return };
        let max = c.max_stack_size();
        for full_stacks in [false, true] {
            for slot in 0..SLOT_COUNT {
                if c.count >= max {
                    break;
                }
                if slot == clicked || self.is_result(slot) {
                    continue;
                }
                let Some(mut s) = self.get(slot).filter(|s| ItemStack::is_same_item_same_components(s, &c)) else { continue };
                if (s.count >= s.max_stack_size()) != full_stacks {
                    continue;
                }
                let n = s.count.min(max - c.count);
                c.count += n;
                s.count -= n;
                self.set(slot, Some(s));
            }
        }
        *self.carried = Some(c);
    }

    pub fn close(&mut self) -> Vec<ItemStack> {
        let mut leftovers = Vec::new();
        let mut give_back = |menu: &mut Menu, stack: ItemStack| {
            let (_, left) = menu.inventory.add(stack.clone());
            if left > 0 {
                leftovers.push(stack.copy_with_count(left));
            }
        };
        if let Some(c) = self.carried.take() {
            give_back(self, c);
        }
        for slot in self.window.grid().0 {
            if let Some(s) = self.get(slot) {
                self.set(slot, None);
                give_back(self, s);
            }
        }
        self.set(0, None);
        leftovers
    }
}

type Ingredient = Vec<Item>;

enum Recipe {
    Shaped { width: usize, height: usize, cells: Vec<Option<Ingredient>>, result: ItemStack },
    Shapeless { ingredients: Vec<Ingredient>, result: ItemStack },
}

fn item_id(name: &str) -> Option<Item> {
    Item::by_name(name)
}

fn ingredient(v: &Value) -> Option<Ingredient> {
    match v {
        Value::String(s) => match s.strip_prefix('#') {
            Some(t) => Some(tag("minecraft:item", t).iter().map(|&i| Item(i as u16)).collect()),
            None => item_id(s).map(|i| vec![i]),
        },
        Value::Array(list) => list.iter().map(|v| v.as_str().and_then(item_id)).collect(),
        _ => None,
    }
}

fn result_stack(v: &Value) -> Option<ItemStack> {
    let patch = match v.get("components") {
        Some(components) => DataComponentPatch::from_nbt(&json_to_nbt(components)).ok()?,
        None => DataComponentPatch::default(),
    };
    ItemStack::from_template(item_id(v["id"].as_str()?)?, v["count"].as_i64().unwrap_or(1) as i32, &patch)
}

fn recipes() -> &'static [Recipe] {
    static RECIPE_LIST: OnceLock<Vec<Recipe>> = OnceLock::new();
    RECIPE_LIST.get_or_init(|| {
        RECIPES
            .iter()
            .filter_map(|(_, json)| {
                let r: Value = serde_json::from_str(json).ok()?;
                match r["type"].as_str()? {
                    "minecraft:crafting_shaped" => {
                        let pattern: Vec<&str> = r["pattern"].as_array()?.iter().map(|p| p.as_str()).collect::<Option<_>>()?;
                        let width = pattern.iter().map(|p| p.chars().count()).max()?;
                        let mut cells = Vec::new();
                        for row in &pattern {
                            for c in row.chars().chain(std::iter::repeat(' ')).take(width) {
                                cells.push(if c == ' ' { None } else { Some(ingredient(&r["key"][c.to_string()])?) });
                            }
                        }
                        Some(Recipe::Shaped { width, height: pattern.len(), cells, result: result_stack(&r["result"])? })
                    }
                    "minecraft:crafting_shapeless" => Some(Recipe::Shapeless {
                        ingredients: r["ingredients"].as_array()?.iter().map(ingredient).collect::<Option<_>>()?,
                        result: result_stack(&r["result"])?,
                    }),
                    _ => None,
                }
            })
            .collect()
    })
}

pub fn craft(grid: &[Option<ItemStack>], width: usize) -> Option<ItemStack> {
    let height = grid.len() / width;
    let filled = |x: usize, y: usize| grid[y * width + x].is_some();
    let xs: Vec<usize> = (0..width).filter(|&x| (0..height).any(|y| filled(x, y))).collect();
    let ys: Vec<usize> = (0..height).filter(|&y| (0..width).any(|x| filled(x, y))).collect();
    let (&x0, &x1, &y0, &y1) = (xs.first()?, xs.last()?, ys.first()?, ys.last()?);
    let (w, h) = (x1 - x0 + 1, y1 - y0 + 1);
    let at = |x: usize, y: usize| grid[(y0 + y) * width + x0 + x].as_ref();
    let items: Vec<ItemStack> = grid.iter().flatten().cloned().collect();

    recipes().iter().find_map(|recipe| match recipe {
        Recipe::Shaped { width: rw, height: rh, cells, result } => {
            if (*rw, *rh) != (w, h) {
                return None;
            }
            let fits = |mirror: bool| {
                (0..h).all(|y| {
                    (0..w).all(|x| {
                        let cell = &cells[y * w + if mirror { w - 1 - x } else { x }];
                        match (cell, at(x, y)) {
                            (None, None) => true,
                            (Some(i), Some(s)) => i.contains(&s.item),
                            _ => false,
                        }
                    })
                })
            };
            (fits(false) || fits(true)).then(|| result.clone())
        }
        Recipe::Shapeless { ingredients, result } => {
            if ingredients.len() != items.len() {
                return None;
            }
            fn assign(ingredients: &[Ingredient], items: &[ItemStack], used: &mut Vec<bool>) -> bool {
                let Some((first, rest)) = ingredients.split_first() else { return true };
                for i in 0..items.len() {
                    if !used[i] && first.contains(&items[i].item) {
                        used[i] = true;
                        if assign(rest, items, used) {
                            return true;
                        }
                        used[i] = false;
                    }
                }
                false
            }
            assign(ingredients, &items, &mut vec![false; items.len()]).then(|| result.clone())
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(name: &str) -> Item {
        Item::by_name(name).unwrap()
    }

    fn stack(name: &str, count: i32) -> Option<ItemStack> {
        Some(ItemStack::new(item(name), count))
    }

    #[test]
    fn crafts_planks_sticks_and_table() {
        assert_eq!(craft(&[None, None, None, stack("minecraft:oak_log", 1)], 2), stack("minecraft:oak_planks", 4));
        let sticks = [None, stack("minecraft:oak_planks", 1), None, stack("minecraft:birch_planks", 1)];
        assert_eq!(craft(&sticks, 2), stack("minecraft:stick", 4));
        let table: [Option<ItemStack>; 4] = std::array::from_fn(|_| stack("minecraft:oak_planks", 1));
        assert_eq!(craft(&table, 2), stack("minecraft:crafting_table", 1));
        assert_eq!(craft(&[stack("minecraft:dirt", 1), None, None, None], 2), None);
    }

    #[test]
    fn crafts_mirrored_pickaxe_on_a_table() {
        let p = || stack("minecraft:oak_planks", 1);
        let s = || stack("minecraft:stick", 1);
        let grid = [p(), p(), p(), None, s(), None, None, s(), None];
        assert_eq!(craft(&grid, 3), stack("minecraft:wooden_pickaxe", 1));
        let axe = [p(), p(), None, p(), s(), None, None, s(), None];
        let mirrored = [None, p(), p(), None, s(), p(), None, s(), None];
        assert_eq!(craft(&axe, 3), stack("minecraft:wooden_axe", 1));
        assert_eq!(craft(&mirrored, 3), stack("minecraft:wooden_axe", 1));
    }

    #[test]
    fn taking_the_result_consumes_the_grid() {
        let mut window = Window::inventory();
        let mut inventory = Inventory::new();
        let mut carried = None;
        inventory.slots[1] = stack("minecraft:oak_log", 2);
        let mut menu = Menu { window: &mut window, inventory: &mut inventory, carried: &mut carried };
        menu.update_result();
        assert_eq!(menu.get(0), stack("minecraft:oak_planks", 4));
        let mut drag = Drag::default();
        menu.click(0, 0, PICKUP, &mut drag);
        assert_eq!(menu.carried.clone(), stack("minecraft:oak_planks", 4));
        assert_eq!(menu.get(1), stack("minecraft:oak_log", 1));
        menu.click(0, 0, QUICK_MOVE, &mut drag);
        assert_eq!(menu.get(1), None);
        assert_eq!(menu.get(44), stack("minecraft:oak_planks", 4));
    }

    #[test]
    fn crafts_stone_pickaxe() {
        let c = || stack("minecraft:cobblestone", 1);
        let s = || stack("minecraft:stick", 1);
        let grid = [c(), c(), c(), None, s(), None, None, s(), None];
        assert_eq!(craft(&grid, 3), stack("minecraft:stone_pickaxe", 1));
    }

    #[test]
    fn shift_click_equips_helmet() {
        let mut window = Window::inventory();
        let mut inventory = Inventory::new();
        let mut carried = None;
        inventory.slots[36] = stack("minecraft:iron_helmet", 1);
        let mut menu = Menu { window: &mut window, inventory: &mut inventory, carried: &mut carried };
        menu.click(36, 0, QUICK_MOVE, &mut Drag::default());
        assert_eq!((menu.get(5), menu.get(36)), (stack("minecraft:iron_helmet", 1), None));
    }

    #[test]
    fn right_click_splits_and_places_one() {
        let mut window = Window::inventory();
        let mut inventory = Inventory::new();
        let mut carried = None;
        inventory.slots[36] = stack("minecraft:dirt", 5);
        let mut menu = Menu { window: &mut window, inventory: &mut inventory, carried: &mut carried };
        let mut drag = Drag::default();
        menu.click(36, 1, PICKUP, &mut drag);
        assert_eq!((menu.carried.clone(), menu.get(36)), (stack("minecraft:dirt", 3), stack("minecraft:dirt", 2)));
        menu.click(37, 1, PICKUP, &mut drag);
        assert_eq!((menu.carried.clone(), menu.get(37)), (stack("minecraft:dirt", 2), stack("minecraft:dirt", 1)));
        menu.click(5, 0, PICKUP, &mut drag);
        assert_eq!(menu.get(5), None);
    }
}
