use crisp_core::random::RandomSource;
use crisp_data::registry;

use super::command_response_tracker::CommandResponseTracker;
use crate::commands::arguments::item::ItemInput;
use crate::commands::brigadier::{CommandContext, CommandSyntaxException, Dynamic2CommandExceptionType, tr};
use crate::commands::source::CommandSourceStack;
use crate::network::chat::{Arg, Component};
use crate::server::Server;
use crate::server::level::SoundSource;
use crate::world::entity::{EntityId, LevelEntity};

static TOO_MANY_ITEMS: Dynamic2CommandExceptionType = Dynamic2CommandExceptionType(|max, item| tr("commands.give.failed.toomanyitems", vec![max, item]));

const MAX_ALLOWED_ITEMSTACKS: i32 = 100;

pub fn give(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let players = context.get_players(server, source, "targets")?;
    give_item(server, source, context.get_item("item"), players, 1)
}

pub fn give_count(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let players = context.get_players(server, source, "targets")?;
    give_item(server, source, context.get_item("item"), players, context.get_int("count"))
}

fn give_item(server: &mut Server, source: &CommandSourceStack, input: &ItemInput, players: Vec<EntityId>, count: i32) -> Result<i32, CommandSyntaxException> {
    let mut tracker = CommandResponseTracker::new();
    let prototype = input.create_item_stack(1)?;
    let max_stack_size = prototype.max_stack_size();
    let max_allowed_count = max_stack_size * MAX_ALLOWED_ITEMSTACKS;
    if count > max_allowed_count {
        return Err(TOO_MANY_ITEMS.create(max_allowed_count, prototype.display_name()));
    }
    for player in players {
        let mut remaining = count;
        while remaining > 0 {
            let size = max_stack_size.min(remaining);
            remaining -= size;
            let mut copy_to_drop = prototype.copy_with_count(size);
            let level = &mut server.level;
            let p = level.player_mut(player);
            let infinite_materials = p.abilities.instabuild;
            let (added, _) = p.inventory.add_stack(&mut copy_to_drop, infinite_materials);
            if added && copy_to_drop.is_empty() {
                let mut drop = level.create_item_stack_to_drop(player, prototype.clone(), false, false);
                drop.make_fake_item();
                level.add_fresh_entity(LevelEntity::Item(drop));
                let p = level.player_mut(player);
                let (x, y, z) = (p.entity.x(), p.entity.y(), p.entity.z());
                let pitch = ((p.entity.random.next_float() - p.entity.random.next_float()) * 0.7 + 1.0) * 2.0;
                let sound = crisp_data::registry_id(registry::SOUND_EVENT, "minecraft:entity.item.pickup");
                level.play_sound_at(None, x, y, z, sound, SoundSource::Players, 0.2, pitch);
                level.broadcast_container_menu_changes(player);
            } else if !copy_to_drop.is_empty() {
                let mut drop = level.create_item_stack_to_drop(player, copy_to_drop, false, false);
                drop.set_no_pickup_delay();
                drop.target = Some(level.player(player).profile.uuid);
                level.add_fresh_entity(LevelEntity::Item(drop));
            }
        }
        tracker.track(player);
    }
    let name = prototype.display_name();
    tracker.send_feedback(
        server,
        source,
        true,
        |player, _| Component::translatable_with("commands.give.success.single", vec![Arg::Int(count), name.clone().into(), server.entity_display_name(player).into()]),
        |players, _| Component::translatable_with("commands.give.success.multiple", vec![Arg::Int(count), name.clone().into(), Arg::Int(players)]),
    )
}
