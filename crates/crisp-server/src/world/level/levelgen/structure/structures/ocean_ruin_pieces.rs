use std::sync::LazyLock;

use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, mth};
use crisp_data::properties::WATERLOGGED;
use crisp_data::{Block, block_tags, blocks, fluid_tags};
use crisp_protocol::nbt::Nbt;

use super::Piece;
use super::ocean_ruin_structure::{OceanRuinStructure, Type};
use crate::world::level::world_gen_level::WorldGenLevel;
use crate::util::valueproviders::IntProvider;
use crate::world::entity::entity_types;
use crate::world::level::block::entity::BlockEntityKind;
use crate::world::level::block::mirror::Mirror;
use crate::world::level::block::rotation::Rotation;
use crate::world::level::chunk::HeightmapType;
use crate::world::level::levelgen::structure::BoundingBox;
use crate::world::level::levelgen::structure::pieces::structure_pieces_builder::StructurePiecesBuilder;
use crate::world::level::levelgen::structure::template_structure_piece::{TemplatePlaceSettings, TemplateStructurePiece};
use crate::world::level::levelgen::structure::templatesystem::pos_rule_test::PosRuleTest;
use crate::world::level::levelgen::structure::templatesystem::rule_test::RuleTest;
use crate::world::level::levelgen::structure::templatesystem::structure_processor::{ProcessorRule, RuleBlockEntityModifier, StructureProcessor};
use crate::world::level::levelgen::structure::templatesystem::structure_template::transform;

pub const OCEAN_RUIN: &str = "minecraft:orp";
const OCEAN_RUIN_WARM_ARCHAEOLOGY: &str = "minecraft:archaeology/ocean_ruin_warm";
const OCEAN_RUIN_COLD_ARCHAEOLOGY: &str = "minecraft:archaeology/ocean_ruin_cold";
const UNDERWATER_RUIN_BIG: &str = "minecraft:chests/underwater_ruin_big";
const UNDERWATER_RUIN_SMALL: &str = "minecraft:chests/underwater_ruin_small";

const WARM_RUINS: [&str; 8] = [
    "minecraft:underwater_ruin/warm_1",
    "minecraft:underwater_ruin/warm_2",
    "minecraft:underwater_ruin/warm_3",
    "minecraft:underwater_ruin/warm_4",
    "minecraft:underwater_ruin/warm_5",
    "minecraft:underwater_ruin/warm_6",
    "minecraft:underwater_ruin/warm_7",
    "minecraft:underwater_ruin/warm_8",
];
const RUINS_BRICK: [&str; 8] = [
    "minecraft:underwater_ruin/brick_1",
    "minecraft:underwater_ruin/brick_2",
    "minecraft:underwater_ruin/brick_3",
    "minecraft:underwater_ruin/brick_4",
    "minecraft:underwater_ruin/brick_5",
    "minecraft:underwater_ruin/brick_6",
    "minecraft:underwater_ruin/brick_7",
    "minecraft:underwater_ruin/brick_8",
];
const RUINS_CRACKED: [&str; 8] = [
    "minecraft:underwater_ruin/cracked_1",
    "minecraft:underwater_ruin/cracked_2",
    "minecraft:underwater_ruin/cracked_3",
    "minecraft:underwater_ruin/cracked_4",
    "minecraft:underwater_ruin/cracked_5",
    "minecraft:underwater_ruin/cracked_6",
    "minecraft:underwater_ruin/cracked_7",
    "minecraft:underwater_ruin/cracked_8",
];
const RUINS_MOSSY: [&str; 8] = [
    "minecraft:underwater_ruin/mossy_1",
    "minecraft:underwater_ruin/mossy_2",
    "minecraft:underwater_ruin/mossy_3",
    "minecraft:underwater_ruin/mossy_4",
    "minecraft:underwater_ruin/mossy_5",
    "minecraft:underwater_ruin/mossy_6",
    "minecraft:underwater_ruin/mossy_7",
    "minecraft:underwater_ruin/mossy_8",
];
const BIG_RUINS_BRICK: [&str; 4] =
    ["minecraft:underwater_ruin/big_brick_1", "minecraft:underwater_ruin/big_brick_2", "minecraft:underwater_ruin/big_brick_3", "minecraft:underwater_ruin/big_brick_8"];
const BIG_RUINS_MOSSY: [&str; 4] =
    ["minecraft:underwater_ruin/big_mossy_1", "minecraft:underwater_ruin/big_mossy_2", "minecraft:underwater_ruin/big_mossy_3", "minecraft:underwater_ruin/big_mossy_8"];
