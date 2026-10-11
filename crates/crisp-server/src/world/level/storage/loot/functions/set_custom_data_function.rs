use crisp_protocol::nbt::Nbt;

use crate::advancements::predicates::nbt_predicate::NbtPredicate;
use crate::util::codec::{self, DataResult};
use crate::world::item::ItemStack;
use crate::world::item::component as dc;
use crate::world::level::storage::loot::loot_context::LootContext;

pub struct SetCustomDataFunction {
    tag: Vec<(String, Nbt)>,
}

impl SetCustomDataFunction {
    pub fn decode(tag: &Nbt) -> DataResult<SetCustomDataFunction> {
        let Nbt::Compound(fields) = codec::decode_field(tag, "tag", NbtPredicate::decode)?.tag else { unreachable!("compound tag") };
        Ok(SetCustomDataFunction { tag: fields })
    }

    pub fn run(&self, mut item_stack: ItemStack, _context: &mut LootContext) -> ItemStack {
        let mut custom_data = match item_stack.get(dc::CUSTOM_DATA) {
            Some(Nbt::Compound(fields)) => fields.clone(),
            _ => Vec::new(),
        };
        crate::nbt::merge(&mut custom_data, &self.tag);
        item_stack.set(dc::CUSTOM_DATA, if custom_data.is_empty() { None } else { Some(Nbt::Compound(custom_data)) });
        item_stack
    }
}
