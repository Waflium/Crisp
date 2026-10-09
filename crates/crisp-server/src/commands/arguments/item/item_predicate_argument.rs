use crisp_data::Item;
use crate::advancements::predicates::min_max_bounds::{Bounds, Ints};
use crisp_protocol::nbt::Nbt;

use super::component_predicate_parser::{self, Context};
use crate::commands::arguments::registry_tag;
use crate::commands::brigadier::{
    CommandSyntaxException, DISPATCHER_PARSE_EXCEPTION, Dynamic2CommandExceptionType, DynamicCommandExceptionType, StringReader, tr,
};
use crate::nbt;
use crate::world::item::ItemStack;
use crate::world::item::component::codec::{self as component_codec, Codec};
use crate::world::item::component::{self as dc, DataComponentType};

pub static ERROR_UNKNOWN_ITEM: DynamicCommandExceptionType = DynamicCommandExceptionType(|id| tr("argument.item.id.invalid", vec![id]));
pub static ERROR_UNKNOWN_TAG: DynamicCommandExceptionType = DynamicCommandExceptionType(|tag| tr("arguments.item.tag.unknown", vec![tag]));
pub static ERROR_UNKNOWN_COMPONENT: DynamicCommandExceptionType = DynamicCommandExceptionType(|id| tr("arguments.item.component.unknown", vec![id]));
pub static ERROR_MALFORMED_COMPONENT: Dynamic2CommandExceptionType =
    Dynamic2CommandExceptionType(|ty, message| tr("arguments.item.component.malformed", vec![ty, message]));
pub static ERROR_UNKNOWN_PREDICATE: DynamicCommandExceptionType = DynamicCommandExceptionType(|id| tr("arguments.item.predicate.unknown", vec![id]));
pub static ERROR_MALFORMED_PREDICATE: Dynamic2CommandExceptionType =
    Dynamic2CommandExceptionType(|ty, message| tr("arguments.item.predicate.malformed", vec![ty, message]));

const COUNT_ID: &str = "minecraft:count";

const ANY: Ints = Ints { bounds: Bounds { min: None, max: None } };

fn decode_ints(tag: &Nbt) -> Result<Ints, String> {
    let bounds = match tag {
        Nbt::Compound(_) => {
            let field = |name: &str| tag.get(name).map(int_value).transpose();
            match (field("min"), field("max")) {
                (Ok(min), Ok(max)) => Bounds { min, max },
                (Err(a), Err(b)) => return Err(format!("{a}; {b}")),
                (Err(e), _) | (_, Err(e)) => return Err(e),
            }
        }
        other => match int_value(other) {
            Ok(v) => Bounds { min: Some(v), max: Some(v) },
            Err(_) => return Err(format!("Failed to parse either. First: Not a map: {}; Second: Not a number", nbt::to_string(other))),
        },
    };
    if let (Some(min), Some(max)) = (bounds.min, bounds.max)
        && min > max
    {
        return Err(format!("Swapped bounds in range: Optional[{min}] is higher than Optional[{max}]"));
    }
    Ok(Ints { bounds })
}

#[derive(Clone, Debug, PartialEq)]
pub enum ItemPredicate {
    Item(Item),
    Tag(&'static [i32]),
    Any,
    Has(DataComponentType),
    Equals(DataComponentType, Nbt),
    Count(Ints),
    Damage { durability: Ints, damage: Ints },
    Not(Box<ItemPredicate>),
    AnyOf(Vec<ItemPredicate>),
    AllOf(Vec<ItemPredicate>),
}

impl ItemPredicate {
    pub fn test(&self, stack: &ItemStack) -> bool {
        match self {
            ItemPredicate::Item(item) => stack.item == *item,
            ItemPredicate::Tag(items) => items.contains(&(stack.item.0 as i32)),
            ItemPredicate::Any => true,
            ItemPredicate::Has(component) => stack.has(*component),
            ItemPredicate::Equals(component, expected) => stack.get(*component) == Some(expected),
            ItemPredicate::Count(range) => range.matches(stack.count),
            ItemPredicate::Damage { durability, damage } => match stack.get(dc::DAMAGE).map(component_codec::as_i32) {
                Some(value) => durability.matches(stack.get(dc::MAX_DAMAGE).map_or(0, component_codec::as_i32) - value) && damage.matches(value),
                _ => false,
            },
            ItemPredicate::Not(inner) => !inner.test(stack),
            ItemPredicate::AnyOf(alternatives) => alternatives.iter().any(|p| p.test(stack)),
            ItemPredicate::AllOf(all) => all.iter().all(|p| p.test(stack)),
        }
    }
}

fn int_value(tag: &Nbt) -> Result<i32, String> {
    Codec::int().decode(tag).map(|v| component_codec::as_i32(&v))
}

pub struct ComponentWrapper(Option<DataComponentType>);

pub enum PredicateWrapper {
    Count,
    Damage,
    Exists(DataComponentType),
    Unsupported(&'static str),
}

struct ItemContext;

fn not_supported(reader: &StringReader, what: String) -> CommandSyntaxException {
    DISPATCHER_PARSE_EXCEPTION.create_with_context(reader, format!("{what} is not supported yet"))
}

impl Context for ItemContext {
    type Predicate = ItemPredicate;
    type ComponentType = ComponentWrapper;
    type PredicateType = PredicateWrapper;