const BIG_RUINS_CRACKED: [&str; 4] =
    ["minecraft:underwater_ruin/big_cracked_1", "minecraft:underwater_ruin/big_cracked_2", "minecraft:underwater_ruin/big_cracked_3", "minecraft:underwater_ruin/big_cracked_8"];
const BIG_WARM_RUINS: [&str; 4] =
    ["minecraft:underwater_ruin/big_warm_4", "minecraft:underwater_ruin/big_warm_5", "minecraft:underwater_ruin/big_warm_6", "minecraft:underwater_ruin/big_warm_7"];

const INTEGRITIES: [f32; 4] = [0.9, 0.8, 0.7, 0.5];

static SETTINGS_PROCESSORS: LazyLock<[[[StructureProcessor; 3]; 4]; 2]> =
    LazyLock::new(|| [Type::Warm, Type::Cold].map(|biome_type| INTEGRITIES.map(|integrity| processors(integrity, biome_type))));

fn archy_rule_processor(candidate_block: Block, replacement_block: Block, loot_table: &str) -> StructureProcessor {
    StructureProcessor::Capped {
        delegate: Box::new(StructureProcessor::Rule(vec![ProcessorRule::new(
            RuleTest::BlockMatch(candidate_block),
            RuleTest::AlwaysTrue,
            PosRuleTest::AlwaysTrue,
            replacement_block.default_state(),
            RuleBlockEntityModifier::AppendLoot(loot_table.to_owned()),
        )])),
        limit: IntProvider::Constant(5),
    }
}

fn get_small_warm_ruin(random: &mut impl RandomSource) -> &'static str {
    WARM_RUINS[random.next_int_bounded(WARM_RUINS.len() as i32) as usize]
}

fn get_big_warm_ruin(random: &mut impl RandomSource) -> &'static str {
    BIG_WARM_RUINS[random.next_int_bounded(BIG_WARM_RUINS.len() as i32) as usize]
}

pub fn add_pieces(position: BlockPos, rotation: Rotation, builder: &mut StructurePiecesBuilder, random: &mut impl RandomSource, structure: &OceanRuinStructure) {
    let is_large = random.next_float() <= structure.large_probability;
    let base_integrity = if is_large { 0.9 } else { 0.8 };
    add_piece(position, rotation, builder, random, structure, is_large, base_integrity);
    if is_large && random.next_float() <= structure.cluster_probability {
        add_cluster_ruins(random, rotation, position, structure, builder);
    }
}

fn add_cluster_ruins(random: &mut impl RandomSource, rotation: Rotation, p: BlockPos, structure: &OceanRuinStructure, builder: &mut StructurePiecesBuilder) {
    let parent_pos = BlockPos::new(p.x, 90, p.z);
    let parent_corner = transform(BlockPos::new(15, 0, 15), Mirror::None, rotation, BlockPos::ZERO).offset(parent_pos.x, parent_pos.y, parent_pos.z);
    let parent_bb = BoundingBox::from_corners(parent_pos, parent_corner);
    let parent_bottom_left = BlockPos::new(parent_pos.x.min(parent_corner.x), parent_pos.y, parent_pos.z.min(parent_corner.z));
    let mut all_positions = all_positions(random, parent_bottom_left);
    let ruins = mth::next_int(random, 4, 8);
    for _ in 0..ruins {
        if !all_positions.is_empty() {
            let idx = random.next_int_bounded(all_positions.len() as i32) as usize;
            let pos = all_positions.remove(idx);
            let next_rotation = Rotation::get_random(random);
            let next_corner = transform(BlockPos::new(5, 0, 6), Mirror::None, next_rotation, BlockPos::ZERO).offset(pos.x, pos.y, pos.z);
            let next_bb = BoundingBox::from_corners(pos, next_corner);
            if !next_bb.intersects(&parent_bb) {
                add_piece(pos, next_rotation, builder, random, structure, false, 0.8);
            }
        }
    }
}

