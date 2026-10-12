use crisp_core::Vec3;
use crisp_data::EquipmentSlot;
use crisp_data::commands::CommandNode;
use crisp_data::damage_types;

use super::command_response_tracker::CommandResponseTracker;
use super::item::block_item_accessor;
use super::item::item_commands::{ERROR_TARGET_INAPPLICABLE_SLOT, ERROR_TARGET_NOT_A_CONTAINER};
use crate::commands::CommandFn;
use crate::commands::arguments::coordinates::get_loaded_block_pos;
use crate::commands::brigadier::{CommandContext, CommandSyntaxException, DynamicCommandExceptionType, tr};
use crate::commands::source::CommandSourceStack;
use crate::network::chat::{Arg, Component};
use crate::server::Server;
use crate::server::level::ServerLevel;
use crate::world::container::ContainerRef;
use crate::world::damagesource::DamageSource;
use crate::world::entity::LevelEntity;
use crate::world::entity::item::ItemEntity;
use crate::world::item::ItemStack;
use crate::world::level::block;
use crate::world::level::BlockGetter;
use crate::world::level::storage::loot::loot_params::LootParams;
use crate::world::level::storage::loot::loot_table::LootTable;

static ERROR_NO_HELD_ITEMS: DynamicCommandExceptionType = DynamicCommandExceptionType(|entity| tr("commands.drop.no_held_items", vec![entity]));
static ERROR_NO_ENTITY_LOOT_TABLE: DynamicCommandExceptionType = DynamicCommandExceptionType(|entity| tr("commands.drop.no_loot_table.entity", vec![entity]));
static ERROR_NO_BLOCK_LOOT_TABLE: DynamicCommandExceptionType = DynamicCommandExceptionType(|block| tr("commands.drop.no_loot_table.block", vec![block]));

const TARGETS: [&str; 7] = [
    "replace entity entities slot",
    "replace entity entities slot count",
    "replace block targetPos slot",
    "replace block targetPos slot count",
    "insert targetPos",
    "give players",
    "spawn targetPos",
];

const SOURCES: [&str; 10] = [
    "fish loot_table pos",
    "fish loot_table pos tool",
    "fish loot_table pos mainhand",
    "fish loot_table pos offhand",
    "loot loot_table",
    "kill target",
    "mine pos",
    "mine pos tool",
    "mine pos mainhand",
    "mine pos offhand",
];

pub fn executors() -> Vec<(String, CommandFn)> {
    let mut out: Vec<(String, CommandFn)> = Vec::new();
    for target in TARGETS {
        for source in SOURCES {
            out.push((format!("loot {target} {source}"), loot));
        }
    }
    out
}

fn name(context: &CommandContext, index: usize) -> &'static str {
    CommandNode::get(context.nodes[index].node).name
}

fn loot(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    if context.has_argument("loot_table") {
        let drops = if context.has_argument("pos") { drop_fishing_loot(server, context, source)? } else { drop_chest_loot(server, context, source) };
        drop(server, context, source, drops)
    } else if context.has_argument("target") {
        drop_kill_loot(server, context, source)
    } else {
        drop_block_loot(server, context, source)
    }
}

fn tool(server: &Server, context: &CommandContext, source: &CommandSourceStack) -> Result<ItemStack, CommandSyntaxException> {
    if context.has_argument("tool") {
        return context.get_item("tool").create_item_stack(1);
    }
    match name(context, context.nodes.len() - 1) {
        "mainhand" => get_source_hand_item(server, source, EquipmentSlot::Mainhand),
        "offhand" => get_source_hand_item(server, source, EquipmentSlot::Offhand),
        _ => Ok(ItemStack::empty()),
    }
}

fn get_source_hand_item(server: &Server, source: &CommandSourceStack, slot: EquipmentSlot) -> Result<ItemStack, CommandSyntaxException> {
    let entity = source.entity_or_exception()?;
    let level = server.entity_level(entity);
    if level.is_living(entity) {
        Ok(level.item_by_slot(entity, slot).unwrap_or_else(ItemStack::empty))
    } else {
        Err(ERROR_NO_HELD_ITEMS.create(server.entity_display_name(entity)))
    }
}

