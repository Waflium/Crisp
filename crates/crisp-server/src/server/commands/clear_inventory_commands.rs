use super::command_response_tracker::CommandResponseTracker;
use crate::commands::arguments::item::item_predicate_argument::ItemPredicate;
use crate::commands::brigadier::{CommandContext, CommandSyntaxException, DynamicCommandExceptionType, tr};
use crate::commands::source::CommandSourceStack;
use crate::network::chat::{Arg, Component};
use crate::server::Server;
use crate::world::entity::EntityId;
use crate::world::item::{ItemStack, OFFHAND};

static ERROR_SINGLE: DynamicCommandExceptionType = DynamicCommandExceptionType(|name| tr("clear.failed.single", vec![name]));
static ERROR_MULTIPLE: DynamicCommandExceptionType = DynamicCommandExceptionType(|count| tr("clear.failed.multiple", vec![count]));

pub fn clear_self(server: &mut Server, _context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let player = source.player_or_exception(server)?;
    clear_inventory(server, source, vec![player], None, -1)
}

pub fn clear_targets(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let players = context.get_players(server, source, "targets")?;
    clear_inventory(server, source, players, None, -1)
}

pub fn clear_item(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let players = context.get_players(server, source, "targets")?;
    clear_inventory(server, source, players, Some(context.get_item_predicate("item")), -1)
}

pub fn clear_item_count(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let players = context.get_players(server, source, "targets")?;
    clear_inventory(server, source, players, Some(context.get_item_predicate("item")), context.get_int("maxCount"))
}

const CLEAR_ORDER: [usize; 41] = {
    let mut order = [0; 41];
    let mut i = 0;
    while i < 36 {
        order[i] = if i < 9 { 36 + i } else { i };
        i += 1;
    }
    let rest = [8, 7, 6, 5, OFFHAND];
    let mut j = 0;
    while j < rest.len() {
        order[36 + j] = rest[j];
        j += 1;
    }
    order
};

fn clear_or_count(stack: &mut Option<ItemStack>, predicate: Option<&ItemPredicate>, amount: i32, counting_only: bool) -> i32 {
    let Some(s) = stack.as_mut().filter(|s| !s.is_empty() && predicate.is_none_or(|p| p.test(s))) else { return 0 };
    if counting_only {
        return s.count;
    }
    let removed = if amount < 0 { s.count } else { amount.min(s.count) };
    s.count -= removed;
    if s.is_empty() {
        *stack = None;
    }
    removed
}

fn clear_inventory(server: &mut Server, source: &CommandSourceStack, players: Vec<EntityId>, predicate: Option<&ItemPredicate>, max_count: i32) -> Result<i32, CommandSyntaxException> {
    let mut tracker = CommandResponseTracker::new();
    let counting_only = max_count == 0;
    for id in players {
        let level = server.entity_level_mut(id);
        let Some(mut inventory_menu) = level.player_mut(id).inventory_menu.take() else { continue };
        let player = level.player_mut(id);
        let mut count = 0;
        for slot in CLEAR_ORDER {
            count += clear_or_count(&mut player.inventory.slots[slot], predicate, max_count - count, counting_only);
        }
        for slot in 0..inventory_menu.containers[0].get_container_size() {
            let mut item_stack = Some(inventory_menu.containers[0].get_item(slot).clone()).filter(|stack| !stack.is_empty());
            let amount_removed = clear_or_count(&mut item_stack, predicate, max_count - count, counting_only);
            if amount_removed > 0 && !counting_only {
                let item_stack = item_stack.unwrap_or_else(ItemStack::empty);
                if item_stack.is_empty() {
                    level.menu_container_set_item(&mut inventory_menu, id, 0, slot, ItemStack::empty());
                } else {
                    *inventory_menu.containers[0].get_item_mut(slot) = item_stack;
                }
            }
            count += amount_removed;
        }
        let player = level.player_mut(id);
        count += clear_or_count(&mut player.carried, predicate, max_count - count, counting_only);
        level.restore_container_menu(id, inventory_menu);
        tracker.track_value(id, count);
        if !counting_only {
            level.broadcast_container_menu_changes(id);
            if let Some(mut inventory_menu) = level.player_mut(id).inventory_menu.take() {
                level.slot_changed_crafting_grid(&mut inventory_menu, id, None);
                level.restore_container_menu(id, inventory_menu);
            }
        }
    }
    if tracker.total_value() == 0 {
        return Err(tracker.dispatch_any(|id, _| ERROR_SINGLE.create(server.entity_display_name(id)), |count, _| ERROR_MULTIPLE.create(count)));
    }
    let (single, multiple) = if counting_only { ("commands.clear.test.single", "commands.clear.test.multiple") } else { ("commands.clear.success.single", "commands.clear.success.multiple") };
    tracker.send_feedback(
        server,
        source,
        true,
        |id, total| Component::translatable_with(single, vec![Arg::Int(total), server.entity_display_name(id).into()]),
        |players, total| Component::translatable_with(multiple, vec![Arg::Int(total), Arg::Int(players)]),
    )
}
