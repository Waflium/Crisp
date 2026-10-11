use crisp_protocol::nbt::Nbt;

use crate::util::codec::{self, DataResult};
use crate::world::item::ItemStack;
use crate::world::item::component as dc;
use crate::world::item::component::codec::registry_id;
use crate::world::level::storage::loot::loot_context::LootContext;

pub struct SetPotionFunction {
    potion: String,
}

pub fn decode_potion(tag: &Nbt) -> DataResult<String> {
    let id = codec::identifier(tag)?;
    registry_id("minecraft:potion", &id)?;
    Ok(id)
}

pub fn with_potion(item_stack: &mut ItemStack, potion: &str) {
    let mut contents = match item_stack.get(dc::POTION_CONTENTS) {
        Some(Nbt::Compound(fields)) => fields.clone(),
        _ => Vec::new(),
    };
    contents.retain(|(key, _)| key != "potion");
    contents.insert(0, ("potion".into(), Nbt::String(potion.to_string())));
    let contents = Nbt::Compound(contents);
    let value = dc::POTION_CONTENTS.decode(&contents).unwrap_or(contents);
    item_stack.set(dc::POTION_CONTENTS, Some(value));
}

impl SetPotionFunction {
    pub fn decode(tag: &Nbt) -> DataResult<SetPotionFunction> {
        Ok(SetPotionFunction { potion: codec::decode_field(tag, "id", decode_potion)? })
    }

    pub fn run(&self, mut item_stack: ItemStack, _context: &mut LootContext) -> ItemStack {
        with_potion(&mut item_stack, &self.potion);
        item_stack
    }
}
