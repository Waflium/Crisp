use crisp_core::{BlockPos, Vec3};
use crisp_protocol::nbt::Nbt;

use super::teleport_command::is_in_spawnable_bounds;
use crate::commands::brigadier::{CommandContext, CommandSyntaxException, DynamicCommandExceptionType, SimpleCommandExceptionType, tr};
use crate::commands::source::CommandSourceStack;
use crate::network::chat::{Arg, Component, HoverEvent};
use crate::server::Server;
use crate::world::entity::EntityId;

static ERROR_FAILED: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.summon.failed");
static ERROR_DUPLICATE_UUID: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.summon.failed.uuid");
static INVALID_POSITION: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.summon.invalidPosition");
static ERROR_NOT_SUMMONABLE_ENTITY: DynamicCommandExceptionType = DynamicCommandExceptionType(|value| tr("entity.not_summonable", vec![value]));

const MODELLED: &[&str] = &["minecraft:item", "minecraft:tnt", "minecraft:lightning_bolt", "minecraft:experience_orb"];

pub fn summonable_entity_type(context: &CommandContext, name: &str) -> Result<String, CommandSyntaxException> {
    let id = context.get_identifier(name).to_string();
    if id == "minecraft:player" || id == "minecraft:fishing_bobber" {
        return Err(ERROR_NOT_SUMMONABLE_ENTITY.create(id));
    }
    Ok(id)
}

pub fn create_entity(server: &mut Server, entity_type: &str, pos: Vec3, nbt: &Nbt) -> Result<EntityId, CommandSyntaxException> {
    if !is_in_spawnable_bounds(BlockPos::containing(pos.x, pos.y, pos.z)) {
        return Err(INVALID_POSITION.create());
    }
    if !MODELLED.contains(&entity_type) {
        let hover = Component::literal(format!("{entity_type} entities are not implemented in Crisp yet"));
        return Err(CommandSyntaxException::new(Component::translatable("command.failed").with_hover(HoverEvent::ShowText(Box::new(hover)))));
    }
    let mut tag = match nbt {
        Nbt::Compound(fields) => fields.clone(),
        _ => Vec::new(),
    };
    tag.retain(|(k, _)| k != "id");
    tag.push(("id".into(), Nbt::String(entity_type.into())));
    let level = &mut server.level;
    let Some(mut entity) = level.load_entity(&Nbt::Compound(tag)) else { return Err(ERROR_FAILED.create()) };
    let e = entity.entity_mut();
    e.set_pos(pos);
    e.set_old_pos();
    let uuid = e.uuid;
    if level.entity_manager.is_loaded(uuid) || e.is_removed() {
        return Err(ERROR_DUPLICATE_UUID.create());
    }
    let id = e.id;
    level.add_fresh_entity(entity);
    Ok(id)
}

fn spawn_entity(server: &mut Server, source: &CommandSourceStack, entity_type: &str, pos: Vec3, nbt: &Nbt) -> Result<i32, CommandSyntaxException> {
    let id = create_entity(server, entity_type, pos, nbt)?;
    source.send_success(server, || Component::translatable_with("commands.summon.success", vec![Arg::Component(server.entity_display_name(id))]), true);
    Ok(1)
}

pub fn summon(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let entity_type = summonable_entity_type(context, "entity")?;
    spawn_entity(server, source, &entity_type, source.position, &Nbt::Compound(Vec::new()))
}

pub fn summon_at(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let entity_type = summonable_entity_type(context, "entity")?;
    let pos = context.get_coordinates("pos").position(server, source);
    spawn_entity(server, source, &entity_type, pos, &Nbt::Compound(Vec::new()))
}

pub fn summon_with_nbt(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let entity_type = summonable_entity_type(context, "entity")?;
    let pos = context.get_coordinates("pos").position(server, source);
    let nbt = context.get_nbt("nbt").clone();
    spawn_entity(server, source, &entity_type, pos, &nbt)
}
