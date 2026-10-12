use crisp_core::random::LegacyRandomSource;
use crisp_core::{BlockPos, ChunkPos};

use crate::commands::arguments::coordinates::{ERROR_NOT_LOADED, get_loaded_block_pos};
use crate::commands::arguments::resource_or_id_argument::RawResourceOrId;
use crate::commands::brigadier::{CommandContext, CommandSyntaxException, DynamicCommandExceptionType, SimpleCommandExceptionType, tr};
use crate::commands::source::CommandSourceStack;
use crate::commands::suggestion::{Suggestions, SuggestionsBuilder};
use crate::commands::shared_suggestion_provider;
use crate::core::holder_set::HolderSet;
use crate::network::chat::Component;
use crate::server::Server;
use crate::server::level::ServerLevel;
use crate::world::level::block::mirror::Mirror;
use crate::world::level::block::rotation::Rotation;
use crate::world::level::levelgen::feature::feature::{self, Feature};
use crate::world::level::levelgen::structure::BoundingBox;
use crate::world::level::levelgen::structure::pools::alias::PoolAliasLookup;
use crate::world::level::levelgen::structure::pools::jigsaw_placement::{self, DimensionPadding, JigsawSettings, MaxDistance};
use crate::world::level::levelgen::structure::pools::structure_template_pool::TemplatePoolHolder;
use crate::world::level::levelgen::structure::structure::{GenerationContext, StructureHolder};
use crate::world::level::levelgen::structure::structures::Piece;
use crate::world::level::levelgen::structure::templatesystem::structure_place_settings::StructurePlaceSettings;
use crate::world::level::levelgen::structure::templatesystem::structure_processor::StructureProcessor;
use crate::world::level::levelgen::structure::templatesystem::structure_template_manager;

static ERROR_FEATURE_FAILED: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.place.feature.failed");
static ERROR_JIGSAW_FAILED: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.place.jigsaw.failed");
static ERROR_STRUCTURE_FAILED: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.place.structure.failed");
static ERROR_TEMPLATE_INVALID: DynamicCommandExceptionType = DynamicCommandExceptionType(|value| tr("commands.place.template.invalid", vec![value]));
static ERROR_TEMPLATE_FAILED: SimpleCommandExceptionType = SimpleCommandExceptionType("commands.place.template.failed");
static ERROR_INVALID_STRUCTURE: DynamicCommandExceptionType = DynamicCommandExceptionType(|value| tr("commands.place.structure.invalid", vec![value]));
static ERROR_INVALID_TEMPLATE_POOL: DynamicCommandExceptionType = DynamicCommandExceptionType(|value| tr("commands.place.jigsaw.invalid", vec![value]));

type Result<T> = std::result::Result<T, CommandSyntaxException>;

pub fn suggest_templates(_server: &Server, _context: &CommandContext, _source: &CommandSourceStack, mut builder: SuggestionsBuilder) -> Result<Suggestions> {
    shared_suggestion_provider::suggest_resource(structure_template_manager::list_templates().iter().map(String::as_str), &mut builder);
    Ok(builder.build())
}

fn source_block_pos(source: &CommandSourceStack) -> BlockPos {
    BlockPos::containing(source.position.x, source.position.y, source.position.z)
}

fn infinite() -> BoundingBox {
    BoundingBox::new(i32::MIN, i32::MIN, i32::MIN, i32::MAX, i32::MAX, i32::MAX)
}

pub fn place_feature_here(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    place_feature(server, source, context.get_resource_or_id("feature"), source_block_pos(source))
}

pub fn place_feature_at(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    let pos = get_loaded_block_pos(server, context, source, "pos")?;
    place_feature(server, source, context.get_resource_or_id("feature"), pos)
}

pub fn place_feature(server: &mut Server, source: &CommandSourceStack, feature_holder: &RawResourceOrId, pos: BlockPos) -> Result<i32> {
    let level = &mut server.levels[source.level];
    let chunk_pos = pos.chunk();
    check_loaded(level, ChunkPos::new(chunk_pos.x - 1, chunk_pos.z - 1), ChunkPos::new(chunk_pos.x + 1, chunk_pos.z + 1))?;
    let inline;
    let feature: &Feature = match feature_holder {
        RawResourceOrId::Reference(id) => feature::feature(id).expect("registered feature"),
        RawResourceOrId::Inline(value) => {
            inline = Feature::parse_nbt(value).expect("validated feature");
            &inline
        }
    };
    if !level.with_level_random(|level, random| feature.place(level, random, pos)) {
        return Err(ERROR_FEATURE_FAILED.create());
    }
    match feature_holder {
        RawResourceOrId::Reference(id) => source.send_success(
            server,
            || Component::translatable_with("commands.place.feature.success", vec![id.as_str().into(), pos.x.into(), pos.y.into(), pos.z.into()]),
            true,
        ),
        RawResourceOrId::Inline(_) => {
            source.send_success(server, || Component::translatable_with("commands.place.feature.success.inline", vec![pos.x.into(), pos.y.into(), pos.z.into()]), true)
        }
    }
    Ok(1)
}