    fn for_element_type(&self, reader: &StringReader, id: &str) -> Result<ItemPredicate, CommandSyntaxException> {
        Item::by_name(id).map(ItemPredicate::Item).ok_or_else(|| ERROR_UNKNOWN_ITEM.create_with_context(reader, id))
    }

    fn for_tag_type(&self, reader: &StringReader, id: &str) -> Result<ItemPredicate, CommandSyntaxException> {
        registry_tag("minecraft:item", id).map(ItemPredicate::Tag).ok_or_else(|| ERROR_UNKNOWN_TAG.create_with_context(reader, id))
    }

    fn lookup_component_type(&self, reader: &StringReader, id: &str) -> Result<ComponentWrapper, CommandSyntaxException> {
        if id == COUNT_ID {
            return Ok(ComponentWrapper(None));
        }
        match DataComponentType::by_name(id) {
            Some(component) if !component.is_transient() => Ok(ComponentWrapper(Some(component))),
            _ => Err(ERROR_UNKNOWN_COMPONENT.create_with_context(reader, id)),
        }
    }

    fn create_component_test(&self, reader: &StringReader, component: &ComponentWrapper, value: &Nbt) -> Result<ItemPredicate, CommandSyntaxException> {
        let name = component.0.map_or(COUNT_ID, DataComponentType::name);
        let malformed = |message: String| ERROR_MALFORMED_COMPONENT.create_with_context(reader, name, message);
        match component.0 {
            None => decode_ints(value).map(ItemPredicate::Count).map_err(malformed),
            Some(ty) => ty.decode(value).map(|expected| ItemPredicate::Equals(ty, expected)).map_err(malformed),
        }
    }

    fn create_component_presence_test(&self, component: &ComponentWrapper) -> ItemPredicate {
        match component.0 {
            None => ItemPredicate::Any,
            Some(ty) => ItemPredicate::Has(ty),
        }
    }

    fn lookup_predicate_type(&self, reader: &StringReader, id: &str) -> Result<PredicateWrapper, CommandSyntaxException> {
        if id == COUNT_ID {
            return Ok(PredicateWrapper::Count);
        }
        if let Some(&predicate) = crisp_data::registry::DATA_COMPONENT_PREDICATE_TYPE.iter().find(|&&p| p == id) {
            return Ok(if predicate == "minecraft:damage" { PredicateWrapper::Damage } else { PredicateWrapper::Unsupported(predicate) });
        }
        match DataComponentType::by_name(id) {
            Some(component) => Ok(PredicateWrapper::Exists(component)),
            None => Err(ERROR_UNKNOWN_PREDICATE.create_with_context(reader, id)),
        }
    }

    fn create_predicate_test(&self, reader: &StringReader, predicate: &PredicateWrapper, value: &Nbt) -> Result<ItemPredicate, CommandSyntaxException> {
        let (id, result) = match predicate {
            PredicateWrapper::Count => (COUNT_ID, decode_ints(value).map(ItemPredicate::Count)),
            PredicateWrapper::Damage => ("minecraft:damage", decode_damage_predicate(value)),
            PredicateWrapper::Exists(component) => (component.name(), Codec::Unit.decode(value).map(|_| ItemPredicate::Has(*component))),
            PredicateWrapper::Unsupported(id) => return Err(not_supported(reader, format!("Item predicate '{id}'"))),
        };
        result.map_err(|message| ERROR_MALFORMED_PREDICATE.create_with_context(reader, id, message))
    }

    fn negate(&self, value: ItemPredicate) -> ItemPredicate {
        ItemPredicate::Not(Box::new(value))
    }

    fn any_of(&self, alternatives: Vec<ItemPredicate>) -> ItemPredicate {
        ItemPredicate::AnyOf(alternatives)
    }
}

fn decode_damage_predicate(value: &Nbt) -> Result<ItemPredicate, String> {
    Codec::Unit.decode(value)?;
    let field = |name: &str| value.get(name).map(decode_ints).transpose().map(|b| b.unwrap_or(ANY));
    match (field("durability"), field("damage")) {
        (Ok(durability), Ok(damage)) => Ok(ItemPredicate::Damage { durability, damage }),
        (Err(a), Err(b)) => Err(format!("{a}; {b}")),
        (Err(e), _) | (_, Err(e)) => Err(e),
    }
}

pub fn parse(reader: &mut StringReader) -> Result<ItemPredicate, CommandSyntaxException> {
    component_predicate_parser::parse(reader, &ItemContext).map(ItemPredicate::AllOf)
}
