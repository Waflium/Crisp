use crisp_protocol::nbt::Nbt;

use super::loot_context::EntityTarget;
use super::parameters::loot_context_params::LootContextParam;
use crate::util::codec::{self, DataResult};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LootContextArg {
    Entity(EntityTarget),
    BlockEntity,
    Tool,
    Container,
}

#[derive(Clone, Copy)]
pub struct ArgCodecBuilder {
    pub any_entity: bool,
    pub any_block_entity: bool,
    pub any_item_stack: bool,
    pub container: bool,
}

impl LootContextArg {
    pub fn decode(tag: &Nbt, builder: ArgCodecBuilder) -> DataResult<LootContextArg> {
        let name = codec::string(tag)?;
        if builder.any_entity
            && let Some((_, target)) = EntityTarget::VALUES.iter().find(|(n, _)| *n == name)
        {
            return Ok(LootContextArg::Entity(*target));
        }
        match name {
            "block_entity" if builder.any_block_entity => Ok(LootContextArg::BlockEntity),
            "tool" if builder.any_item_stack => Ok(LootContextArg::Tool),
            "container" if builder.container => Ok(LootContextArg::Container),
            _ => Err(format!("Unknown element name:{name}")),
        }
    }

    pub fn context_param(self) -> LootContextParam {
        match self {
            LootContextArg::Entity(target) => target.context_param(),
            LootContextArg::BlockEntity => LootContextParam::BlockEntity,
            LootContextArg::Tool => LootContextParam::Tool,
            LootContextArg::Container => LootContextParam::Container,
        }
    }
}
