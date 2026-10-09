use crisp_data::Item;

use super::ItemInput;
use crate::commands::arguments::read_identifier;
use crate::commands::brigadier::{CommandSyntaxException, Dynamic2CommandExceptionType, DynamicCommandExceptionType, SimpleCommandExceptionType, StringReader, tr};
use crate::nbt;
use crate::world::item::component::{DataComponentPatch, DataComponentType};

pub static ERROR_UNKNOWN_ITEM: DynamicCommandExceptionType = DynamicCommandExceptionType(|id| tr("argument.item.id.invalid", vec![id]));
pub static ERROR_UNKNOWN_COMPONENT: DynamicCommandExceptionType = DynamicCommandExceptionType(|id| tr("arguments.item.component.unknown", vec![id]));
pub static ERROR_MALFORMED_COMPONENT: Dynamic2CommandExceptionType =
    Dynamic2CommandExceptionType(|ty, message| tr("arguments.item.component.malformed", vec![ty, message]));
pub static ERROR_EXPECTED_COMPONENT: SimpleCommandExceptionType = SimpleCommandExceptionType("arguments.item.component.expected");
pub static ERROR_REPEATED_COMPONENT: DynamicCommandExceptionType = DynamicCommandExceptionType(|id| tr("arguments.item.component.repeated", vec![id]));

pub fn parse(reader: &mut StringReader) -> Result<ItemInput, CommandSyntaxException> {
    let cursor = reader.cursor;
    let result = parse_state(reader);
    if result.is_err() {
        reader.cursor = cursor;
    }
    result
}

fn parse_state(reader: &mut StringReader) -> Result<ItemInput, CommandSyntaxException> {
    let item = read_item(reader)?;
    let mut components = DataComponentPatch::default();
    if reader.at('[') {
        read_components(reader, &mut components)?;
    }
    Ok(ItemInput { item, components })
}

fn read_item(reader: &mut StringReader) -> Result<Item, CommandSyntaxException> {
    let cursor = reader.cursor;
    let id = read_identifier(reader)?;
    Item::by_name(&id).ok_or_else(|| {
        reader.cursor = cursor;
        ERROR_UNKNOWN_ITEM.create_with_context(reader, id)
    })
}

fn read_components(reader: &mut StringReader, components: &mut DataComponentPatch) -> Result<(), CommandSyntaxException> {
    reader.expect('[')?;
    let mut visited: Vec<DataComponentType> = Vec::new();
    while reader.can_read() && !reader.at(']') {
        reader.skip_whitespace();
        if reader.at('!') {
            reader.skip();
            let component = read_component_type(reader)?;
            if visited.contains(&component) {
                return Err(ERROR_REPEATED_COMPONENT.create(component.name()));
            }
            visited.push(component);
            components.put(component, None);
            reader.skip_whitespace();
        } else {
            let component = read_component_type(reader)?;
            if visited.contains(&component) {
                return Err(ERROR_REPEATED_COMPONENT.create(component.name()));
            }
            visited.push(component);
            reader.skip_whitespace();
            reader.expect('=')?;
            reader.skip_whitespace();
            let value = read_component(reader, component)?;
            components.put(component, Some(value));
            reader.skip_whitespace();
        }
        if !reader.at(',') {
            break;
        }
        reader.skip();
        reader.skip_whitespace();
        if !reader.can_read() {
            return Err(ERROR_EXPECTED_COMPONENT.create_with_context(reader));
        }
    }
    reader.expect(']')
}

pub fn read_component_type(reader: &mut StringReader) -> Result<DataComponentType, CommandSyntaxException> {
    if !reader.can_read() {
        return Err(ERROR_EXPECTED_COMPONENT.create_with_context(reader));
    }
    let cursor = reader.cursor;
    let id = read_identifier(reader)?;
    match DataComponentType::by_name(&id) {
        Some(component) if !component.is_transient() => Ok(component),
        _ => {
            reader.cursor = cursor;
            Err(ERROR_UNKNOWN_COMPONENT.create_with_context(reader, id))
        }
    }
}

fn read_component(reader: &mut StringReader, component: DataComponentType) -> Result<crisp_protocol::nbt::Nbt, CommandSyntaxException> {
    let cursor = reader.cursor;
    let tag = nbt::parse_as_argument(reader)?;
    component.decode(&tag).map_err(|message| {
        reader.cursor = cursor;
        ERROR_MALFORMED_COMPONENT.create_with_context(reader, component.name(), message)
    })
}
