use std::sync::LazyLock;

use crisp_core::{BlockPos, ChunkPos};
use serde_json::Value;

use super::BoundingBox;
use super::pieces::structure_pieces_builder::StructurePiecesBuilder;
use super::structure_start::StructureStart;
use super::structures::StructureType;
use super::terrain_adjustment::TerrainAdjustment;
use crate::core::holder_set::HolderSet;
use crate::world::level::block::rotation::Rotation;
use crate::world::level::chunk::{Biome, HeightmapType};
use crate::world::level::levelgen::blockpredicates::block_predicate::resource;
use crate::world::level::levelgen::density_function::SamplerContext;
use crate::world::level::levelgen::worldgen_random::WorldgenRandom;
use crate::world::level::levelgen::{NoiseBasedChunkGenerator, quart_pos};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct StructureHolder(pub u16);

impl StructureHolder {
    pub fn by_name(name: &str) -> Option<StructureHolder> {
        let name = resource(name);
        crisp_data::STRUCTURES.iter().position(|(n, _)| *n == name).map(|i| StructureHolder(i as u16))
    }

    pub fn key(self) -> &'static str {
        crisp_data::STRUCTURES[self.0 as usize].0
    }

    pub fn value(self) -> &'static Structure {
        &STRUCTURES[self.0 as usize]
    }

    pub fn all() -> impl Iterator<Item = StructureHolder> {
        (0..crisp_data::STRUCTURES.len()).map(|i| StructureHolder(i as u16))
    }
}

static STRUCTURES: LazyLock<Vec<Structure>> = LazyLock::new(|| {
    crisp_data::STRUCTURES
        .iter()
        .map(|(name, source)| {
            let json: Value = serde_json::from_str(source).expect("structure json");
            Structure::parse(&json).unwrap_or_else(|e| panic!("structure {name}: {e}"))
        })
        .collect()
});

pub struct StructureSettings {
    pub biomes: HolderSet,
    pub step: usize,
    pub terrain_adaptation: TerrainAdjustment,
}

pub const DECORATION_STEPS: [&str; 11] = [
    "raw_generation",
    "lakes",
    "local_modifications",
    "underground_structures",
    "surface_structures",
    "strongholds",
    "underground_ores",
    "underground_decoration",
    "fluid_springs",
    "vegetal_decoration",
    "top_layer_modification",
];

pub fn parse_biome_holder_set(json: &Value) -> Result<HolderSet, String> {
    let registry = crisp_data::SyncedRegistry::get("minecraft:worldgen/biome");
    let id = |name: &str| registry.id_of(&resource(name)).map(|id| id as i32).ok_or_else(|| format!("unknown biome {name}"));
    match json {
        Value::String(s) => match s.strip_prefix('#') {
            Some(tag) => Ok(HolderSet::Named(resource(tag), crisp_data::tag("minecraft:worldgen/biome", &resource(tag)))),
            None => Ok(HolderSet::Direct(vec![id(s)?])),
        },
        Value::Array(list) => Ok(HolderSet::Direct(list.iter().map(|v| id(v.as_str().ok_or("biome ids are strings")?)).collect::<Result<_, _>>()?)),
        other => Err(format!("bad biome holder set {other}")),
    }
}

impl StructureSettings {
    fn parse(json: &Value) -> Result<StructureSettings, String> {
        let step = json.get("step").and_then(Value::as_str).ok_or("missing step")?;
        Ok(StructureSettings {
            biomes: parse_biome_holder_set(json.get("biomes").ok_or("missing biomes")?)?,
            step: DECORATION_STEPS.iter().position(|s| *s == step).ok_or_else(|| format!("unknown step {step}"))?,
            terrain_adaptation: match json.get("terrain_adaptation").and_then(Value::as_str) {
                Some(name) => TerrainAdjustment::by_name(name).ok_or_else(|| format!("unknown terrain adaptation {name}"))?,
                None => TerrainAdjustment::None,
            },
        })
    }
}

pub struct Structure {
    pub settings: StructureSettings,
    pub structure_type: StructureType,
}