pub fn place_jigsaw_here(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    let pool = get_structure_template_pool(context, "pool")?;
    place_jigsaw(server, source, pool, context.get_identifier("target"), context.get_int("max_depth"), source_block_pos(source))
}

pub fn place_jigsaw_at(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    let pool = get_structure_template_pool(context, "pool")?;
    let pos = get_loaded_block_pos(server, context, source, "position")?;
    place_jigsaw(server, source, pool, context.get_identifier("target"), context.get_int("max_depth"), pos)
}

fn get_structure_template_pool(context: &CommandContext, name: &str) -> Result<TemplatePoolHolder> {
    let id = context.get_identifier(name);
    TemplatePoolHolder::by_name(id).ok_or_else(|| ERROR_INVALID_TEMPLATE_POOL.create(id))
}

pub fn place_jigsaw(server: &mut Server, source: &CommandSourceStack, pool: TemplatePoolHolder, target: &str, max_depth: i32, pos: BlockPos) -> Result<i32> {
    let level = &mut server.levels[source.level];
    let chunk = pos.chunk();
    check_loaded(level, chunk, chunk)?;
    if !generate_jigsaw(level, pool, target, max_depth, pos, false) {
        return Err(ERROR_JIGSAW_FAILED.create());
    }
    source.send_success(server, || Component::translatable_with("commands.place.jigsaw.success", vec![pos.x.into(), pos.y.into(), pos.z.into()]), true);
    Ok(1)
}

fn generate_jigsaw(level: &mut ServerLevel, pool: TemplatePoolHolder, target: &str, max_depth: i32, position: BlockPos, keep_jigsaws: bool) -> bool {
    let generator = level.chunks.generator.clone();
    let all_biomes = HolderSet::Direct((0..crisp_data::SyncedRegistry::get("minecraft:worldgen/biome").entries.len() as i32).collect());
    let (seed, min_y, height) = (level.seed, level.chunks.min_y(), level.chunks.height());
    let mut generation_context = GenerationContext::new(&generator, seed, position.chunk(), min_y, height, &all_biomes);
    let settings = JigsawSettings {
        start_pool: pool,
        start_jigsaw_name: Some(target),
        max_depth,
        use_expansion_hack: false,
        project_start_to_heightmap: None,
        max_distance_from_center: MaxDistance { horizontal: 128, vertical: 128 },
        dimension_padding: DimensionPadding::ZERO,
        apply_waterlogging: true,
    };
    let Some(stub) = jigsaw_placement::add_pieces(&mut generation_context, settings, position, PoolAliasLookup::create(&[], position, seed)) else { return false };
    let builder = stub.get_pieces_builder(&mut generation_context);
    for piece in builder.build().pieces {
        if let Piece::Jigsaw(mut pool_piece) = piece {
            level.with_level_random(|level, random| pool_piece.place(level, random, &infinite(), position, keep_jigsaws));
        }
    }
    true
}

pub fn place_structure_here(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    let structure = get_structure(context, "structure")?;
    place_structure(server, source, structure, source_block_pos(source))
}

pub fn place_structure_at(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    let structure = get_structure(context, "structure")?;
    let pos = get_loaded_block_pos(server, context, source, "pos")?;
    place_structure(server, source, structure, pos)
}

fn get_structure(context: &CommandContext, name: &str) -> Result<StructureHolder> {
    let id = context.get_identifier(name);
    StructureHolder::by_name(id).ok_or_else(|| ERROR_INVALID_STRUCTURE.create(id))
}

