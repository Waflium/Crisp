use crisp_protocol::nbt::Nbt;

use crate::util::codec::{self, DataResult};
use crate::world::level::storage::loot::loot_context::LootContext;
use crate::world::level::storage::loot::loot_context_arg::{ArgCodecBuilder, LootContextArg};

const GETTER: ArgCodecBuilder = ArgCodecBuilder { any_entity: true, any_block_entity: true, any_item_stack: false, container: false };

pub enum NbtProvider {
    Context(LootContextArg),
    Storage(String),
}

impl NbtProvider {
    pub fn decode(tag: &Nbt) -> DataResult<NbtProvider> {
        match codec::either(tag, |t| LootContextArg::decode(t, GETTER), |t| {
            let ty = codec::decode_field(t, "type", codec::identifier)?;
            match ty.as_str() {
                "minecraft:storage" => Ok(NbtProvider::Storage(codec::decode_field(t, "source", codec::identifier)?)),
                "minecraft:context" => Ok(NbtProvider::Context(codec::decode_field(t, "target", |a| LootContextArg::decode(a, GETTER))?)),
                other => Err(format!("Unknown registry key in ResourceKey[minecraft:root / minecraft:loot_nbt_provider_type]: {other}")),
            }
        })? {
            Ok(arg) => Ok(NbtProvider::Context(arg)),
            Err(provider) => Ok(provider),
        }
    }

    pub fn get(&self, context: &mut LootContext) -> Option<Nbt> {
        match self {
            NbtProvider::Context(LootContextArg::Entity(target)) => target.get(&context.params).map(|entity| context.params.level.entity_tag_to_compare(entity)),
            NbtProvider::Context(LootContextArg::BlockEntity) => context.params.block_entity.map(|entity| entity.save_with_full_metadata()),
            NbtProvider::Context(_) => None,
            NbtProvider::Storage(id) => Some(Nbt::Compound(context.level().command_storage.borrow_mut().get(id))),
        }
    }
}
