use crisp_core::{BlockPos, Vec3};
use crisp_protocol::nbt::Nbt;

use crate::world::level::is_in_spawnable_bounds;
use crate::commands::brigadier::{CommandContext, CommandSyntaxException, DynamicCommandExceptionType, SimpleCommandExceptionType, tr};
use crate::commands::source::CommandSourceStack;
use crate::network::chat::{Arg, Component, HoverEvent};
use crate::server::Server;
use crate::world::entity::{EntityId, LevelEntity};

static ERROR_FAILED: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.summon.failed");
static ERROR_DUPLICATE_UUID: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.summon.failed.uuid");
static INVALID_POSITION: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.summon.invalidPosition");
static ERROR_NOT_SUMMONABLE_ENTITY: DynamicCommandExceptionType = DynamicCommandExceptionType(|value| tr("entity.not_summonable", vec![value]));

const MODELLED: &[&str] = &[
    "minecraft:item",
    "minecraft:tnt",
    "minecraft:falling_block",
    "minecraft:lightning_bolt",
    "minecraft:experience_orb",
    "minecraft:chicken",
    "minecraft:cow",
    "minecraft:mooshroom",
    "minecraft:pig",
    "minecraft:sheep",
    "minecraft:snowball",
    "minecraft:egg",
    "minecraft:ender_pearl",
    "minecraft:experience_bottle",
    "minecraft:splash_potion",
    "minecraft:lingering_potion",
    "minecraft:arrow",
    "minecraft:spectral_arrow",
    "minecraft:firework_rocket",
    "minecraft:area_effect_cloud",
    "minecraft:minecart",
    "minecraft:acacia_boat",
    "minecraft:birch_boat",
    "minecraft:cherry_boat",
    "minecraft:dark_oak_boat",
    "minecraft:jungle_boat",
    "minecraft:mangrove_boat",
    "minecraft:oak_boat",
    "minecraft:pale_oak_boat",
    "minecraft:poplar_boat",
    "minecraft:spruce_boat",
    "minecraft:bamboo_raft",
    "minecraft:acacia_chest_boat",
    "minecraft:birch_chest_boat",
    "minecraft:cherry_chest_boat",
    "minecraft:dark_oak_chest_boat",
    "minecraft:jungle_chest_boat",
    "minecraft:mangrove_chest_boat",
    "minecraft:oak_chest_boat",
    "minecraft:pale_oak_chest_boat",
    "minecraft:poplar_chest_boat",
    "minecraft:spruce_chest_boat",
    "minecraft:bamboo_chest_raft",
];

pub fn summonable_entity_type(context: &CommandContext, name: &str) -> Result<String, CommandSyntaxException> {
    let id = context.get_identifier(name).to_string();
    if id == "minecraft:player" || id == "minecraft:fishing_bobber" {
        return Err(ERROR_NOT_SUMMONABLE_ENTITY.create(id));
    }
    Ok(id)
}

pub fn create_entity(server: &mut Server, source: &CommandSourceStack, entity_type: &str, pos: Vec3, nbt: &Nbt, finalize: bool) -> Result<EntityId, CommandSyntaxException> {
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
    let level = &mut server.levels[source.level];
    let mut loaded = level.load_entity_recursive(&Nbt::Compound(tag), &mut |entity| {
        let e = entity.entity_mut();
        let (y_rot, x_rot) = (e.y_rot, e.x_rot);
        e.snap_to(pos.x, pos.y, pos.z, y_rot, x_rot);
    });
    let Some(entity) = loaded.first_mut() else { return Err(ERROR_FAILED.create()) };
    if finalize && let LevelEntity::Mob(mob) = entity {
        level.finalize_mob_spawn(mob);
    }
    if loaded.iter().any(|e| level.entity_manager.is_loaded(e.entity().uuid)) || loaded[0].entity().is_removed() {
        return Err(ERROR_DUPLICATE_UUID.create());
    }
    let id = loaded[0].entity().id;
    for entity in loaded {
        level.add_fresh_entity(entity);
    }
    Ok(id)
}

fn spawn_entity(server: &mut Server, source: &CommandSourceStack, entity_type: &str, pos: Vec3, nbt: &Nbt, finalize: bool) -> Result<i32, CommandSyntaxException> {
    let id = create_entity(server, source, entity_type, pos, nbt, finalize)?;
    source.send_success(server, || Component::translatable_with("commands.summon.success", vec![Arg::Component(server.entity_display_name(id))]), true);
    Ok(1)
}

pub fn summon(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let entity_type = summonable_entity_type(context, "entity")?;
    spawn_entity(server, source, &entity_type, source.position, &Nbt::Compound(Vec::new()), true)
}

pub fn summon_at(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let entity_type = summonable_entity_type(context, "entity")?;
    let pos = context.get_coordinates("pos").position(server, source);
    spawn_entity(server, source, &entity_type, pos, &Nbt::Compound(Vec::new()), true)
}

pub fn summon_with_nbt(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let entity_type = summonable_entity_type(context, "entity")?;
    let pos = context.get_coordinates("pos").position(server, source);
    let nbt = context.get_nbt("nbt").clone();
    spawn_entity(server, source, &entity_type, pos, &nbt, false)
}
