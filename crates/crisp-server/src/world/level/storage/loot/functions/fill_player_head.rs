use crisp_data::items;
use crisp_protocol::nbt::Nbt;

use crate::util::codec::{self, DataResult};
use crate::world::item::ItemStack;
use crate::world::item::component as dc;
use crate::world::level::storage::loot::loot_context::{EntityTarget, LootContext};

pub struct FillPlayerHead {
    entity_target: EntityTarget,
}

impl FillPlayerHead {
    pub fn decode(tag: &Nbt) -> DataResult<FillPlayerHead> {
        Ok(FillPlayerHead { entity_target: codec::decode_field(tag, "entity", EntityTarget::decode)? })
    }

    pub fn run(&self, mut item_stack: ItemStack, context: &mut LootContext) -> ItemStack {
        if !item_stack.is(items::PLAYER_HEAD) {
            return item_stack;
        }
        let Some(player) = self.entity_target.get(&context.params).and_then(|id| context.params.level.players.get(&id)) else { return item_stack };
        let profile = &player.profile;
        let uuid = profile.uuid;
        let id = Nbt::IntArray(vec![(uuid >> 96) as i32, (uuid >> 64) as i32, (uuid >> 32) as i32, uuid as i32]);
        let properties = profile
            .properties
            .iter()
            .map(|property| {
                let mut fields = vec![("name".to_string(), Nbt::String(property.name.clone())), ("value".to_string(), Nbt::String(property.value.clone()))];
                if let Some(signature) = &property.signature {
                    fields.push(("signature".into(), Nbt::String(signature.clone())));
                }
                Nbt::Compound(fields)
            })
            .collect();
        let value = Nbt::Compound(vec![("name".into(), Nbt::String(profile.name.clone())), ("id".into(), id), ("properties".into(), Nbt::List(properties))]);
        let value = dc::PROFILE.decode(&value).unwrap_or(value);
        item_stack.set(dc::PROFILE, Some(value));
        item_stack
    }
}
