use crisp_core::{BlockPos, Vec3};
use crisp_data::BlockState;
use crisp_protocol::nbt::Nbt;

use super::{BlockEntity, int_value, long_value};
use crate::server::level::ServerLevel;
use crate::world::item::ItemStack;
use crate::world::level::storage::loot::loot_context::{LootContext, LootContextBuilder};
use crate::world::level::storage::loot::loot_params::{LootParams, SlotProvider};
use crate::world::item::component::values::ItemStackTemplate;
use crate::world::item::component::{self as dc, DataComponentType};

#[derive(Clone, Default)]
pub struct BaseContainerBlockEntity {
    pub lock_key: Option<Nbt>,
    pub name: Option<Nbt>,
    pub items: Vec<ItemStack>,
    pub loot_table: Option<String>,
    pub loot_table_seed: i64,
}

impl BaseContainerBlockEntity {
    pub fn with_size(size: usize) -> BaseContainerBlockEntity {
        BaseContainerBlockEntity { items: vec![ItemStack::empty(); size], ..Default::default() }
    }

    pub fn load_additional(&mut self, input: &Nbt) {
        self.lock_key = input.get("lock").and_then(|lock| dc::LOCK.decode(lock).ok());
        self.name = input.get("CustomName").and_then(|name| dc::CUSTOM_NAME.decode(name).ok());
    }

    pub fn save_additional(&self, output: &mut Vec<(String, Nbt)>) {
        if let Some(lock) = &self.lock_key {
            output.push(("lock".into(), lock.clone()));
        }
        if let Some(name) = &self.name {
            output.push(("CustomName".into(), name.clone()));
        }
    }

    pub fn is_locked(&self) -> bool {
        self.lock_key.as_ref().is_some_and(|lock| !matches!(lock, Nbt::Compound(fields) if fields.is_empty()))
    }

    pub fn try_load_loot_table(&mut self, input: &Nbt) -> bool {
        self.loot_table = input.get("LootTable").and_then(Nbt::as_str).and_then(crate::commands::arguments::parse_identifier);
        self.loot_table_seed = input.get("LootTableSeed").and_then(long_value).unwrap_or(0);
        self.loot_table.is_some()
    }

    pub fn try_save_loot_table(&self, output: &mut Vec<(String, Nbt)>) -> bool {
        let Some(loot_table) = &self.loot_table else { return false };
        output.push(("LootTable".into(), Nbt::String(loot_table.clone())));
        if self.loot_table_seed != 0 {
            output.push(("LootTableSeed".into(), Nbt::Long(self.loot_table_seed)));
        }
        true
    }

    pub fn load_all_items(&mut self, input: &Nbt, size: usize) {
        self.items = vec![ItemStack::empty(); size];
        load_all_items(input, &mut self.items);
    }

    pub fn apply_implicit_components<'a>(&mut self, get: &mut impl FnMut(DataComponentType) -> Option<&'a Nbt>) {
        self.name = get(dc::CUSTOM_NAME).cloned();
        self.lock_key = get(dc::LOCK).cloned();
        let contents = get(dc::CONTAINER).cloned();
        copy_into(contents.as_ref(), &mut self.items);
        if let Some(loot) = get(dc::CONTAINER_LOOT) {
            self.loot_table = loot.get("loot_table").and_then(Nbt::as_str).map(str::to_string);
            self.loot_table_seed = loot.get("seed").and_then(long_value).unwrap_or(0);
        }
    }

    pub fn collect_implicit_components(&self, components: &mut Vec<(DataComponentType, Option<Nbt>)>, randomizable: bool) {
        let mut set = |ty: DataComponentType, value: Option<Nbt>| {
            components.retain(|(t, _)| *t != ty);
            components.push((ty, value));
        };
        set(dc::CUSTOM_NAME, self.name.clone());
        if self.is_locked() {
            set(dc::LOCK, self.lock_key.clone());
        }
        set(dc::CONTAINER, Some(from_items(&self.items)));
        if randomizable && let Some(loot_table) = &self.loot_table {
            let mut loot = vec![("loot_table".to_string(), Nbt::String(loot_table.clone()))];
            if self.loot_table_seed != 0 {
                loot.push(("seed".into(), Nbt::Long(self.loot_table_seed)));
            }
            set(dc::CONTAINER_LOOT, Some(Nbt::Compound(loot)));
        }
    }
}

impl ServerLevel {
    pub fn get_loot_context<'a>(&'a mut self, pos: BlockPos, state: BlockState, block_entity: Option<&'a BlockEntity>) -> LootContext<'a> {
        let mut params = LootParams::new(self);
        params.block_state = Some(state);
        params.block_entity = block_entity;
        params.origin = Some(Vec3::at_center_of(pos));
        params.container = block_entity.map(SlotProvider::BlockEntity);
        LootContextBuilder::new(params).create(None)
    }
}

pub fn save_all_items(output: &mut Vec<(String, Nbt)>, items: &[ItemStack], also_when_empty: bool) {
    let list: Vec<Nbt> = items
        .iter()
        .enumerate()
        .filter(|(_, stack)| !stack.is_empty())
        .map(|(slot, stack)| {
            let Nbt::Compound(fields) = stack.to_nbt() else { unreachable!("stacks save as compounds") };
            let mut entry = vec![("Slot".to_string(), Nbt::Byte(slot as u8 as i8))];
            entry.extend(fields);
            Nbt::Compound(entry)
        })
        .collect();
    if !list.is_empty() || also_when_empty {
        output.push(("Items".into(), Nbt::List(list)));
    }
}

pub fn load_all_items(input: &Nbt, items: &mut [ItemStack]) {
    for entry in input.get("Items").map(Nbt::as_list).unwrap_or(&[]) {
        let slot = match entry.get("Slot") {
            Some(slot) => match int_value(slot) {
                Some(slot) => slot as u8 as usize,
                None => continue,
            },
            None => 0,
        };
        if let Ok(stack) = ItemStack::from_nbt(entry)
            && slot < items.len()
        {
            items[slot] = stack;
        }
    }
}

pub fn from_items(items: &[ItemStack]) -> Nbt {
    Nbt::List(
        items
            .iter()
            .enumerate()
            .filter(|(_, stack)| !stack.is_empty())
            .map(|(slot, stack)| {
                let template = ItemStackTemplate { item: stack.item.name().to_string(), count: stack.count, components: stack.components_patch().to_nbt() };
                Nbt::Compound(vec![("slot".into(), Nbt::Int(slot as i32)), ("item".into(), template.to_nbt())])
            })
            .collect(),
    )
}

pub fn copy_into(contents: Option<&Nbt>, destination: &mut [ItemStack]) {
    let slots = contents.map(crate::world::item::component::values::container_slots).unwrap_or_default();
    for (i, slot) in destination.iter_mut().enumerate() {
        *slot = slots.iter().rev().find(|(index, _)| *index as usize == i).and_then(|(_, template)| template.create()).unwrap_or_else(ItemStack::empty);
    }
}
