use crate::commands::brigadier::{CommandContext, CommandSyntaxException, Dynamic2CommandExceptionType, DynamicCommandExceptionType, SimpleCommandExceptionType, tr};
use crate::commands::source::CommandSourceStack;
use crate::network::chat::Component;
use crate::server::Server;
use crate::world::entity::EntityId;

static ERROR_NOT_RIDING: DynamicCommandExceptionType = DynamicCommandExceptionType(|entity| tr("commands.ride.not_riding", vec![entity]));
static ERROR_ALREADY_RIDING: Dynamic2CommandExceptionType =
    Dynamic2CommandExceptionType(|entity, vehicle| tr("commands.ride.already_riding", vec![entity, vehicle]));
static ERROR_MOUNT_FAILED: Dynamic2CommandExceptionType =
    Dynamic2CommandExceptionType(|entity, vehicle| tr("commands.ride.mount.failure.generic", vec![entity, vehicle]));
static ERROR_MOUNTING_PLAYER: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.ride.mount.failure.cant_ride_players");
static ERROR_MOUNTING_LOOP: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.ride.mount.failure.loop");
static ERROR_WRONG_DIMENSION: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.ride.mount.failure.wrong_dimension");

pub fn ride_mount(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let target = context.get_entity(server, source, "target")?;
    let vehicle = context.get_entity(server, source, "vehicle")?;
    mount(server, source, target, vehicle)
}

pub fn ride_dismount(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let target = context.get_entity(server, source, "target")?;
    dismount(server, source, target)
}

fn mount(server: &mut Server, source: &CommandSourceStack, target: EntityId, vehicle: EntityId) -> Result<i32, CommandSyntaxException> {
    let level = server.entity_level(target);
    if let Some(current_vehicle) = level.vehicle(target) {
        return Err(ERROR_ALREADY_RIDING.create(server.entity_display_name(target), server.entity_display_name(current_vehicle)));
    }
    if server.has_player(vehicle) {
        return Err(ERROR_MOUNTING_PLAYER.create());
    }
    if server.entity_level(target).self_and_passengers(target).contains(&vehicle) {
        return Err(ERROR_MOUNTING_LOOP.create());
    }
    if server.level_of(target) != server.level_of(vehicle) {
        return Err(ERROR_WRONG_DIMENSION.create());
    }
    if !server.entity_level_mut(target).start_riding(target, vehicle, true, true) {
        return Err(ERROR_MOUNT_FAILED.create(server.entity_display_name(target), server.entity_display_name(vehicle)));
    }
    source.send_success(
        server,
        || Component::translatable_with("commands.ride.mount.success", vec![server.entity_display_name(target).into(), server.entity_display_name(vehicle).into()]),
        true,
    );
    Ok(1)
}

fn dismount(server: &mut Server, source: &CommandSourceStack, target: EntityId) -> Result<i32, CommandSyntaxException> {
    let Some(vehicle) = server.entity_level(target).vehicle(target) else { return Err(ERROR_NOT_RIDING.create(server.entity_display_name(target))) };
    server.entity_level_mut(target).stop_riding(target);
    source.send_success(
        server,
        || Component::translatable_with("commands.ride.dismount.success", vec![server.entity_display_name(target).into(), server.entity_display_name(vehicle).into()]),
        true,
    );
    Ok(1)
}
