use crisp_protocol::nbt::Nbt;

use crate::advancements::predicates::collection_predicate::CollectionPredicate;
use crate::advancements::predicates::min_max_bounds::Doubles;
use crate::core::holder_set::HolderSet;
use crate::util::codec::{self, DataResult};
use crate::world::item::component::codec::{self as component_codec, registry_id};

const OPERATIONS: &[(&str, &str)] = &[("add_value", "add_value"), ("add_multiplied_base", "add_multiplied_base"), ("add_multiplied_total", "add_multiplied_total")];
const SLOT_GROUPS: &[(&str, &str)] = &[
    ("any", "any"),
    ("mainhand", "mainhand"),
    ("offhand", "offhand"),
    ("hand", "hand"),
    ("feet", "feet"),
    ("legs", "legs"),
    ("chest", "chest"),
    ("head", "head"),
    ("armor", "armor"),
    ("body", "body"),
    ("saddle", "saddle"),
];

#[derive(Debug)]
pub struct EntryPredicate {
    attribute: Option<HolderSet>,
    id: Option<String>,
    amount: Doubles,
    operation: Option<&'static str>,
    slot: Option<&'static str>,
}

impl EntryPredicate {
    fn decode(tag: &Nbt) -> DataResult<EntryPredicate> {
        codec::map(tag)?;
        Ok(EntryPredicate {
            attribute: codec::decode_optional(tag, "attribute", |t| HolderSet::decode("minecraft:attribute", t))?,
            id: codec::decode_optional(tag, "id", codec::identifier)?,
            amount: codec::decode_or(tag, "amount", Doubles::ANY, Doubles::decode)?,
            operation: codec::decode_optional(tag, "operation", |t| codec::string_enum(t, OPERATIONS))?,
            slot: codec::decode_optional(tag, "slot", |t| codec::string_enum(t, SLOT_GROUPS))?,
        })
    }

    fn test(&self, value: &Nbt) -> bool {
        if let Some(attribute) = &self.attribute {
            let id = value.get("type").and_then(Nbt::as_str).and_then(|t| registry_id("minecraft:attribute", t).ok());
            if !id.is_some_and(|id| attribute.contains(id)) {
                return false;
            }
        }
        if self.id.as_ref().is_some_and(|id| value.get("id").and_then(Nbt::as_str) != Some(id.as_str())) {
            return false;
        }
        if !self.amount.matches(value.get("amount").map_or(0.0, component_codec::as_f64)) {
            return false;
        }
        if self.operation.is_some_and(|operation| value.get("operation").and_then(Nbt::as_str) != Some(operation)) {
            return false;
        }
        self.slot.is_none_or(|slot| value.get("slot").and_then(Nbt::as_str).unwrap_or("any") == slot)
    }
}

#[derive(Debug)]
pub struct AttributeModifiersPredicate {
    modifiers: Option<CollectionPredicate<EntryPredicate>>,
}

impl AttributeModifiersPredicate {
    pub fn decode(tag: &Nbt) -> DataResult<AttributeModifiersPredicate> {
        codec::map(tag)?;
        Ok(AttributeModifiersPredicate { modifiers: codec::decode_optional(tag, "modifiers", |t| CollectionPredicate::decode(t, EntryPredicate::decode))? })
    }

    pub fn matches(&self, value: &Nbt) -> bool {
        self.modifiers.as_ref().is_none_or(|modifiers| modifiers.test(value.as_list(), |p, entry| p.test(entry)))
    }
}
