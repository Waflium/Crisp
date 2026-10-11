use crisp_core::Direction;
use crisp_protocol::nbt::Nbt;

use super::{int_value, long_value};
use crate::world::item::ItemStack;

#[derive(Clone)]
pub struct BrushableBlockEntity {
    pub item: ItemStack,
    pub hit_direction: Option<Direction>,
    pub loot_table: Option<String>,
    pub loot_table_seed: i64,
}

impl Default for BrushableBlockEntity {
    fn default() -> BrushableBlockEntity {
        BrushableBlockEntity { item: ItemStack::empty(), hit_direction: None, loot_table: None, loot_table_seed: 0 }
    }
}

impl BrushableBlockEntity {
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

    pub fn get_update_tag(&self) -> Nbt {
        let mut tag = Vec::new();
        if let Some(hit_direction) = self.hit_direction {
            tag.push(("hit_direction".into(), Nbt::Byte(hit_direction.ordinal() as i8)));
        }
        if !self.item.is_empty() {
            tag.push(("item".into(), self.item.to_nbt()));
        }
        Nbt::Compound(tag)
    }

    pub fn load_additional(&mut self, input: &Nbt) {
        self.item = if !self.try_load_loot_table(input) {
            input.get("item").and_then(|item| ItemStack::from_nbt(item).ok()).unwrap_or_else(ItemStack::empty)
        } else {
            ItemStack::empty()
        };
        self.hit_direction = input.get("hit_direction").and_then(int_value).map(|value| Direction::from_3d_data_value(value as i8 as i32));
    }

    pub fn save_additional(&self, output: &mut Vec<(String, Nbt)>) {
        if !self.try_save_loot_table(output) && !self.item.is_empty() {
            output.push(("item".into(), self.item.to_nbt()));
        }
    }

    pub fn set_loot_table(&mut self, loot_table: &str, seed: i64) {
        self.loot_table = Some(loot_table.to_owned());
        self.loot_table_seed = seed;
    }
}