pub fn place_structure(server: &mut Server, source: &CommandSourceStack, structure_holder: StructureHolder, pos: BlockPos) -> Result<i32> {
    let level = &mut server.levels[source.level];
    let structure = structure_holder.value();
    let chunk_generator = level.chunks.generator.clone();
    let all_biomes = HolderSet::Direct((0..crisp_data::SyncedRegistry::get("minecraft:worldgen/biome").entries.len() as i32).collect());
    let (seed, min_y, height) = (level.seed, level.chunks.min_y(), level.chunks.height());
    let Some(start) = structure.generate(structure_holder, &chunk_generator, seed, pos.chunk(), 0, min_y, height, &all_biomes) else {
        return Err(ERROR_STRUCTURE_FAILED.create());
    };
    let bounding_box = start.get_bounding_box();
    let chunk_min = ChunkPos::new(bounding_box.min.x >> 4, bounding_box.min.z >> 4);
    let chunk_max = ChunkPos::new(bounding_box.max.x >> 4, bounding_box.max.z >> 4);
    check_loaded(level, chunk_min, chunk_max)?;
    for c in range_closed(chunk_min, chunk_max) {
        let chunk_bb = BoundingBox::new(c.min_block_x(), min_y, c.min_block_z(), c.min_block_x() + 15, min_y + height, c.min_block_z() + 15);
        level.with_level_random(|level, random| start.place_in_chunk(level, random, &chunk_bb, c));
    }
    let id = structure_holder.key();
    source.send_success(
        server,
        || Component::translatable_with("commands.place.structure.success", vec![id.into(), pos.x.into(), pos.y.into(), pos.z.into()]),
        true,
    );
    Ok(1)
}

pub fn place_template_here(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    place_template(server, source, context.get_identifier("template"), source_block_pos(source), Rotation::None, Mirror::None, 1.0, 0, false)
}

pub fn place_template_with(server: &mut Server, context: &CommandContext, source: &CommandSourceStack) -> Result<i32> {
    let pos = get_loaded_block_pos(server, context, source, "pos")?;
    let rotation = if context.has_argument("rotation") { context.get_rotation("rotation") } else { Rotation::None };
    let mirror = if context.has_argument("mirror") { context.get_mirror("mirror") } else { Mirror::None };
    let integrity = if context.has_argument("integrity") { context.get_float("integrity") } else { 1.0 };
    let seed = if context.has_argument("seed") { context.get_int("seed") } else { 0 };
    let strict = context.nodes.iter().any(|n| crisp_data::commands::CommandNode::get(n.node).name == "strict");
    place_template(server, source, context.get_identifier("template"), pos, rotation, mirror, integrity, seed, strict)
}

fn create_random(seed: i64) -> LegacyRandomSource {
    if seed == 0 { LegacyRandomSource::create() } else { LegacyRandomSource::new(seed) }
}

#[allow(clippy::too_many_arguments)]
pub fn place_template(
    server: &mut Server,
    source: &CommandSourceStack,
    template: &str,
    pos: BlockPos,
    rotation: Rotation,
    mirror: Mirror,
    integrity: f32,
    seed: i32,
    strict: bool,
) -> Result<i32> {
    let level = &mut server.levels[source.level];
    let Some(structure_template) = structure_template_manager::get(template) else {
        return Err(ERROR_TEMPLATE_INVALID.create(template));
    };
    let size = structure_template.get_size();
    check_loaded(level, pos.chunk(), pos.offset(size.x, size.y, size.z).chunk())?;
    let block_rot_processor = StructureProcessor::BlockRot { rottable_blocks: None, integrity };
    let mut place_settings = StructurePlaceSettings { mirror, rotation, known_shape: strict, ..StructurePlaceSettings::default() };
    if integrity < 1.0 {
        place_settings.processors = vec![&block_rot_processor];
        place_settings.random = true;
    }
    let mut random = create_random(seed as i64);
    if !structure_template.place_in_world(level, pos, pos, &place_settings, &mut random, 2 | if strict { 816 } else { 0 }) {
        return Err(ERROR_TEMPLATE_FAILED.create());
    }
    source.send_success(
        server,
        || Component::translatable_with("commands.place.template.success", vec![Component::literal(template).into(), pos.x.into(), pos.y.into(), pos.z.into()]),
        true,
    );
    Ok(1)
}

fn check_loaded(level: &ServerLevel, chunk_min: ChunkPos, chunk_max: ChunkPos) -> Result<()> {
    if range_closed(chunk_min, chunk_max).any(|c| !level.is_loaded(BlockPos::new(c.min_block_x(), 0, c.min_block_z()))) {
        return Err(ERROR_NOT_LOADED.create());
    }
    Ok(())
}

fn range_closed(from: ChunkPos, to: ChunkPos) -> impl Iterator<Item = ChunkPos> {
    let x_diff = if from.x < to.x { 1 } else { -1 };
    let z_diff = if from.z < to.z { 1 } else { -1 };
    let x_size = (from.x - to.x).abs() + 1;
    let z_size = (from.z - to.z).abs() + 1;
    (0..z_size).flat_map(move |dz| (0..x_size).map(move |dx| ChunkPos::new(from.x + dx * x_diff, from.z + dz * z_diff)))
}