pub struct GenerationContext<'a> {
    pub chunk_generator: &'a NoiseBasedChunkGenerator,
    pub sampler_context: SamplerContext,
    pub random: WorldgenRandom,
    pub seed: i64,
    pub chunk_pos: ChunkPos,
    pub min_y: i32,
    pub height: i32,
    pub valid_biome: &'a HolderSet,
}

pub type PiecesGenerator<'a> = Box<dyn FnOnce(&mut StructurePiecesBuilder, &mut GenerationContext) + 'a>;

pub struct GenerationStub<'a> {
    pub position: BlockPos,
    pub generator: PiecesGenerator<'a>,
}

impl GenerationStub<'_> {
    pub fn get_pieces_builder(self, context: &mut GenerationContext) -> StructurePiecesBuilder {
        let mut builder = StructurePiecesBuilder::default();
        (self.generator)(&mut builder, context);
        builder
    }
}

impl<'a> GenerationContext<'a> {
    pub fn new(chunk_generator: &'a NoiseBasedChunkGenerator, seed: i64, chunk_pos: ChunkPos, min_y: i32, height: i32, valid_biome: &'a HolderSet) -> GenerationContext<'a> {
        let mut random = WorldgenRandom::legacy(0);
        random.set_large_feature_seed(seed, chunk_pos.x, chunk_pos.z);
        GenerationContext { chunk_generator, sampler_context: SamplerContext::cached(), random, seed, chunk_pos, min_y, height, valid_biome }
    }

    pub fn max_y(&self) -> i32 {
        self.min_y + self.height - 1
    }

    pub fn get_noise_biome(&mut self, quart_x: i32, quart_y: i32, quart_z: i32) -> Biome {
        self.chunk_generator.get_noise_biome(&mut self.sampler_context, quart_x, quart_y, quart_z)
    }

    pub fn is_valid_biome(&mut self, stub: &GenerationStub) -> bool {
        let pos = stub.position;
        let biome = self.get_noise_biome(quart_pos::from_block(pos.x), quart_pos::from_block(pos.y), quart_pos::from_block(pos.z));
        self.valid_biome.contains(biome.0 as i32)
    }

    pub fn could_structure_exist_in_column(&mut self, block_x: i32, block_z: i32, min_block_y: i32, max_block_y: i32) -> bool {
        let quart_x = quart_pos::from_block(block_x);
        let quart_z = quart_pos::from_block(block_z);
        let min_quart_y = quart_pos::from_block(min_block_y);
        let max_quart_y = quart_pos::from_block(max_block_y);
        (min_quart_y..=max_quart_y).any(|quart_y| {
            let biome = self.get_noise_biome(quart_x, quart_y, quart_z);
            self.valid_biome.contains(biome.0 as i32)
        })
    }

    pub fn could_valid_biome_exist_in_terrain_column(&mut self, block_x: i32, block_z: i32) -> bool {
        self.could_structure_exist_in_column(block_x, block_z, self.min_y - 1, self.max_y())
    }

    pub fn could_valid_biome_exist_on_top_of_chunk_center(&mut self) -> bool {
        self.could_valid_biome_exist_in_terrain_column(self.chunk_pos.min_block_x() + 8, self.chunk_pos.min_block_z() + 8)
    }

    pub fn get_first_occupied_height(&self, x: i32, z: i32, heightmap: HeightmapType) -> i32 {
        self.chunk_generator.get_first_occupied_height(x, z, heightmap, self.min_y, self.height)
    }
}

pub fn on_top_of_chunk_center<'a>(context: &mut GenerationContext, heightmap: HeightmapType, generator: PiecesGenerator<'a>) -> Option<GenerationStub<'a>> {
    if !context.could_valid_biome_exist_on_top_of_chunk_center() {
        return None;
    }
    on_top_of_chunk_center_without_biome_check(context, heightmap, generator)
}