fn accept(server: &mut Server, context: &CommandContext, source: &CommandSourceStack, drops: &[ItemStack], used_items: &mut CommandResponseTracker<ItemStack>) -> Result<(), CommandSyntaxException> {
    let count = |drops: &[ItemStack]| if context.has_argument("count") { context.get_int("count") } else { drops.len() as i32 };
    match (name(context, 1), name(context, 2)) {
        ("replace", "entity") => {
            let entities = context.get_entities(server, source, "entities")?;
            entity_replace(server, entities, context.get_int("slot"), count(drops), drops, used_items);
            Ok(())
        }
        ("replace", _) => {
            let pos = get_loaded_block_pos(server, context, source, "targetPos")?;
            block_replace(&mut server.levels[source.level], pos, context.get_int("slot"), count(drops), drops, used_items)
        }
        ("insert", _) => {
            let pos = get_loaded_block_pos(server, context, source, "targetPos")?;
            block_distribute(&mut server.levels[source.level], pos, drops, used_items)
        }
        ("give", _) => {
            let players = context.get_players(server, source, "players")?;
            player_give(server, &players, drops, used_items);
            Ok(())
        }
        _ => {
            let pos = context.get_coordinates("targetPos").position(server, source);
            drop_in_world(&mut server.levels[source.level], pos, drops, used_items);
            Ok(())
        }
    }
}

fn block_distribute(level: &mut ServerLevel, pos: crisp_core::BlockPos, drops: &[ItemStack], used_items: &mut CommandResponseTracker<ItemStack>) -> Result<(), CommandSyntaxException> {
    let container = block_item_accessor::get_container(level, pos, &ERROR_TARGET_NOT_A_CONTAINER)?;
    for drop in drops {
        if distribute_to_container(level, container, drop.clone()) {
            level.container_set_changed(container);
            used_items.track(drop.clone());
        }
    }
    Ok(())
}

fn distribute_to_container(level: &mut ServerLevel, container: ContainerRef, mut item_stack: ItemStack) -> bool {
    let mut changed = false;
    let mut slot = 0;
    while slot < level.container_size(container) && !item_stack.is_empty() {
        let mut current = level.get_item(container, slot);
        if level.can_place_item(container, slot, &item_stack) {
            if current.is_empty() {
                level.set_item(container, slot, item_stack);
                changed = true;
                break;
            }
            if can_merge_items(&current, &item_stack) {
                let space = item_stack.max_stack_size() - current.count;
                let count = item_stack.count.min(space);
                item_stack.shrink(count);
                current.grow(count);
                level.put_item(container, slot, current);
                changed = true;
            }
        }
        slot += 1;
    }
    changed
}

fn is_list_backed(container: ContainerRef) -> bool {
    !matches!(container, ContainerRef::Jukebox(_) | ContainerRef::DecoratedPot(_) | ContainerRef::Composter(_) | ContainerRef::Lectern { .. })
}

fn block_replace(
    level: &mut ServerLevel,
    pos: crisp_core::BlockPos,
    start_slot: i32,
    slot_count: i32,
    drops: &[ItemStack],
    used_items: &mut CommandResponseTracker<ItemStack>,
) -> Result<(), CommandSyntaxException> {
    let container = block_item_accessor::get_container(level, pos, &ERROR_TARGET_NOT_A_CONTAINER)?;
    let max_slot = level.container_size(container) as i32;
    if start_slot < 0 || start_slot >= max_slot {
        return Err(ERROR_TARGET_INAPPLICABLE_SLOT.create(start_slot));
    }
    for i in 0..slot_count {
        let slot = start_slot + i;
        let to_add = drops.get(i as usize).cloned().unwrap_or_else(ItemStack::empty);
        if slot >= max_slot && is_list_backed(container) {
            return Err(crate::commands::unexpected_error(&format!("Index {slot} out of bounds for length {max_slot}")));
        }
        if level.can_place_item(container, slot as usize, &to_add) {
            let mut tracked = to_add.clone();
            tracked.limit_size(level.container_max_stack_size(container, &tracked));
            level.set_item(container, slot as usize, to_add);
            used_items.track(tracked);
        }
    }
    Ok(())
}

fn can_merge_items(a: &ItemStack, b: &ItemStack) -> bool {
    a.count <= a.max_stack_size() && ItemStack::is_same_item_same_components(a, b)
}

fn player_give(server: &mut Server, players: &[crate::world::entity::EntityId], drops: &[ItemStack], used_items: &mut CommandResponseTracker<ItemStack>) {
    for drop in drops {
        for &player in players {
            let level = server.entity_level_mut(player);
            let p = level.player_mut(player);
            let infinite_materials = p.abilities.instabuild;
            let (added, _) = p.inventory.add_stack(&mut drop.clone(), infinite_materials);
            if added {
                used_items.track(drop.clone());
            }
        }
    }
}

fn set_slots(level: &mut ServerLevel, entity: crate::world::entity::EntityId, items_to_set: &[ItemStack], start_slot: i32, count: i32, used_items: &mut CommandResponseTracker<ItemStack>) {
    for i in 0..count {
        let item = items_to_set.get(i as usize).cloned().unwrap_or_else(ItemStack::empty);
        if let Some(slot_access) = level.get_slot(entity, start_slot + i)
            && level.level_slot_set(&slot_access, item.clone())
        {
            used_items.track(item);
        }
    }
}

