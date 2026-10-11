use crisp_core::random::RandomSource;
use crisp_protocol::nbt::Nbt;

use crate::core::holder_set::HolderSet;
use crate::util::codec::{self, DataResult};
use crate::world::item::ItemStack;
use crate::world::item::component as dc;
use crate::world::item::component::codec::registry;
use crate::world::level::storage::loot::loot_context::LootContext;

pub struct SetInstrumentFunction {
    options: HolderSet,
}

impl SetInstrumentFunction {
    pub fn decode(tag: &Nbt) -> DataResult<SetInstrumentFunction> {
        Ok(SetInstrumentFunction { options: codec::decode_field(tag, "options", |t| HolderSet::decode("minecraft:instrument", t))? })
    }

    pub fn run(&self, mut item_stack: ItemStack, context: &mut LootContext) -> ItemStack {
        let contents = self.options.contents();
        if !contents.is_empty() {
            let instrument = contents[context.random().next_int_bounded(contents.len() as i32) as usize];
            item_stack.set(dc::INSTRUMENT, Some(Nbt::String(registry("minecraft:instrument")[instrument as usize].to_string())));
        }
        item_stack
    }
}