fn all_positions(random: &mut impl RandomSource, origin: BlockPos) -> Vec<BlockPos> {
    vec![
        origin.offset(-16 + mth::next_int(random, 1, 8), 0, 16 + mth::next_int(random, 1, 7)),
        origin.offset(-16 + mth::next_int(random, 1, 8), 0, mth::next_int(random, 1, 7)),
        origin.offset(-16 + mth::next_int(random, 1, 8), 0, -16 + mth::next_int(random, 4, 8)),
        origin.offset(mth::next_int(random, 1, 7), 0, 16 + mth::next_int(random, 1, 7)),
        origin.offset(mth::next_int(random, 1, 7), 0, -16 + mth::next_int(random, 4, 6)),
        origin.offset(16 + mth::next_int(random, 1, 7), 0, 16 + mth::next_int(random, 3, 8)),
        origin.offset(16 + mth::next_int(random, 1, 7), 0, mth::next_int(random, 1, 7)),
        origin.offset(16 + mth::next_int(random, 1, 7), 0, -16 + mth::next_int(random, 4, 8)),
    ]
}

#[allow(clippy::too_many_arguments)]
fn add_piece(
    position: BlockPos,
    rotation: Rotation,
    builder: &mut StructurePiecesBuilder,
    random: &mut impl RandomSource,
    structure: &OceanRuinStructure,
    is_large: bool,
    base_integrity: f32,
) {
    match structure.biome_temp {
        Type::Warm => {
            let start_piece_location = if is_large { get_big_warm_ruin(random) } else { get_small_warm_ruin(random) };
            builder.add_piece(Piece::OceanRuin(OceanRuinPiece::new(start_piece_location, position, rotation, base_integrity, structure.biome_temp, is_large)));
        }
        Type::Cold => {
            let bricks: &[&'static str] = if is_large { &BIG_RUINS_BRICK } else { &RUINS_BRICK };
            let cracked: &[&'static str] = if is_large { &BIG_RUINS_CRACKED } else { &RUINS_CRACKED };
            let mossy: &[&'static str] = if is_large { &BIG_RUINS_MOSSY } else { &RUINS_MOSSY };
            let idx = random.next_int_bounded(bricks.len() as i32) as usize;
            builder.add_piece(Piece::OceanRuin(OceanRuinPiece::new(bricks[idx], position, rotation, base_integrity, structure.biome_temp, is_large)));
            builder.add_piece(Piece::OceanRuin(OceanRuinPiece::new(cracked[idx], position, rotation, 0.7, structure.biome_temp, is_large)));
            builder.add_piece(Piece::OceanRuin(OceanRuinPiece::new(mossy[idx], position, rotation, 0.5, structure.biome_temp, is_large)));
        }
    }
}

#[derive(Clone)]
pub struct OceanRuinPiece {
    pub template: TemplateStructurePiece,
    biome_type: Type,
    integrity: f32,
    is_large: bool,
}

fn processors(integrity: f32, biome_type: Type) -> [StructureProcessor; 3] {
    let suspicious_block_processor = match biome_type {
        Type::Cold => archy_rule_processor(blocks::GRAVEL, blocks::SUSPICIOUS_GRAVEL, OCEAN_RUIN_COLD_ARCHAEOLOGY),
        Type::Warm => archy_rule_processor(blocks::SAND, blocks::SUSPICIOUS_SAND, OCEAN_RUIN_WARM_ARCHAEOLOGY),
    };
    [
        StructureProcessor::BlockRot { rottable_blocks: None, integrity },
        StructureProcessor::BlockIgnore(vec![blocks::STRUCTURE_BLOCK, blocks::AIR]),
        suspicious_block_processor,
    ]
}

fn make_settings(rotation: Rotation, integrity: f32, biome_type: Type) -> TemplatePlaceSettings {
    let processors: &'static [StructureProcessor] = match INTEGRITIES.iter().position(|&i| i == integrity) {
        Some(index) => &SETTINGS_PROCESSORS[biome_type as usize][index],
        None => Box::leak(Box::new(processors(integrity, biome_type))),
    };
    TemplatePlaceSettings { mirror: Mirror::None, rotation, rotation_pivot: BlockPos::ZERO, apply_waterlogging: true, processors }
}

impl OceanRuinPiece {
    pub fn new(template_location: &'static str, position: BlockPos, rotation: Rotation, integrity: f32, biome_type: Type, is_large: bool) -> OceanRuinPiece {
        OceanRuinPiece {
            template: TemplateStructurePiece::new(
                OCEAN_RUIN,
                0,
                template_location,
                template_location.to_owned(),
                make_settings(rotation, integrity, biome_type),
                position,
            ),
            biome_type,
            integrity,
            is_large,
        }
    }

    pub fn load(tag: &Nbt) -> Option<OceanRuinPiece> {
        let rotation = tag.get("Rot").and_then(Nbt::as_str).and_then(Rotation::by_legacy_name)?;
        let integrity = match tag.get("Integrity") {
            Some(Nbt::Float(v)) => *v,
            _ => 0.0,
        };
        let biome_type = tag.get("BiomeType").and_then(Nbt::as_str).and_then(Type::by_legacy_name)?;
        let is_large = matches!(tag.get("IsLarge"), Some(Nbt::Byte(1..)));
        Some(OceanRuinPiece {
            template: TemplateStructurePiece::load(OCEAN_RUIN, tag, |_| make_settings(rotation, integrity, biome_type))?,
            biome_type,
            integrity,
            is_large,
        })
    }

    pub fn add_additional_save_data(&self, tag: &mut Vec<(String, Nbt)>) {
        self.template.add_additional_save_data(tag);
        tag.push(("Rot".into(), Nbt::String(self.template.place_settings.rotation.legacy_name().to_owned())));
        tag.push(("Integrity".into(), Nbt::Float(self.integrity)));
        tag.push(("BiomeType".into(), Nbt::String(self.biome_type.legacy_name().to_owned())));
        tag.push(("IsLarge".into(), Nbt::Byte(self.is_large as i8)));
    }

    fn handle_data_marker(is_large: bool, marker_id: &str, position: BlockPos, level: &mut impl WorldGenLevel, random: &mut impl RandomSource) {
        if marker_id == "chest" {
            let waterlogged = level.get_fluid_state(position).is(&fluid_tags::WATER);
            level.set_block(position, blocks::CHEST.default_state().set(WATERLOGGED, waterlogged), 2);
            if let Some(entity) = level.get_block_entity(position)
                && matches!(entity.kind, BlockEntityKind::Chest(_))
                && let Some(base) = entity.container_base_mut()
            {
                base.loot_table = Some(if is_large { UNDERWATER_RUIN_BIG } else { UNDERWATER_RUIN_SMALL }.to_owned());
                base.loot_table_seed = random.next_long();
            }
        } else if marker_id == "drowned" && entity_types::DROWNED.can_spawn(level.get_difficulty()) {
            if position.y > level.get_sea_level() {
                level.set_block(position, blocks::AIR.default_state(), 2);
            } else {
                level.set_block(position, blocks::WATER.default_state(), 2);
            }
        }
    }

    pub fn post_process(&mut self, level: &mut impl WorldGenLevel, random: &mut impl RandomSource, chunk_bb: &BoundingBox, reference_pos: BlockPos) {
        let template_position = self.template.template_position;
        let height = level.get_height(HeightmapType::OceanFloorWg, template_position.x, template_position.z);
        self.template.template_position = BlockPos::new(template_position.x, height, template_position.z);
        let size = self.template.template.get_size();
        let template_position = self.template.template_position;
        let corner = transform(BlockPos::new(size.x - 1, 0, size.z - 1), Mirror::None, self.template.place_settings.rotation, BlockPos::ZERO).offset(
            template_position.x,
            template_position.y,
            template_position.z,
        );
        self.template.template_position = BlockPos::new(template_position.x, get_height(template_position, level, corner), template_position.z);
        let is_large = self.is_large;
        self.template.post_process(level, random, chunk_bb, reference_pos, |marker_id, position, level, random, _chunk_bb| {
            OceanRuinPiece::handle_data_marker(is_large, marker_id, position, level, random)
        });
    }
}

fn get_height(pos: BlockPos, level: &mut impl WorldGenLevel, corner: BlockPos) -> i32 {
    let mut new_y = pos.y;
    let mut min_y = 512;
    let top_y = new_y - 1;
    let mut area = 0;
    for z in pos.z.min(corner.z)..=pos.z.max(corner.z) {
        for x in pos.x.min(corner.x)..=pos.x.max(corner.x) {
            let mut floor_y = pos.y - 1;
            let mut temp_pos = BlockPos::new(x, floor_y, z);
            let mut temp_state = level.get_block_state(temp_pos);
            let mut temp_fluid = level.get_fluid_state(temp_pos);
            while (temp_state.is_air() || temp_fluid.is(&fluid_tags::WATER) || temp_state.is(&block_tags::ICE)) && floor_y > level.get_min_y() + 1 {
                floor_y -= 1;
                temp_pos = BlockPos::new(x, floor_y, z);
                temp_state = level.get_block_state(temp_pos);
                temp_fluid = level.get_fluid_state(temp_pos);
            }
            min_y = min_y.min(floor_y);
            if floor_y < top_y - 2 {
                area += 1;
            }
        }
    }
    let width = (pos.x - corner.x).abs();
    if top_y - min_y > 2 && area > width - 2 {
        new_y = min_y + 1;
    }
    new_y
}
