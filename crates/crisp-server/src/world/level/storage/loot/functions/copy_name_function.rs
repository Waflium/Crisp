use crisp_protocol::nbt::Nbt;

use crate::util::codec::{self, DataResult};
use crate::world::entity::data::{DataValue, ids};
use crate::world::item::ItemStack;
use crate::world::item::component as dc;
use crate::world::level::storage::loot::loot_context::LootContext;
use crate::world::level::storage::loot::loot_context_arg::{ArgCodecBuilder, LootContextArg};

pub const ENTITY_OR_BLOCK: ArgCodecBuilder = ArgCodecBuilder { any_entity: true, any_block_entity: true, any_item_stack: false, container: false };

pub struct CopyNameFunction {
    source: LootContextArg,
}

impl CopyNameFunction {
    pub fn decode(tag: &Nbt) -> DataResult<CopyNameFunction> {
        Ok(CopyNameFunction { source: codec::decode_field(tag, "source", |t| LootContextArg::decode(t, ENTITY_OR_BLOCK))? })
    }

    pub fn run(&self, mut item_stack: ItemStack, context: &mut LootContext) -> ItemStack {
        match self.source {
            LootContextArg::Entity(target) => {
                if let Some(entity) = target.get(&context.params) {
                    let name = match context.params.level.entity(entity).data.get(ids::CUSTOM_NAME) {
                        DataValue::OptionalComponent(name) => name.clone(),
                        _ => None,
                    };
                    item_stack.set(dc::CUSTOM_NAME, name);
                }
            }
            LootContextArg::BlockEntity => {
                if let Some(base) = context.params.block_entity.and_then(|entity| entity.container_base()) {
                    item_stack.set(dc::CUSTOM_NAME, base.name.clone());
                }
            }
            LootContextArg::Tool | LootContextArg::Container => {}
        }
        item_stack
    }
}
