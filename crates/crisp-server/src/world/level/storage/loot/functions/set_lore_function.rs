use crisp_protocol::nbt::Nbt;

use super::list_operation::ListOperation;
use super::set_name_function::decode_component;
use crate::util::codec::{self, DataResult};
use crate::world::item::ItemStack;
use crate::world::item::component as dc;
use crate::world::level::storage::loot::loot_context::{EntityTarget, LootContext};

pub struct SetLoreFunction {
    lore: Vec<Nbt>,
    mode: ListOperation,
    resolution_context: Option<EntityTarget>,
}

impl SetLoreFunction {
    pub fn decode(tag: &Nbt) -> DataResult<SetLoreFunction> {
        let lore = codec::decode_field(tag, "lore", |t| codec::list_of(t, decode_component))?;
        if lore.len() > 256 {
            return Err(format!("Too long: {}, expected range [0-256]", lore.len()));
        }
        Ok(SetLoreFunction { lore, mode: ListOperation::decode(tag, 256)?, resolution_context: codec::decode_optional(tag, "entity", EntityTarget::decode)? })
    }

    pub fn run(&self, mut item_stack: ItemStack, context: &mut LootContext) -> ItemStack {
        let old_lore = item_stack.get(dc::LORE).map_or(Vec::new(), |l| l.as_list().to_vec());
        let _ = self.resolution_context.and_then(|target| target.get(&context.params));
        let lines = self.mode.apply(&old_lore, &self.lore, 256);
        item_stack.set(dc::LORE, Some(Nbt::List(lines)));
        item_stack
    }
}
