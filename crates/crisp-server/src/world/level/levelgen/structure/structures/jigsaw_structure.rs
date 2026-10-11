use crisp_core::BlockPos;
use serde_json::Value;

use crate::world::level::chunk::HeightmapType;
use crate::world::level::levelgen::blockpredicates::block_predicate::resource;
use crate::world::level::levelgen::heightproviders::HeightProvider;
use crate::world::level::levelgen::material::WorldGenerationContext;
use crate::world::level::levelgen::placement::placement_modifier::parse_heightmap;
use crate::world::level::levelgen::structure::pools::alias::{PoolAliasBinding, PoolAliasLookup};
use crate::world::level::levelgen::structure::pools::jigsaw_placement::{self, DimensionPadding, JigsawSettings, MaxDistance};
use crate::world::level::levelgen::structure::pools::structure_template_pool::TemplatePoolHolder;
use crate::world::level::levelgen::structure::structure::{GenerationContext, GenerationStub};

const Y_SIZE: i32 = (1 << 12) - 32;

pub struct JigsawStructure {
    start_pool: TemplatePoolHolder,
    start_jigsaw_name: Option<String>,
    max_depth: i32,
    start_height: HeightProvider,
    use_expansion_hack: bool,
    project_start_to_heightmap: Option<HeightmapType>,
    max_distance_from_center: MaxDistance,
    pool_aliases: Vec<PoolAliasBinding>,
    dimension_padding: DimensionPadding,
    apply_waterlogging: bool,
}

impl JigsawStructure {
    pub fn parse(json: &Value) -> Result<JigsawStructure, String> {
        let start_pool_name = json.get("start_pool").and_then(Value::as_str).ok_or("missing start_pool")?;
        let max_distance_from_center = match json.get("max_distance_from_center").ok_or("missing max_distance_from_center")? {
            Value::Number(n) => {
                let value = n.as_i64().ok_or("bad max distance")? as i32;
                MaxDistance { horizontal: value, vertical: value }
            }
            other => MaxDistance {
                horizontal: other.get("horizontal").and_then(Value::as_i64).ok_or("missing horizontal")? as i32,
                vertical: other.get("vertical").and_then(Value::as_i64).map_or(Y_SIZE, |v| v as i32),
            },
        };
        let dimension_padding = match json.get("dimension_padding") {
            None => DimensionPadding::ZERO,
            Some(Value::Number(n)) => {
                let value = n.as_i64().ok_or("bad dimension padding")? as i32;
                DimensionPadding { bottom: value, top: value }
            }
            Some(other) => DimensionPadding {
                bottom: other.get("bottom").and_then(Value::as_i64).unwrap_or(0) as i32,
                top: other.get("top").and_then(Value::as_i64).unwrap_or(0) as i32,
            },
        };
        Ok(JigsawStructure {
            start_pool: TemplatePoolHolder::by_name(start_pool_name).ok_or_else(|| format!("unknown pool {start_pool_name}"))?,
            start_jigsaw_name: json.get("start_jigsaw_name").and_then(Value::as_str).map(resource),
            max_depth: json.get("size").and_then(Value::as_i64).ok_or("missing size")? as i32,
            start_height: HeightProvider::parse(json.get("start_height").ok_or("missing start_height")?)?,
            use_expansion_hack: json.get("use_expansion_hack").and_then(Value::as_bool).ok_or("missing use_expansion_hack")?,
            project_start_to_heightmap: json.get("project_start_to_heightmap").and_then(Value::as_str).map(parse_heightmap).transpose()?,
            max_distance_from_center,
            pool_aliases: json.get("pool_aliases").and_then(Value::as_array).map_or(Ok(Vec::new()), |list| list.iter().map(PoolAliasBinding::parse).collect())?,
            dimension_padding,
            apply_waterlogging: json.get("liquid_settings").and_then(Value::as_str) != Some("ignore_waterlogging"),
        })
    }

    pub fn find_generation_point(&self, context: &mut GenerationContext) -> Option<GenerationStub<'static>> {
        let chunk_pos = context.chunk_pos;
        let generation = WorldGenerationContext {
            min_gen_y: context.min_y.max(context.chunk_generator.min_y()),
            gen_depth: context.height.min(context.chunk_generator.gen_depth()),
            sea_level: context.chunk_generator.sea_level(),
        };
        let height = self.start_height.sample(&mut context.random, &generation);
        let start_pos = BlockPos::new(chunk_pos.min_block_x(), height, chunk_pos.min_block_z());
        let pool_alias_lookup = PoolAliasLookup::create(&self.pool_aliases, start_pos, context.seed);
        jigsaw_placement::add_pieces(
            context,
            JigsawSettings {
                start_pool: self.start_pool,
                start_jigsaw_name: self.start_jigsaw_name.as_deref(),
                max_depth: self.max_depth,
                use_expansion_hack: self.use_expansion_hack,
                project_start_to_heightmap: self.project_start_to_heightmap,
                max_distance_from_center: self.max_distance_from_center,
                dimension_padding: self.dimension_padding,
                apply_waterlogging: self.apply_waterlogging,
            },
            start_pos,
            pool_alias_lookup,
        )
    }
}