fn entity_replace(
    server: &mut Server,
    entities: Vec<crate::world::entity::EntityId>,
    start_slot: i32,
    count: i32,
    drops: &[ItemStack],
    used_items: &mut CommandResponseTracker<ItemStack>,
) {
    for entity in entities {
        let level = server.entity_level_mut(entity);
        set_slots(level, entity, drops, start_slot, count, used_items);
        if level.is_player(entity) {
            level.broadcast_container_menu_changes(entity);
        }
    }
}

fn drop_in_world(level: &mut ServerLevel, pos: Vec3, drops: &[ItemStack], used_items: &mut CommandResponseTracker<ItemStack>) {
    for drop in drops {
        let mut entity = ItemEntity::new(pos, drop.clone());
        entity.set_default_pickup_delay();
        level.add_fresh_entity(LevelEntity::Item(entity));
        used_items.track(drop.clone());
    }
}

fn drop_block_loot(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let pos = get_loaded_block_pos(server, context, source, "pos")?;
    let tool = tool(server, context, source)?;
    let level = &mut server.levels[source.level];
    let block_state = level.block_state(pos);
    let block_entity = level.get_block_entity(pos).cloned();
    let Some(loot_table) = block_state.block().info().loot_table else {
        return Err(ERROR_NO_BLOCK_LOOT_TABLE.create(Component::translatable(block_state.block().info().description_id)));
    };
    let mut params = LootParams::new(level);
    params.origin = Some(Vec3::at_center_of(pos));
    params.block_state = Some(block_state);
    params.block_entity = block_entity.as_ref();
    params.this_entity = source.entity;
    params.tool = Some(&tool);
    let drops = block::of(block_state).get_drops(block_state, params);
    drop_with_table(server, context, source, drops, loot_table)
}

fn drop_kill_loot(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let target = context.get_entity(server, source, "target")?;
    let level_index = if server.levels[source.level].entity_ref(target).is_some() { source.level } else { server.level_of(target).unwrap_or(source.level) };
    let Some(loot_table_id) = server.levels[level_index].get_loot_table(target) else {
        return Err(ERROR_NO_ENTITY_LOOT_TABLE.create(server.entity_display_name(target)));
    };
    let level = &mut server.levels[level_index];
    let killer = source.entity;
    let mut params = LootParams::new(level);
    if let Some(killer) = killer
        && params.level.is_player(killer)
    {
        params.last_damage_player = Some(killer);
    }
    params.damage_source = Some(DamageSource::new(damage_types::MAGIC));
    params.direct_attacking_entity = killer;
    params.attacking_entity = killer;
    params.this_entity = Some(target);
    params.origin = Some(source.position);
    let drops = LootTable::get(&loot_table_id).get_random_items(params);
    drop_with_table(server, context, source, drops, &loot_table_id)
}

fn drop_chest_loot(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Vec<ItemStack> {
    let loot_table = context.get_loot_table("loot_table");
    let mut params = LootParams::new(&mut server.levels[source.level]);
    params.this_entity = source.entity;
    params.origin = Some(source.position);
    loot_table.value().get_random_items(params)
}

fn drop_fishing_loot(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<Vec<ItemStack>, CommandSyntaxException> {
    let loot_table = context.get_loot_table("loot_table");
    let pos = get_loaded_block_pos(server, context, source, "pos")?;
    let tool = tool(server, context, source)?;
    let mut params = LootParams::new(&mut server.levels[source.level]);
    params.origin = Some(Vec3::at_center_of(pos));
    params.tool = Some(&tool);
    params.this_entity = source.entity;
    Ok(loot_table.value().get_random_items(params))
}

fn drop(server: &mut Server, context: &CommandContext, source: &CommandSourceStack, drops: Vec<ItemStack>) -> Result<i32, CommandSyntaxException> {
    let mut used_items = CommandResponseTracker::new();
    accept(server, context, source, &drops, &mut used_items)?;
    used_items.send_feedback(
        server,
        source,
        false,
        |drop, _| Component::translatable_with("commands.drop.success.single", vec![Arg::Int(drop.count), drop.display_name().into()]),
        |drop_count, _| Component::translatable_with("commands.drop.success.multiple", vec![Arg::Int(drop_count)]),
    )
}

fn drop_with_table(server: &mut Server, context: &CommandContext, source: &CommandSourceStack, drops: Vec<ItemStack>, key: &str) -> Result<i32, CommandSyntaxException> {
    let mut used_items = CommandResponseTracker::new();
    accept(server, context, source, &drops, &mut used_items)?;
    used_items.send_feedback(
        server,
        source,
        false,
        |drop, _| {
            Component::translatable_with("commands.drop.success.single_with_table", vec![Arg::Int(drop.count), drop.display_name().into(), Arg::String(key.to_string())])
        },
        |drop_count, _| Component::translatable_with("commands.drop.success.multiple_with_table", vec![Arg::Int(drop_count), Arg::String(key.to_string())]),
    )
}
