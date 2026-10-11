pub mod buried_treasure_pieces;
pub mod buried_treasure_structure;
pub mod desert_pyramid_piece;
pub mod desert_pyramid_structure;
pub mod igloo_pieces;
pub mod igloo_structure;
pub mod jigsaw_structure;
pub mod jungle_temple_piece;
pub mod ocean_ruin_pieces;
pub mod ocean_ruin_structure;
pub mod shipwreck_pieces;
pub mod shipwreck_structure;
pub mod stronghold_pieces;
pub mod stronghold_structure;
pub mod swamp_hut_piece;
pub mod swamp_hut_structure;

use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, ChunkPos};
use crisp_protocol::nbt::Nbt;
use serde_json::Value;

use buried_treasure_pieces::BuriedTreasurePiece;
use desert_pyramid_piece::DesertPyramidPiece;
use igloo_pieces::IglooPiece;
use jungle_temple_piece::JungleTemplePiece;
use ocean_ruin_pieces::OceanRuinPiece;
use ocean_ruin_structure::OceanRuinStructure;
use shipwreck_pieces::ShipwreckPiece;
use stronghold_pieces::StrongholdPieceKind;
use swamp_hut_piece::SwampHutPiece;

use super::BoundingBox;
use super::pieces::pieces_container::PiecesContainer;
use super::pool_element_structure_piece::{self, PoolElementStructurePiece};
use super::single_piece_structure;
use super::structure::{GenerationContext, GenerationStub};
use super::structure_piece::StructurePiece;
use crate::server::level::world_gen_region::WorldGenRegion;

pub enum StructureType {
    BuriedTreasure,
    DesertPyramid,
    Igloo,
    Jigsaw(Box<jigsaw_structure::JigsawStructure>),
    JungleTemple,
    OceanRuin(OceanRuinStructure),
    Shipwreck { is_beached: bool },
    Stronghold,
    SwampHut,
    NotImplemented,
}

impl StructureType {
    pub fn parse(json: &Value) -> Result<StructureType, String> {
        let ty = json.get("type").and_then(Value::as_str).ok_or("structure without type")?;
        Ok(match ty.strip_prefix("minecraft:").unwrap_or(ty) {
            "buried_treasure" => StructureType::BuriedTreasure,
            "desert_pyramid" => StructureType::DesertPyramid,
            "igloo" => StructureType::Igloo,
            "jigsaw" => StructureType::Jigsaw(Box::new(jigsaw_structure::JigsawStructure::parse(json)?)),
            "jungle_temple" => StructureType::JungleTemple,
            "ocean_ruin" => StructureType::OceanRuin(OceanRuinStructure::parse(json)?),
            "shipwreck" => StructureType::Shipwreck { is_beached: json.get("is_beached").and_then(Value::as_bool).ok_or("missing is_beached")? },
            "stronghold" => StructureType::Stronghold,
            "swamp_hut" => StructureType::SwampHut,
            _ => StructureType::NotImplemented,
        })
    }

    pub fn find_generation_point(&self, context: &mut GenerationContext) -> Option<GenerationStub<'static>> {
        match self {
            StructureType::BuriedTreasure => buried_treasure_structure::find_generation_point(context),
            StructureType::DesertPyramid => {
                single_piece_structure::find_generation_point(context, |random, x, z| Piece::DesertPyramid(DesertPyramidPiece::new(random, x, z)), 21, 21)
            }
            StructureType::Igloo => igloo_structure::find_generation_point(context),
            StructureType::Jigsaw(structure) => structure.find_generation_point(context),
            StructureType::JungleTemple => {
                single_piece_structure::find_generation_point(context, |random, x, z| Piece::JungleTemple(JungleTemplePiece::new(random, x, z)), 12, 15)
            }
            StructureType::OceanRuin(structure) => structure.find_generation_point(context),
            StructureType::Shipwreck { is_beached } => shipwreck_structure::find_generation_point(context, *is_beached),
            StructureType::Stronghold => stronghold_structure::find_generation_point(context),
            StructureType::SwampHut => swamp_hut_structure::find_generation_point(context),
            StructureType::NotImplemented => None,
        }
    }

    pub fn after_place(&self, level: &mut WorldGenRegion, _random: &mut impl RandomSource, chunk_bb: &BoundingBox, _chunk_pos: ChunkPos, pieces: &PiecesContainer) {
        if let StructureType::DesertPyramid = self {
            desert_pyramid_structure::after_place(level, chunk_bb, pieces);
        }
    }
}

#[derive(Clone)]
pub enum Piece {
    BuriedTreasure(BuriedTreasurePiece),
    DesertPyramid(DesertPyramidPiece),
    Igloo(IglooPiece),
    Jigsaw(PoolElementStructurePiece),
    JungleTemple(JungleTemplePiece),
    OceanRuin(OceanRuinPiece),
    Shipwreck(ShipwreckPiece),
    Stronghold(StrongholdPieceKind),
    SwampHut(SwampHutPiece),
}

impl Piece {
    pub fn piece(&self) -> &StructurePiece {
        match self {
            Piece::BuriedTreasure(piece) => &piece.piece,
            Piece::DesertPyramid(piece) => &piece.scattered.piece,
            Piece::Igloo(piece) => &piece.template.piece,
            Piece::Jigsaw(piece) => &piece.piece,
            Piece::JungleTemple(piece) => &piece.scattered.piece,
            Piece::OceanRuin(piece) => &piece.template.piece,
            Piece::Shipwreck(piece) => &piece.template.piece,
            Piece::Stronghold(piece) => piece.piece(),
            Piece::SwampHut(piece) => &piece.scattered.piece,
        }
    }

