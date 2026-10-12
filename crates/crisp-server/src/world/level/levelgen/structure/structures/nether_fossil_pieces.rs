use std::sync::LazyLock;

use crisp_core::BlockPos;
use crisp_core::random::{LegacyRandomSource, RandomSource};
use crisp_data::blocks;
use crisp_protocol::nbt::Nbt;

use super::Piece;
use crate::world::level::block;
use crate::world::level::block::mirror::Mirror;
use crate::world::level::block::rotation::Rotation;
use crate::world::level::levelgen::structure::BoundingBox;
use crate::world::level::levelgen::structure::pieces::structure_pieces_builder::StructurePiecesBuilder;
use crate::world::level::levelgen::structure::template_structure_piece::{TemplatePlaceSettings, TemplateStructurePiece};
use crate::world::level::levelgen::structure::templatesystem::structure_processor::StructureProcessor;
use crate::world::level::world_gen_level::WorldGenLevel;

pub const NETHER_FOSSIL: &str = "minecraft:nefos";
const FOSSILS: [&str; 14] = [
    "minecraft:nether_fossils/fossil_1",
    "minecraft:nether_fossils/fossil_2",
    "minecraft:nether_fossils/fossil_3",
    "minecraft:nether_fossils/fossil_4",
    "minecraft:nether_fossils/fossil_5",
    "minecraft:nether_fossils/fossil_6",
    "minecraft:nether_fossils/fossil_7",
    "minecraft:nether_fossils/fossil_8",
    "minecraft:nether_fossils/fossil_9",
    "minecraft:nether_fossils/fossil_10",
    "minecraft:nether_fossils/fossil_11",
    "minecraft:nether_fossils/fossil_12",
    "minecraft:nether_fossils/fossil_13",
    "minecraft:nether_fossils/fossil_14",
];

static STRUCTURE_AND_AIR: LazyLock<[StructureProcessor; 1]> = LazyLock::new(|| [StructureProcessor::BlockIgnore(vec![blocks::AIR, blocks::STRUCTURE_BLOCK])]);

pub fn add_pieces(builder: &mut StructurePiecesBuilder, random: &mut impl RandomSource, position: BlockPos) {
    let next_rotation = Rotation::get_random(random);
    let template_location = FOSSILS[random.next_int_bounded(FOSSILS.len() as i32) as usize];
    builder.add_piece(Piece::NetherFossil(NetherFossilPiece::new(template_location, position, next_rotation)));
}

#[derive(Clone)]
pub struct NetherFossilPiece {
    pub template: TemplateStructurePiece,
}

fn make_settings(rotation: Rotation) -> TemplatePlaceSettings {
    TemplatePlaceSettings { mirror: Mirror::None, rotation, rotation_pivot: BlockPos::ZERO, apply_waterlogging: true, processors: &*STRUCTURE_AND_AIR }
}

impl NetherFossilPiece {
    pub fn new(template_location: &'static str, position: BlockPos, rotation: Rotation) -> NetherFossilPiece {
        NetherFossilPiece { template: TemplateStructurePiece::new(NETHER_FOSSIL, 0, template_location, template_location.to_owned(), make_settings(rotation), position) }
    }

    pub fn load(tag: &Nbt) -> Option<NetherFossilPiece> {
        let rotation = tag.get("Rot").and_then(Nbt::as_str).and_then(Rotation::by_legacy_name)?;
        Some(NetherFossilPiece { template: TemplateStructurePiece::load(NETHER_FOSSIL, tag, |_| make_settings(rotation))? })
    }

    pub fn add_additional_save_data(&self, tag: &mut Vec<(String, Nbt)>) {
        self.template.add_additional_save_data(tag);
        tag.push(("Rot".into(), Nbt::String(self.template.place_settings.rotation.legacy_name().to_owned())));
    }

    pub fn post_process(&mut self, level: &mut impl WorldGenLevel, random: &mut impl RandomSource, chunk_bb: &BoundingBox, reference_pos: BlockPos) {
        let fossil_bb = self.template.template.get_bounding_box(&self.template.place_settings.settings(None), self.template.template_position);
        let chunk_bb = BoundingBox::encapsulating(chunk_bb, &fossil_bb);
        self.template.post_process(level, random, &chunk_bb, reference_pos, |_, _, _, _, _| {});
        self.place_dried_ghast(level, &fossil_bb, &chunk_bb);
    }

    fn place_dried_ghast(&self, level: &mut impl WorldGenLevel, fossil_bb: &BoundingBox, chunk_bb: &BoundingBox) {
        let center = fossil_bb.get_center();
        let mut positional_random = LegacyRandomSource::new(level.get_seed()).fork_positional().at(center.x, center.y, center.z);
        if positional_random.next_float() < 0.5 {
            let x = fossil_bb.min.x + positional_random.next_int_bounded(fossil_bb.x_span());
            let y = fossil_bb.min.y;
            let z = fossil_bb.min.z + positional_random.next_int_bounded(fossil_bb.z_span());
            let random_pos = BlockPos::new(x, y, z);
            if level.get_block_state(random_pos).is_air() && chunk_bb.is_inside(random_pos) {
                level.set_block(random_pos, block::rotate(blocks::DRIED_GHAST.default_state(), Rotation::get_random(&mut positional_random)), 2);
            }
        }
    }
}
