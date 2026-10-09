use super::look_at::LookAt;
use crate::commands::brigadier::{CommandContext, CommandSyntaxException};
use crate::commands::source::{Anchor, CommandSourceStack};
use crate::network::chat::Component;
use crate::server::Server;
use crate::world::entity::EntityId;

fn success(server: &Server, source: &CommandSourceStack, entity: EntityId) -> i32 {
    source.send_success(server, || Component::translatable_with("commands.rotate.success", vec![server.entity_display_name(entity).into()]), true);
    1
}

pub fn rotate(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let entity = context.get_entity(server, source, "target")?;
    let rotation = context.get_coordinates("rotation");
    let (x_rot, y_rot) = rotation.rotation(source);
    let e = server.level.entity(entity);
    let (relative_y, relative_x) = (rotation.is_y_relative(), rotation.is_x_relative());
    let y_rot = if relative_y { y_rot - e.y_rot } else { y_rot };
    let x_rot = if relative_x { x_rot - e.x_rot } else { x_rot };
    server.level.force_set_rotation(entity, y_rot, relative_y, x_rot, relative_x);
    Ok(success(server, source, entity))
}

fn rotate_facing(server: &mut Server, context: &CommandContext, source: &CommandSourceStack, facing: LookAt) -> Result<i32, CommandSyntaxException> {
    let entity = context.get_entity(server, source, "target")?;
    facing.perform(&mut server.level, source, entity);
    Ok(success(server, source, entity))
}

pub fn rotate_facing_entity(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let facing = context.get_entity(server, source, "facingEntity")?;
    rotate_facing(server, context, source, LookAt::Entity { entity: facing, anchor: Anchor::Feet })
}

pub fn rotate_facing_entity_anchor(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let facing = context.get_entity(server, source, "facingEntity")?;
    rotate_facing(server, context, source, LookAt::Entity { entity: facing, anchor: context.get_anchor("facingAnchor") })
}

pub fn rotate_facing_location(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let pos = context.get_coordinates("facingLocation").position(server, source);
    rotate_facing(server, context, source, LookAt::Position(pos))
}
