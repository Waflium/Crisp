use crisp_data::packets::play::clientbound;

use super::command_response_tracker::CommandResponseTracker;
use crate::commands::arguments::item::item_predicate_argument::ItemPredicate;
use crate::commands::brigadier::{CommandContext, CommandSyntaxException, DynamicCommandExceptionType, tr};
use crate::commands::source::CommandSourceStack;
use crate::network::chat::{Arg, Component};
use crate::server::Server;
use crate::world::entity::EntityId;
use crate::world::inventory::{Kind, craft};
use crate::world::item::{ItemStack, OFFHAND, write_slot};

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

const CLEAR_ORDER: [usize; 45] = {
    let mut order = [0; 45];
    let mut i = 0;
    while i < 36 {
        order[i] = if i < 9 { 36 + i } else { i };
        i += 1;
    }
    let rest = [8, 7, 6, 5, OFFHAND, 1, 2, 3, 4];
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
        let player = server.level.player_mut(id);
        let mut count = 0;
        let mut changed = Vec::new();
        for slot in CLEAR_ORDER {
            let removed = clear_or_count(&mut player.inventory.slots[slot], predicate, max_count - count, counting_only);
            if removed > 0 && !counting_only {
                changed.push(slot);
            }
            count += removed;
        }
        let carried_before = player.carried.clone();
        count += clear_or_count(&mut player.carried, predicate, max_count - count, counting_only);
        tracker.track_value(id, count);
        if !counting_only {
            changed.sort_unstable();
            player.dirty_slots.extend(changed);
            player.sync_inventory();
            if player.carried != carried_before {
                player.connection.send(clientbound::SET_CURSOR_ITEM, |w| write_slot(w, player.carried.as_ref()));
            }
            let slots = &mut player.inventory.slots;
            slots[0] = craft(&slots[1..=4], 2);
            if player.window.kind == Kind::Inventory {
                player.send_slot(0);
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
