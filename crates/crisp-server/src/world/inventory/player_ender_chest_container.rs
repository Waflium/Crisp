use crisp_core::BlockPos;
use crisp_protocol::nbt::Nbt;

use crate::world::item::ItemStack;

pub const CONTAINER_SIZE: usize = 27;

#[derive(Clone)]
pub struct PlayerEnderChestContainer {
    pub items: Vec<ItemStack>,
    active_chest: Option<(BlockPos, u64)>,
}

impl Default for PlayerEnderChestContainer {
    fn default() -> PlayerEnderChestContainer {
        PlayerEnderChestContainer { items: vec![ItemStack::empty(); CONTAINER_SIZE], active_chest: None }
    }
}

impl PlayerEnderChestContainer {
    pub fn set_active_chest(&mut self, active_chest: Option<(BlockPos, u64)>) {
        self.active_chest = active_chest;
    }

    pub fn active_chest(&self) -> Option<(BlockPos, u64)> {
        self.active_chest
    }

    pub fn is_active_chest(&self, pos: BlockPos, id: u64) -> bool {
        self.active_chest == Some((pos, id))
    }

    pub fn from_slots(&mut self, list: &[Nbt]) {
        self.items = vec![ItemStack::empty(); CONTAINER_SIZE];
        for entry in list {
            let Some(slot) = entry.get("Slot").and_then(Nbt::as_i64).map(|slot| slot as i8 as i32) else { continue };
            if let Ok(stack) = ItemStack::from_nbt(entry)
                && (0..CONTAINER_SIZE as i32).contains(&slot)
            {
                self.items[slot as usize] = stack;
            }
        }
    }

    pub fn store_as_slots(&self) -> Nbt {
        Nbt::List(
            self.items
                .iter()
                .enumerate()
                .filter(|(_, stack)| !stack.is_empty())
                .filter_map(|(slot, stack)| {
                    let Nbt::Compound(mut fields) = stack.to_nbt() else { return None };
                    fields.insert(0, ("Slot".into(), Nbt::Byte(slot as i8)));
                    Some(Nbt::Compound(fields))
                })
                .collect(),
        )
    }
}
