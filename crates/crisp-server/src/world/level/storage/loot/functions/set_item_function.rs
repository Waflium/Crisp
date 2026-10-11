use crisp_data::Item;
use crisp_protocol::nbt::Nbt;

use crate::util::codec::{self, DataResult};
use crate::world::item::ItemStack;
use crate::world::level::storage::loot::loot_context::LootContext;

pub struct SetItemFunction {
    item: Item,
}

pub fn decode_item(tag: &Nbt) -> DataResult<Item> {
    let id = codec::identifier(tag)?;
    Item::by_name(&id).ok_or_else(|| format!("Unknown registry key in ResourceKey[minecraft:root / minecraft:item]: {id}"))
}

impl SetItemFunction {
    pub fn decode(tag: &Nbt) -> DataResult<SetItemFunction> {
        Ok(SetItemFunction { item: codec::decode_field(tag, "item", decode_item)? })
    }

    pub fn run(&self, item_stack: ItemStack, _context: &mut LootContext) -> ItemStack {
        item_stack.transmute_copy(self.item, item_stack.count)
    }
}
