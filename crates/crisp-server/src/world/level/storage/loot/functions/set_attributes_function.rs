use crisp_core::random::RandomSource;
use crisp_protocol::nbt::Nbt;

use crate::core::holder::Holder;
use crate::util::codec::{self, DataResult};
use crate::world::entity::attributes::{Attribute, Operation};
use crate::world::item::ItemStack;
use crate::world::item::component as dc;
use crate::world::level::storage::loot::loot_context::LootContext;
use crate::world::level::storage::loot::providers::number::floats::ContextFloatProvider;

const EQUIPMENT_SLOT_GROUPS: [&str; 11] = ["any", "mainhand", "offhand", "hand", "feet", "legs", "chest", "head", "armor", "body", "saddle"];

struct Modifier {
    id: String,
    attribute: Attribute,
    operation: Operation,
    amount: Holder<ContextFloatProvider>,
    slots: Vec<&'static str>,
}

impl Modifier {
    fn decode(tag: &Nbt) -> DataResult<Modifier> {
        let slots = codec::decode_field(tag, "slot", |t| {
            codec::compact_list_of(t, |s| {
                let name = codec::string(s)?;
                EQUIPMENT_SLOT_GROUPS.into_iter().find(|&group| group == name).ok_or_else(|| format!("Unknown element name:{name}"))
            })
        })?;
        if slots.is_empty() {
            return Err("List must have contents".to_string());
        }
        Ok(Modifier {
            id: codec::decode_field(tag, "id", codec::identifier)?,
            attribute: codec::decode_field(tag, "attribute", |t| {
                let id = codec::identifier(t)?;
                Attribute::by_name(&id).ok_or_else(|| format!("Unknown registry key in ResourceKey[minecraft:root / minecraft:attribute]: {id}"))
            })?,
            operation: codec::decode_field(tag, "operation", |t| {
                let name = codec::string(t)?;
                Operation::by_name(name).ok_or_else(|| format!("Unknown element name:{name}"))
            })?,
            amount: codec::decode_field(tag, "amount", ContextFloatProvider::decode_holder)?,
            slots,
        })
    }
}

pub struct SetAttributesFunction {
    modifiers: Vec<Modifier>,
    replace: bool,
}

impl SetAttributesFunction {
    pub fn decode(tag: &Nbt) -> DataResult<SetAttributesFunction> {
        Ok(SetAttributesFunction {
            modifiers: codec::decode_field(tag, "modifiers", |t| codec::list_of(t, Modifier::decode))?,
            replace: codec::decode_or(tag, "replace", true, codec::bool_value)?,
        })
    }

    pub fn run(&self, mut item_stack: ItemStack, context: &mut LootContext) -> ItemStack {
        let item_modifiers = if self.replace { Vec::new() } else { item_stack.get(dc::ATTRIBUTE_MODIFIERS).map_or(Vec::new(), |m| m.as_list().to_vec()) };
        let value = Nbt::List(self.update_modifiers(context, item_modifiers));
        let value = dc::ATTRIBUTE_MODIFIERS.decode(&value).unwrap_or(value);
        item_stack.set(dc::ATTRIBUTE_MODIFIERS, Some(value));
        item_stack
    }

    fn update_modifiers(&self, context: &mut LootContext, mut item_modifiers: Vec<Nbt>) -> Vec<Nbt> {
        for modifier in &self.modifiers {
            let slot = modifier.slots[context.random().next_int_bounded(modifier.slots.len() as i32) as usize];
            let amount = modifier.amount.value().get_float(context);
            let attribute = modifier.attribute.name();
            item_modifiers.retain(|entry| {
                let same_attribute = entry.get("type").and_then(Nbt::as_str) == Some(attribute);
                let same_id = entry.get("id").and_then(Nbt::as_str) == Some(modifier.id.as_str());
                !(same_attribute && same_id)
            });
            item_modifiers.push(Nbt::Compound(vec![
                ("type".into(), Nbt::String(attribute.to_string())),
                ("id".into(), Nbt::String(modifier.id.clone())),
                ("amount".into(), Nbt::Double(amount as f64)),
                ("operation".into(), Nbt::String(modifier.operation.name().to_string())),
                ("slot".into(), Nbt::String(slot.to_string())),
            ]));
        }
        item_modifiers
    }
}