pub fn on_top_of_chunk_center_without_biome_check<'a>(context: &mut GenerationContext, heightmap: HeightmapType, generator: PiecesGenerator<'a>) -> Option<GenerationStub<'a>> {
    let block_x = context.chunk_pos.min_block_x() + 8;
    let block_z = context.chunk_pos.min_block_z() + 8;
    let block_y = context.get_first_occupied_height(block_x, block_z, heightmap);
    Some(GenerationStub { position: BlockPos::new(block_x, block_y, block_z), generator })
}

fn get_corner_heights(context: &GenerationContext, min_x: i32, size_x: i32, min_z: i32, size_z: i32) -> [i32; 4] {
    [
        context.get_first_occupied_height(min_x, min_z, HeightmapType::WorldSurfaceWg),
        context.get_first_occupied_height(min_x, min_z + size_z, HeightmapType::WorldSurfaceWg),
        context.get_first_occupied_height(min_x + size_x, min_z, HeightmapType::WorldSurfaceWg),
        context.get_first_occupied_height(min_x + size_x, min_z + size_z, HeightmapType::WorldSurfaceWg),
    ]
}

pub fn get_mean_first_occupied_height(context: &GenerationContext, min_x: i32, size_x: i32, min_z: i32, size_z: i32) -> i32 {
    let corner_heights = get_corner_heights(context, min_x, size_x, min_z, size_z);
    (corner_heights[0] + corner_heights[1] + corner_heights[2] + corner_heights[3]) / 4
}

pub fn get_lowest_y(context: &GenerationContext, size_x: i32, size_z: i32) -> i32 {
    get_lowest_y_at(context, context.chunk_pos.min_block_x(), context.chunk_pos.min_block_z(), size_x, size_z)
}

pub fn get_lowest_y_at(context: &GenerationContext, min_x: i32, min_z: i32, size_x: i32, size_z: i32) -> i32 {
    let corner_heights = get_corner_heights(context, min_x, size_x, min_z, size_z);
    corner_heights[0].min(corner_heights[1]).min(corner_heights[2].min(corner_heights[3]))
}

pub fn get_lowest_y_in_5_by_5_box(context: &GenerationContext, block_x: i32, block_z: i32, rotation: Rotation) -> BlockPos {
    let (offset_x, offset_z) = match rotation {
        Rotation::Clockwise90 => (-5, 5),
        Rotation::Clockwise180 => (-5, -5),
        Rotation::Counterclockwise90 => (5, -5),
        Rotation::None => (5, 5),
    };
    BlockPos::new(block_x, get_lowest_y_at(context, block_x, block_z, offset_x, offset_z), block_z)
}

impl Structure {
    fn parse(json: &Value) -> Result<Structure, String> {
        Ok(Structure { settings: StructureSettings::parse(json)?, structure_type: StructureType::parse(json)? })
    }

    pub fn biomes(&self) -> &HolderSet {
        &self.settings.biomes
    }

    pub fn step(&self) -> usize {
        self.settings.step
    }

    pub fn terrain_adaptation(&self) -> TerrainAdjustment {
        self.settings.terrain_adaptation
    }

    pub fn adjust_bounding_box(&self, bounding_box: BoundingBox) -> BoundingBox {
        if self.terrain_adaptation() != TerrainAdjustment::None { bounding_box.inflated_by(12) } else { bounding_box }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn generate(
        &self,
        selected: StructureHolder,
        chunk_generator: &NoiseBasedChunkGenerator,
        seed: i64,
        source_chunk_pos: ChunkPos,
        references: i32,
        min_y: i32,
        height: i32,
        valid_biome: &HolderSet,
    ) -> Option<StructureStart> {
        let mut context = GenerationContext::new(chunk_generator, seed, source_chunk_pos, min_y, height, valid_biome);
        let generation = self.find_valid_generation_point(&mut context)?;
        let builder = generation.get_pieces_builder(&mut context);
        let start = StructureStart::new(selected, source_chunk_pos, references, builder.build());
        start.is_valid().then_some(start)
    }

    pub fn find_valid_generation_point<'a>(&'a self, context: &mut GenerationContext) -> Option<GenerationStub<'a>> {
        self.structure_type.find_generation_point(context).filter(|stub| context.is_valid_biome(stub))
    }
}
