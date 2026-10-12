use std::time::{Duration, Instant};

use crisp_core::BlockPos;

use crate::commands::arguments::registry_tag;
use crate::commands::brigadier::{CommandContext, CommandSyntaxException, DynamicCommandExceptionType, tr};
use crate::commands::source::CommandSourceStack;
use crate::network::chat::{Arg, ChatFormatting, ClickEvent, Component, HoverEvent};
use crate::server::Server;
use crate::world::level::chunk::Biome;
use crate::world::level::levelgen::structure::structure::StructureHolder;

static ERROR_STRUCTURE_NOT_FOUND: DynamicCommandExceptionType = DynamicCommandExceptionType(|value| tr("commands.locate.structure.not_found", vec![value]));
static ERROR_STRUCTURE_INVALID: DynamicCommandExceptionType = DynamicCommandExceptionType(|value| tr("commands.locate.structure.invalid", vec![value]));
static ERROR_BIOME_NOT_FOUND: DynamicCommandExceptionType = DynamicCommandExceptionType(|value| tr("commands.locate.biome.not_found", vec![value]));
const MAX_STRUCTURE_SEARCH_RADIUS: i32 = 100;
const MAX_BIOME_SEARCH_RADIUS: i32 = 6400;
const BIOME_SAMPLE_RESOLUTION_HORIZONTAL: i32 = 32;
const BIOME_SAMPLE_RESOLUTION_VERTICAL: i32 = 64;

fn as_printable(tag: bool, id: &str) -> String {
    if tag { format!("#{id}") } else { id.to_string() }
}

fn get_holders(tag: bool, id: &str) -> Option<Vec<StructureHolder>> {
    if tag {
        registry_tag("minecraft:worldgen/structure", id).map(|ids| ids.iter().map(|&id| StructureHolder(id as u16)).collect())
    } else {
        StructureHolder::by_name(id).map(|holder| vec![holder])
    }
}

pub fn locate_structure(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let (tag, id) = context.get_resource_or_tag("structure");
    let target = get_holders(tag, id).ok_or_else(|| ERROR_STRUCTURE_INVALID.create(as_printable(tag, id)))?;
    let source_pos = BlockPos::containing_vec(source.position);
    let stopwatch = Instant::now();
    let generator = server.level.chunks.generator.clone();
    let nearest = generator.find_nearest_map_structure(&mut server.level, &target, source_pos, MAX_STRUCTURE_SEARCH_RADIUS, false);
    let elapsed = stopwatch.elapsed();
    let Some((found_pos, found)) = nearest else { return Err(ERROR_STRUCTURE_NOT_FOUND.create(as_printable(tag, id))) };
    let found_name = if tag { format!("#{id} ({})", found.key()) } else { id.to_string() };
    Ok(show_locate_result(server, source, source_pos, found_pos, "commands.locate.structure.success", false, &found_name, elapsed))
}

pub fn locate_biome(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32, CommandSyntaxException> {
    let (tag, id) = context.get_resource_or_tag("biome");
    let source_pos = BlockPos::containing_vec(source.position);
    let stopwatch = Instant::now();
    let biome_test = |biome: Biome| match tag {
        true => registry_tag("minecraft:worldgen/biome", id).is_some_and(|ids| ids.contains(&(biome.0 as i32))),
        false => biome.name() == id,
    };
    let nearest = server.level.find_closest_biome_3d(
        biome_test,
        source_pos,
        MAX_BIOME_SEARCH_RADIUS,
        BIOME_SAMPLE_RESOLUTION_HORIZONTAL,
        BIOME_SAMPLE_RESOLUTION_VERTICAL,
    );
    let elapsed = stopwatch.elapsed();
    let Some((found_pos, found)) = nearest else { return Err(ERROR_BIOME_NOT_FOUND.create(as_printable(tag, id))) };
    let found_name = if tag { format!("#{id} ({})", found.name()) } else { id.to_string() };
    Ok(show_locate_result(server, source, source_pos, found_pos, "commands.locate.biome.success", true, &found_name, elapsed))
}

#[allow(clippy::too_many_arguments)]
fn show_locate_result(
    server: &Server,
    source: &CommandSourceStack,
    source_pos: BlockPos,
    found_pos: BlockPos,
    success_message_key: &str,
    include_y: bool,
    found_name: &str,
    task_duration: Duration,
) -> i32 {
    let distance = if include_y {
        crisp_core::mth::floor_f(crisp_core::mth::sqrt(source_pos.dist_sqr(found_pos) as f32))
    } else {
        crisp_core::mth::floor_f(dist(source_pos.x, source_pos.z, found_pos.x, found_pos.z))
    };
    let displayed_y = if include_y { found_pos.y.to_string() } else { "~".to_string() };
    let coordinates = Component::translatable_with(
        "chat.square_brackets",
        vec![Arg::Component(Component::translatable_with(
            "chat.coordinates",
            vec![Arg::Int(found_pos.x), Arg::String(displayed_y.clone()), Arg::Int(found_pos.z)],
        ))],
    )
    .with_color(ChatFormatting::Green)
    .with_click(ClickEvent::SuggestCommand(format!("/tp @s {} {displayed_y} {}", found_pos.x, found_pos.z)))
    .with_hover(HoverEvent::ShowText(Box::new(Component::translatable("chat.coordinates.tooltip"))));
    source.send_success(
        server,
        || Component::translatable_with(success_message_key, vec![Arg::String(found_name.to_string()), Arg::Component(coordinates), Arg::Int(distance)]),
        false,
    );
    info!("Locating element {found_name} took {} ms", task_duration.as_millis());
    distance
}

fn dist(x1: i32, z1: i32, x2: i32, z2: i32) -> f32 {
    let dx = x2.wrapping_sub(x1);
    let dz = z2.wrapping_sub(z1);
    crisp_core::mth::sqrt(dx.wrapping_mul(dx).wrapping_add(dz.wrapping_mul(dz)) as f32)
}
