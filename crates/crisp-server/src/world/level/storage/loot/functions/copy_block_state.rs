use crisp_data::Block;
use crisp_protocol::nbt::Nbt;

use crate::util::codec::{self, DataResult};
use crate::world::item::ItemStack;
use crate::world::item::component as dc;
use crate::world::level::storage::loot::loot_context::LootContext;

pub struct CopyBlockState {
    properties: Vec<&'static str>,
}

impl CopyBlockState {
    pub fn decode(tag: &Nbt) -> DataResult<CopyBlockState> {
        let block = codec::decode_field(tag, "block", |t| {
            let id = codec::identifier(t)?;
            Block::by_name(&id).ok_or_else(|| format!("Unknown registry key in ResourceKey[minecraft:root / minecraft:block]: {id}"))
        })?;
        let names = codec::decode_field(tag, "properties", |t| codec::list_of(t, |p| Ok(codec::string(p)?.to_string())))?;
        let mut properties = Vec::new();
        for name in names {
            if let Some((name, _)) = block.property_by_name(&name)
                && !properties.contains(&name)
            {
                properties.push(name);
            }
        }
        Ok(CopyBlockState { properties })
    }

    pub fn run(&self, mut item_stack: ItemStack, context: &mut LootContext) -> ItemStack {
        let Some(state) = context.params.block_state else { return item_stack };
        let mut item_state = match item_stack.get(dc::BLOCK_STATE) {
            Some(Nbt::Compound(fields)) => fields.clone(),
            _ => Vec::new(),
        };
        for &property in &self.properties {
            if let Some(value) = state.property_value_by_name(property) {
                let value = Nbt::String(value.to_string());
                match item_state.iter_mut().find(|(k, _)| k == property) {
                    Some((_, existing)) => *existing = value,
                    None => item_state.push((property.to_string(), value)),
                }
            }
        }
        item_stack.set(dc::BLOCK_STATE, Some(Nbt::Compound(item_state)));
        item_stack
    }
}
