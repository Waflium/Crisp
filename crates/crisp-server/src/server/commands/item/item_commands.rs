use std::cell::Cell;
use std::rc::Rc;

use crisp_data::commands::CommandNode;

use super::super::command_response_tracker::CommandResponseTracker;
use super::block_item_accessor::BlockItemAccessor;
use super::entity_item_accessor::EntityItemAccessor;
use super::item_accessor::ItemAccessor;
use crate::commands::CommandFn;
use crate::commands::arguments::coordinates::get_loaded_block_pos;
use crate::commands::arguments::slot_source_argument::SlotSourceResult;
use crate::commands::brigadier::{CommandContext, CommandSyntaxException, Dynamic3CommandExceptionType, DynamicCommandExceptionType, SimpleCommandExceptionType, tr};
use crate::commands::source::CommandSourceStack;
use crate::server::Server;
use crate::server::level::ServerLevel;
use crate::world::item::ItemStack;
use crate::world::item::item_provider::{self, ItemProvider};
use crate::world::item::slot::counting_modifier::CountingModifier;
use crate::world::item::slot::slot_collection::SlotCollection;
use crate::world::item::slot::slot_selector;
use crate::world::item::slot::slot_source::SlotSource;
use crate::world::level::storage::loot::functions::loot_item_function::LootItemFunction;
use crate::world::level::storage::loot::loot_context::LootContextBuilder;
use crate::world::level::storage::loot::loot_params::{LootParams, SlotProvider};

pub static ERROR_TARGET_NOT_A_CONTAINER: Dynamic3CommandExceptionType = Dynamic3CommandExceptionType(|x, y, z| tr("commands.item.target.not_a_container", vec![x, y, z]));
pub static ERROR_SOURCE_NOT_A_CONTAINER: Dynamic3CommandExceptionType = Dynamic3CommandExceptionType(|x, y, z| tr("commands.item.source.not_a_container", vec![x, y, z]));
pub static ERROR_TARGET_INAPPLICABLE_SLOT: DynamicCommandExceptionType = DynamicCommandExceptionType(|slot| tr("commands.item.target.no_such_slot", vec![slot]));
static ERROR_TARGET_INAPPLICABLE_UNNAMED_SLOT: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.item.target.no_such_slot.unnamed");
static ERROR_SOURCE_INAPPLICABLE_SLOT: DynamicCommandExceptionType = DynamicCommandExceptionType(|slot| tr("commands.item.source.no_such_slot", vec![slot]));
static ERROR_SOURCE_INAPPLICABLE_UNNAMED_SLOT: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.item.source.no_such_slot.unnamed");
pub static ERROR_TARGET_NO_CHANGES: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.item.target.failed");
pub static ERROR_TARGET_NO_CHANGES_KNOWN_ITEM: DynamicCommandExceptionType = DynamicCommandExceptionType(|item| tr("commands.item.target.failed.known_item", vec![item]));

const ALL_PROVIDERS: [&str; 2] = ["entity", "block"];

pub fn executors() -> Vec<(String, CommandFn)> {
    let mut out: Vec<(String, CommandFn)> = Vec::new();
    for target in ALL_PROVIDERS {
        for distributor in ["replace", "fill", "override"] {
            let slots_node = format!("item {distributor} {target} target slots");
            for source in ALL_PROVIDERS {
                out.push((format!("{slots_node} from {source} source sourceSlots"), set_items_from));
                out.push((format!("{slots_node} from {source} source sourceSlots modifier"), set_items_from));
            }
            out.push((format!("{slots_node} with item"), set_item));
            out.push((format!("{slots_node} with item count"), set_item));
        }
        out.push((format!("item modify {target} target slots modifier"), modify));
    }
    out
}

fn name(context: &CommandContext, index: usize) -> &'static str {
    CommandNode::get(context.nodes[index].node).name
}

pub enum ArgAccessor {
    Entity(EntityItemAccessor),
    Block(BlockItemAccessor),
}

pub fn access(server: &Server, context: &CommandContext, source: &CommandSourceStack, kind: &str, arg: &str) -> Result<ArgAccessor, CommandSyntaxException> {
    Ok(match kind {
        "entity" => ArgAccessor::Entity(EntityItemAccessor { entities: context.get_entities(server, source, arg)? }),
        _ => ArgAccessor::Block(BlockItemAccessor { pos: get_loaded_block_pos(server, context, source, arg)? }),
    })
}

impl ArgAccessor {
    pub fn get_slots(&self, server: &mut Server, source: &CommandSourceStack, slot_source: &SlotSource) -> Result<Vec<ItemStack>, CommandSyntaxException> {
        match self {
            ArgAccessor::Entity(accessor) => accessor.get_slots(server, source, slot_source),
            ArgAccessor::Block(accessor) => accessor.get_slots(server, source, slot_source),
        }
    }
}

fn distribute(distributor: &str, items: Vec<ItemStack>) -> Box<dyn ItemProvider> {
    match distributor {
        "replace" => item_provider::of(items),
        "fill" => item_provider::cycle(items),
        _ => item_provider::or_else_provide(item_provider::of(items), ItemStack::empty()),
    }
}

fn target_inapplicable_slot(slot_source: &SlotSourceResult) -> CommandSyntaxException {
    match slot_source.name() {
        Some(name) => ERROR_TARGET_INAPPLICABLE_SLOT.create(name),
        None => ERROR_TARGET_INAPPLICABLE_UNNAMED_SLOT.create(),
    }
}

