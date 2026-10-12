use crisp_core::{BlockPos, ChunkPos};
use crisp_data::game_rule;

use crate::commands::arguments::coordinates::get_loaded_block_pos;
use crate::commands::arguments::{registry_tag, ERROR_UNKNOWN_RESOURCE};
use crate::commands::brigadier::{CommandContext, CommandSyntaxException, Dynamic2CommandExceptionType, SimpleCommandExceptionType, tr};
use crate::commands::source::CommandSourceStack;
use crate::network::chat::{Arg, Component};
use crate::server::Server;
use crate::server::level::ServerLevel;
use crate::world::level::chunk::Biome;
use crate::world::level::levelgen::structure::BoundingBox;

pub static ERROR_NOT_LOADED: SimpleCommandExceptionType = SimpleCommandExceptionType("argument.pos.unloaded");
static ERROR_VOLUME_TOO_LARGE: Dynamic2CommandExceptionType = Dynamic2CommandExceptionType(|max, count| tr("commands.fillbiome.toobig", vec![max, count]));
pub static ERROR_NO_BIOMES_SET: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.fillbiome.no_changes");

type Result<T> = std::result::Result<T, CommandSyntaxException>;

fn quantize_coord(block_coord: i32) -> i32 {
    (block_coord >> 2) << 2
}

fn quantize(block: BlockPos) -> BlockPos {
    BlockPos::new(quantize_coord(block.x), quantize_coord(block.y), quantize_coord(block.z))
}

pub fn fill(
    level: &mut ServerLevel,
    raw_from: BlockPos,
    raw_to: BlockPos,
    biome: Biome,
    filter: impl Fn(Biome) -> bool,
    success_message_consumer: impl FnOnce(Component),
) -> Result<i32> {
    let from = quantize(raw_from);
    let to = quantize(raw_to);
    let region = BoundingBox::from_corners(from, to);
    let volume = region.x_span() as i64 * region.y_span() as i64 * region.z_span() as i64;
    let limit = level.game_rules.get_int(game_rule::MAX_BLOCK_MODIFICATIONS);
    if volume > limit as i64 {
        return Err(ERROR_VOLUME_TOO_LARGE.create(limit, Arg::Long(volume)));
    }
    let mut chunks = Vec::new();
    for chunk_z in region.min.z >> 4..=region.max.z >> 4 {
        for chunk_x in region.min.x >> 4..=region.max.x >> 4 {
            let pos = ChunkPos::new(chunk_x, chunk_z);
            if level.chunks.loaded_chunk(pos).is_none() {
                return Err(ERROR_NOT_LOADED.create());
            }
            chunks.push(pos);
        }
    }
    let mut changed_count = 0;
    chunks.retain(|&pos| {
        let chunk = level.chunks.loaded_chunk_mut(pos).expect("full chunk");
        let previous_changed_count = changed_count;
        chunk.fill_biomes_from_noise(&mut |chunk, quart_x, quart_y, quart_z| {
            let (block_x, block_y, block_z) = (quart_x << 2, quart_y << 2, quart_z << 2);
            let current_biome = chunk.noise_biome(quart_x, quart_y, quart_z);
            if region.is_inside(BlockPos::new(block_x, block_y, block_z)) && filter(current_biome) {
                if current_biome != biome {
                    changed_count += 1;
                }
                biome
            } else {
                current_biome
            }
        });
        if previous_changed_count != changed_count {
            chunk.unsaved = true;
            true
        } else {
            false
        }
    });
    if changed_count > 0 {
        level.resend_biomes_for_chunks(&chunks);
        success_message_consumer(Component::translatable_with(
            "commands.fillbiome.success.count",
            vec![changed_count.into(), region.min.x.into(), region.min.y.into(), region.min.z.into(), region.max.x.into(), region.max.y.into(), region.max.z.into()],
        ));
        Ok(changed_count)
    } else {
        Err(ERROR_NO_BIOMES_SET.create())
    }
}

pub fn fill_all(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    fill_command(server, context, source, |_| true)
}

pub fn fill_replace(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    let (tag, id) = context.get_resource_or_tag("filter");
    let biomes = crisp_data::registry_entries("minecraft:worldgen/biome").unwrap_or_default();
    let id = id.to_string();
    if tag {
        let members = registry_tag("minecraft:worldgen/biome", &id).unwrap_or_default();
        fill_command(server, context, source, move |biome| biomes.iter().position(|&b| b == biome.name()).is_some_and(|i| members.contains(&(i as i32))))
    } else {
        fill_command(server, context, source, move |biome| biome.name() == id)
    }
}

fn fill_command(server: &mut Server, context: &CommandContext, source: &CommandSourceStack, filter: impl Fn(Biome) -> bool) -> Result<i32> {
    let raw_from = get_loaded_block_pos(server, context, source, "from")?;
    let raw_to = get_loaded_block_pos(server, context, source, "to")?;
    let biome_id = context.get_identifier("biome");
    let biome = Biome::by_name(biome_id).ok_or_else(|| ERROR_UNKNOWN_RESOURCE.create(biome_id, "minecraft:worldgen/biome"))?;
    let mut message = None;
    let result = fill(&mut server.levels[source.level], raw_from, raw_to, biome, filter, |m| message = Some(m))?;
    if let Some(message) = message {
        source.send_success(server, || message, true);
    }
    Ok(result)
}
