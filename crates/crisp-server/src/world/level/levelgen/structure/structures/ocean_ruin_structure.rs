use crisp_core::BlockPos;
use serde_json::Value;

use super::ocean_ruin_pieces;
use crate::util::valueproviders::float;
use crate::world::level::block::rotation::Rotation;
use crate::world::level::chunk::HeightmapType;
use crate::world::level::levelgen::structure::pieces::structure_pieces_builder::StructurePiecesBuilder;
use crate::world::level::levelgen::structure::structure::{GenerationContext, GenerationStub, on_top_of_chunk_center};

#[derive(Clone, Copy)]
pub struct OceanRuinStructure {
    pub biome_temp: Type,
    pub large_probability: f32,
    pub cluster_probability: f32,
}

impl OceanRuinStructure {
    pub fn parse(json: &Value) -> Result<OceanRuinStructure, String> {
        Ok(OceanRuinStructure {
            biome_temp: json.get("biome_temp").and_then(Value::as_str).and_then(Type::by_name).ok_or("missing biome_temp")?,
            large_probability: float(json.get("large_probability").ok_or("missing large_probability")?)?,
            cluster_probability: float(json.get("cluster_probability").ok_or("missing cluster_probability")?)?,
        })
    }

    pub fn find_generation_point(self, context: &mut GenerationContext) -> Option<GenerationStub<'static>> {
        on_top_of_chunk_center(context, HeightmapType::OceanFloorWg, Box::new(move |builder, context| self.generate_pieces(builder, context)))
    }

    fn generate_pieces(self, builder: &mut StructurePiecesBuilder, context: &mut GenerationContext) {
        let offset = BlockPos::new(context.chunk_pos.min_block_x(), 90, context.chunk_pos.min_block_z());
        let rotation = Rotation::get_random(&mut context.random);
        ocean_ruin_pieces::add_pieces(offset, rotation, builder, &mut context.random, &self);
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Type {
    Warm,
    Cold,
}

impl Type {
    pub fn by_name(name: &str) -> Option<Type> {
        match name {
            "warm" => Some(Type::Warm),
            "cold" => Some(Type::Cold),
            _ => None,
        }
    }

    pub fn legacy_name(self) -> &'static str {
        match self {
            Type::Warm => "WARM",
            Type::Cold => "COLD",
        }
    }

    pub fn by_legacy_name(name: &str) -> Option<Type> {
        match name {
            "WARM" => Some(Type::Warm),
            "COLD" => Some(Type::Cold),
            _ => None,
        }
    }
}
