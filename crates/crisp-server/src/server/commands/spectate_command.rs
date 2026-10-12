use crate::commands::brigadier::{CommandContext, CommandSyntaxException, DynamicCommandExceptionType, SimpleCommandExceptionType, tr};
use crate::commands::source::CommandSourceStack;
use crate::network::chat::Component;
use crate::server::Server;
use crate::world::entity::EntityId;

static ERROR_SELF: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.spectate.self");
static ERROR_NOT_SPECTATOR: DynamicCommandExceptionType = DynamicCommandExceptionType(|s| tr("commands.spectate.not_spectator", vec![s]));
static ERROR_CANNOT_SPECTATE: DynamicCommandExceptionType = DynamicCommandExceptionType(|s| tr("commands.spectate.cannot_spectate", vec![s]));

pub fn spectate_stop(server: &mut Server, _context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let player = source.player_or_exception(server)?;
    spectate(server, source, None, player)
}

pub fn spectate_target(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let target = context.get_entity(server, source, "target")?;
    let player = source.player_or_exception(server)?;
    spectate(server, source, Some(target), player)
}

pub fn spectate_target_player(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let target = context.get_entity(server, source, "target")?;
    let player = context.get_player(server, source, "player")?;
    spectate(server, source, Some(target), player)
}

fn spectate(server: &mut Server, source: &CommandSourceStack, target: Option<EntityId>, player: EntityId) -> Result<i32, CommandSyntaxException> {
    if target == Some(player) {
        return Err(ERROR_SELF.create());
    }
    if !server.entity_level(player).is_spectator(player) {
        return Err(ERROR_NOT_SPECTATOR.create(server.entity_display_name(player)));
    }
    if let Some(target) = target
        && crisp_data::registry::ENTITY_TYPE[server.entity_level(target).entity(target).entity_type as usize] == "minecraft:marker"
    {
        return Err(ERROR_CANNOT_SPECTATE.create(server.entity_display_name(target)));
    }
    server.set_camera(player, target);
    match target {
        Some(target) => {
            source.send_success(server, || Component::translatable_with("commands.spectate.success.started", vec![server.entity_display_name(target).into()]), false)
        }
        None => source.send_success(server, || Component::translatable("commands.spectate.success.stopped"), false),
    }
    Ok(1)
}
