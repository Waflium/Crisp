use crisp_core::BlockPos;
use crisp_protocol::nbt::Nbt;

use super::{BlockEntityKind, long_value};
use crate::server::level::ServerLevel;
use crate::world::item::ItemStack;
use crate::world::item::component::values::ItemStackTemplate;
use crate::world::item::component::{self as dc, DataComponentType};

pub const TAG_SHERDS: &str = "sherds";
pub const TAG_ITEM: &str = "item";
pub const EVENT_POT_WOBBLES: i32 = 1;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WobbleStyle {
    Positive,
    Negative,
}

impl WobbleStyle {
    pub const VALUES: [WobbleStyle; 2] = [WobbleStyle::Positive, WobbleStyle::Negative];

    pub fn duration(self) -> i32 {
        match self {
            WobbleStyle::Positive => 7,
            WobbleStyle::Negative => 10,
        }
    }
}

fn pot_decorations_side(decorations: &Nbt, side: &str) -> Option<ItemStack> {
    match decorations.get(side)? {
        Nbt::String(id) => ItemStackTemplate { item: id.clone(), count: 1, components: Nbt::Compound(Vec::new()) }.create(),
        template => ItemStackTemplate::from_nbt(template).create(),
    }
}

fn is_empty_decorations(decorations: &Nbt) -> bool {
    matches!(decorations, Nbt::Compound(fields) if fields.is_empty())
}

#[derive(Clone)]
pub struct DecoratedPotBlockEntity {
    pub wobble_started_at_tick: i64,
    pub last_wobble_style: Option<WobbleStyle>,
    decorations: Nbt,
    item: ItemStack,
    pub loot_table: Option<String>,
    pub loot_table_seed: i64,
}

impl Default for DecoratedPotBlockEntity {
    fn default() -> DecoratedPotBlockEntity {
        DecoratedPotBlockEntity {
            wobble_started_at_tick: 0,
            last_wobble_style: None,
            decorations: Nbt::Compound(Vec::new()),
            item: ItemStack::empty(),
            loot_table: None,
            loot_table_seed: 0,
        }
    }
}

impl DecoratedPotBlockEntity {
    fn try_load_loot_table(&mut self, input: &Nbt) -> bool {
        self.loot_table = input.get("LootTable").and_then(Nbt::as_str).and_then(crate::commands::arguments::parse_identifier);
        self.loot_table_seed = input.get("LootTableSeed").and_then(long_value).unwrap_or(0);
        self.loot_table.is_some()
    }

    fn try_save_loot_table(&self, output: &mut Vec<(String, Nbt)>) -> bool {
        let Some(loot_table) = &self.loot_table else { return false };
        output.push(("LootTable".into(), Nbt::String(loot_table.clone())));
        if self.loot_table_seed != 0 {
            output.push(("LootTableSeed".into(), Nbt::Long(self.loot_table_seed)));
        }
        true
    }

    pub fn save_additional(&self, output: &mut Vec<(String, Nbt)>) {
        if !is_empty_decorations(&self.decorations) {
            output.push((TAG_SHERDS.into(), self.decorations.clone()));
        }
        if !self.try_save_loot_table(output) && !self.item.is_empty() {
            output.push((TAG_ITEM.into(), self.item.to_nbt()));
        }
    }

    pub fn load_additional(&mut self, input: &Nbt) {
        self.decorations = input.get(TAG_SHERDS).and_then(|sherds| dc::POT_DECORATIONS.decode(sherds).ok()).unwrap_or(Nbt::Compound(Vec::new()));
        if !self.try_load_loot_table(input) {
            self.item = input.get(TAG_ITEM).and_then(|item| ItemStack::from_nbt(item).ok()).unwrap_or_else(ItemStack::empty);
        } else {
            self.item = ItemStack::empty();
        }
    }

    pub fn get_decorations(&self) -> &Nbt {
        &self.decorations
    }

    pub fn sherds(&self) -> Vec<ItemStack> {
        ["left", "back", "front", "right"].iter().filter_map(|side| pot_decorations_side(&self.decorations, side)).collect()
    }

    pub fn create_decorated_pot_instance(decorations: &Nbt) -> ItemStack {
        let mut item = ItemStack::new(crisp_data::items::DECORATED_POT, 1);
        item.set(dc::POT_DECORATIONS, Some(decorations.clone()));
        item
    }

    pub fn collect_implicit_components(&self, components: &mut Vec<(DataComponentType, Option<Nbt>)>) {
        let mut set = |ty: DataComponentType, value: Option<Nbt>| {
            components.retain(|(t, _)| *t != ty);
            components.push((ty, value));
        };
        set(dc::POT_DECORATIONS, Some(self.decorations.clone()));
        set(dc::CONTAINER, Some(super::base_container_block_entity::from_items(std::slice::from_ref(&self.item))));
    }

    pub fn apply_implicit_components<'a>(&mut self, get: &mut impl FnMut(DataComponentType) -> Option<&'a Nbt>) {
        self.decorations = get(dc::POT_DECORATIONS).cloned().unwrap_or(Nbt::Compound(Vec::new()));
        let mut items = [ItemStack::empty()];
        super::base_container_block_entity::copy_into(get(dc::CONTAINER), &mut items);
        let [item] = items;
        self.item = item;
    }

    pub fn trigger_event(&mut self, event: i32, data: i32, game_time: i64) -> bool {
        if event == EVENT_POT_WOBBLES && data >= 0 && (data as usize) < WobbleStyle::VALUES.len() {
            self.wobble_started_at_tick = game_time;
            self.last_wobble_style = Some(WobbleStyle::VALUES[data as usize]);
            return true;
        }
        false
    }
}

impl ServerLevel {
    pub fn decorated_pot(&mut self, pos: BlockPos) -> Option<&mut DecoratedPotBlockEntity> {
        match &mut self.get_block_entity(pos)?.kind {
            BlockEntityKind::DecoratedPot(pot) => Some(pot),
            _ => None,
        }
    }

    pub fn decorated_pot_get_the_item(&mut self, pos: BlockPos) -> ItemStack {
        self.unpack_loot_table(pos, None);
        self.decorated_pot(pos).map_or_else(ItemStack::empty, |pot| pot.item.clone())
    }

    pub fn decorated_pot_split_the_item(&mut self, pos: BlockPos, count: i32) -> ItemStack {
        self.unpack_loot_table(pos, None);
        let Some(pot) = self.decorated_pot(pos) else { return ItemStack::empty() };
        let result = pot.item.split(count);
        if pot.item.is_empty() {
            pot.item = ItemStack::empty();
        }
        result
    }

    pub fn decorated_pot_set_the_item(&mut self, pos: BlockPos, item_stack: ItemStack) {
        self.unpack_loot_table(pos, None);
        if let Some(pot) = self.decorated_pot(pos) {
            pot.item = item_stack;
        }
    }

    pub fn decorated_pot_grow_the_item(&mut self, pos: BlockPos, amount: i32) {
        if let Some(pot) = self.decorated_pot(pos) {
            pot.item.grow(amount);
        }
    }

    pub fn decorated_pot_wobble(&mut self, pos: BlockPos, wobble_style: WobbleStyle) {
        let Some(state) = self.chunks.block_entity_ref(pos).map(|entity| entity.block_state()) else { return };
        self.block_event(pos, state.block(), EVENT_POT_WOBBLES, wobble_style as i32);
    }
}