    pub fn piece_mut(&mut self) -> &mut StructurePiece {
        match self {
            Piece::BuriedTreasure(piece) => &mut piece.piece,
            Piece::DesertPyramid(piece) => &mut piece.scattered.piece,
            Piece::Igloo(piece) => &mut piece.template.piece,
            Piece::Jigsaw(piece) => &mut piece.piece,
            Piece::JungleTemple(piece) => &mut piece.scattered.piece,
            Piece::OceanRuin(piece) => &mut piece.template.piece,
            Piece::Shipwreck(piece) => &mut piece.template.piece,
            Piece::Stronghold(piece) => piece.piece_mut(),
            Piece::SwampHut(piece) => &mut piece.scattered.piece,
        }
    }

    pub fn load(piece_id: &str, tag: &Nbt) -> Option<Piece> {
        Some(match piece_id {
            buried_treasure_pieces::BURIED_TREASURE_PIECE => Piece::BuriedTreasure(BuriedTreasurePiece::load(tag)?),
            desert_pyramid_piece::DESERT_PYRAMID_PIECE => Piece::DesertPyramid(DesertPyramidPiece::load(tag)?),
            igloo_pieces::IGLOO => Piece::Igloo(IglooPiece::load(tag)?),
            pool_element_structure_piece::JIGSAW => Piece::Jigsaw(PoolElementStructurePiece::load(tag)?),
            jungle_temple_piece::JUNGLE_PYRAMID_PIECE => Piece::JungleTemple(JungleTemplePiece::load(tag)?),
            ocean_ruin_pieces::OCEAN_RUIN => Piece::OceanRuin(OceanRuinPiece::load(tag)?),
            shipwreck_pieces::SHIPWRECK_PIECE => Piece::Shipwreck(ShipwreckPiece::load(tag)?),
            stronghold_pieces::STRONGHOLD_CHEST_CORRIDOR
            | stronghold_pieces::STRONGHOLD_FILLER_CORRIDOR
            | stronghold_pieces::STRONGHOLD_FIVE_CROSSING
            | stronghold_pieces::STRONGHOLD_LEFT_TURN
            | stronghold_pieces::STRONGHOLD_LIBRARY
            | stronghold_pieces::STRONGHOLD_PORTAL_ROOM
            | stronghold_pieces::STRONGHOLD_PRISON_HALL
            | stronghold_pieces::STRONGHOLD_RIGHT_TURN
            | stronghold_pieces::STRONGHOLD_ROOM_CROSSING
            | stronghold_pieces::STRONGHOLD_STAIRS_DOWN
            | stronghold_pieces::STRONGHOLD_START
            | stronghold_pieces::STRONGHOLD_STRAIGHT
            | stronghold_pieces::STRONGHOLD_STRAIGHT_STAIRS_DOWN => Piece::Stronghold(StrongholdPieceKind::load(piece_id, tag)?),
            swamp_hut_piece::SWAMPLAND_HUT => Piece::SwampHut(SwampHutPiece::load(tag)?),
            _ => return None,
        })
    }

    pub fn create_tag(&self) -> Nbt {
        let mut additional = Vec::new();
        match self {
            Piece::BuriedTreasure(_) => {}
            Piece::DesertPyramid(piece) => piece.add_additional_save_data(&mut additional),
            Piece::Igloo(piece) => piece.add_additional_save_data(&mut additional),
            Piece::Jigsaw(piece) => piece.add_additional_save_data(&mut additional),
            Piece::JungleTemple(piece) => piece.add_additional_save_data(&mut additional),
            Piece::OceanRuin(piece) => piece.add_additional_save_data(&mut additional),
            Piece::Shipwreck(piece) => piece.add_additional_save_data(&mut additional),
            Piece::Stronghold(piece) => piece.add_additional_save_data(&mut additional),
            Piece::SwampHut(piece) => piece.add_additional_save_data(&mut additional),
        }
        self.piece().create_tag(additional)
    }

    pub fn post_process(&mut self, level: &mut WorldGenRegion, random: &mut impl RandomSource, chunk_bb: &BoundingBox, chunk_pos: ChunkPos, reference_pos: BlockPos) {
        match self {
            Piece::BuriedTreasure(piece) => piece.post_process(level, random, chunk_bb),
            Piece::DesertPyramid(piece) => piece.post_process(level, random, chunk_bb),
            Piece::Igloo(piece) => piece.post_process(level, random, chunk_bb, reference_pos),
            Piece::Jigsaw(piece) => piece.post_process(level, random, chunk_bb, reference_pos),
            Piece::JungleTemple(piece) => piece.post_process(level, random, chunk_bb),
            Piece::OceanRuin(piece) => piece.post_process(level, random, chunk_bb, reference_pos),
            Piece::Shipwreck(piece) => piece.post_process(level, random, chunk_bb, reference_pos),
            Piece::Stronghold(piece) => piece.post_process(level, random, chunk_bb),
            Piece::SwampHut(piece) => piece.post_process(level, chunk_bb, chunk_pos, reference_pos),
        }
    }
}