fn set_items<A: ItemAccessor>(
    server: &mut Server,
    source: &CommandSourceStack,
    accessor: &A,
    slot_source: &SlotSourceResult,
    items: &mut dyn ItemProvider,
    known_item: Option<&ItemStack>,
) -> Result<i32, CommandSyntaxException> {
    let selected_count = Rc::new(Cell::new(0));
    let slot_selector = slot_selector::tracking(slot_selector::ANY_SLOT, selected_count.clone());
    let mut tracker = CommandResponseTracker::new();
    accessor.set_items(server, source, slot_source.value(), &mut |level, target, slots| {
        items.restart();
        let count = slots.replace_slot_items(level, items, &slot_selector);
        tracker.track_value(target, count);
        count
    })?;
    if selected_count.get() == 0 {
        return Err(target_inapplicable_slot(slot_source));
    }
    accessor.get_replace_success(server, source, tracker, known_item)
}

fn set_items_with(
    server: &mut Server,
    source: &CommandSourceStack,
    accessor: ArgAccessor,
    slot_source: &SlotSourceResult,
    items: &mut dyn ItemProvider,
    known_item: Option<&ItemStack>,
) -> Result<i32, CommandSyntaxException> {
    match accessor {
        ArgAccessor::Entity(accessor) => set_items(server, source, &accessor, slot_source, items, known_item),
        ArgAccessor::Block(accessor) => set_items(server, source, &accessor, slot_source, items, known_item),
    }
}

fn set_items_from(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let target = access(server, context, source, name(context, 2), "target")?;
    let slot_source = context.get_slot_source("slots");
    let source_accessor = access(server, context, source, name(context, 6), "source")?;
    let modifier = context.has_argument("modifier").then(|| context.get_loot_modifier("modifier"));
    let items = get_items(server, source, &source_accessor, context.get_slot_source("sourceSlots"), modifier.map(|modifier| modifier.value()))?;
    let mut items = distribute(name(context, 1), items);
    set_items_with(server, source, target, slot_source, &mut *items, None)
}

fn set_item(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let target = access(server, context, source, name(context, 2), "target")?;
    let slot_source = context.get_slot_source("slots");
    let count = if context.has_argument("count") { context.get_int("count") } else { 1 };
    let item = context.get_item("item").create_item_stack(count)?;
    let mut items = distribute(name(context, 1), vec![item.clone()]);
    set_items_with(server, source, target, slot_source, &mut *items, Some(&item))
}

fn modify_items<A: ItemAccessor>(server: &mut Server, source: &CommandSourceStack, accessor: &A, slot_source: &SlotSourceResult, modifier: &LootItemFunction) -> Result<i32, CommandSyntaxException> {
    let selected_count = Rc::new(Cell::new(0));
    let slot_selector = slot_selector::tracking(slot_selector::non_empty_slots(), selected_count.clone());
    let mut apply = |level: &mut ServerLevel, item: ItemStack| apply_modifier(level, source, modifier, item);
    let mut counting_modifier = CountingModifier::new(&mut apply);
    let mut tracker = CommandResponseTracker::new();
    accessor.set_items(server, source, slot_source.value(), &mut |level, target, slots| {
        let last_updated = counting_modifier.updated_count();
        slots.modify_slots(level, &mut |level, slot| counting_modifier.accept(level, slot), &slot_selector);
        let count = counting_modifier.updated_count() - last_updated;
        tracker.track_value(target, count);
        count
    })?;
    if selected_count.get() == 0 {
        return Err(target_inapplicable_slot(slot_source));
    }
    accessor.get_modify_success(server, source, tracker)
}

fn modify(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let target = access(server, context, source, name(context, 2), "target")?;
    let slot_source = context.get_slot_source("slots");
    let modifier = context.get_loot_modifier("modifier").value();
    match target {
        ArgAccessor::Entity(accessor) => modify_items(server, source, &accessor, slot_source, modifier),
        ArgAccessor::Block(accessor) => modify_items(server, source, &accessor, slot_source, modifier),
    }
}

fn get_items(
    server: &mut Server,
    source: &CommandSourceStack,
    accessor: &ArgAccessor,
    slot_source: &SlotSourceResult,
    modifier: Option<&LootItemFunction>,
) -> Result<Vec<ItemStack>, CommandSyntaxException> {
    let slots = accessor.get_slots(server, source, slot_source.value())?;
    let items: Vec<ItemStack> = match modifier {
        Some(modifier) => {
            let level = &mut server.levels[source.level];
            slots.into_iter().map(|item| apply_modifier(level, source, modifier, item)).collect()
        }
        None => slots,
    };
    if items.is_empty() {
        return Err(match slot_source.name() {
            Some(name) => ERROR_SOURCE_INAPPLICABLE_SLOT.create(name),
            None => ERROR_SOURCE_INAPPLICABLE_UNNAMED_SLOT.create(),
        });
    }
    Ok(items)
}

pub fn apply_modifier(level: &mut ServerLevel, source: &CommandSourceStack, modifier: &LootItemFunction, item: ItemStack) -> ItemStack {
    let mut params = LootParams::new(level);
    params.origin = Some(source.position);
    params.this_entity = source.entity;
    let mut context = LootContextBuilder::new(params).create(None);
    let mut new_item = modifier.apply(item, &mut context);
    new_item.limit_size(new_item.max_stack_size());
    new_item
}

pub fn get_slots_from_provider<'s>(level: &mut ServerLevel, source: &CommandSourceStack, container: SlotProvider, slot_source: &'s SlotSource) -> SlotCollection<'s> {
    let mut params = LootParams::new(level);
    params.origin = Some(source.position);
    params.container = Some(container);
    params.this_entity = source.entity;
    let mut context = LootContextBuilder::new(params).create(None);
    slot_source.provide(&mut context)
}

pub fn item_stack_to_string(item_stack: &ItemStack) -> String {
    format!("{} {}", item_stack.count, item_stack.get_item().name())
}
