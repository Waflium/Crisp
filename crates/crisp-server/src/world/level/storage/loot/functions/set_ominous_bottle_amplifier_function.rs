use crisp_core::mth;
use crisp_protocol::nbt::Nbt;

use crate::core::holder::Holder;
use crate::util::codec::{self, DataResult};
use crate::world::item::ItemStack;
use crate::world::item::component as dc;
use crate::world::level::storage::loot::loot_context::LootContext;
use crate::world::level::storage::loot::providers::number::ints::ContextIntProvider;

pub struct SetOminousBottleAmplifierFunction {
    amplifier: Holder<ContextIntProvider>,
}

impl SetOminousBottleAmplifierFunction {
    pub fn decode(tag: &Nbt) -> DataResult<SetOminousBottleAmplifierFunction> {
        Ok(SetOminousBottleAmplifierFunction { amplifier: codec::decode_field(tag, "amplifier", ContextIntProvider::decode_holder)? })
    }

    pub fn run(&self, mut item_stack: ItemStack, context: &mut LootContext) -> ItemStack {
        let amplifier_value = mth::clamp_i(self.amplifier.value().get_int(context), 0, 4);
        item_stack.set(dc::OMINOUS_BOTTLE_AMPLIFIER, Some(Nbt::Int(amplifier_value)));
        item_stack
    }
}
