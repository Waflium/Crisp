use std::sync::LazyLock;

use crisp_core::random::RandomSource;
use crisp_core::BlockPos;
use crisp_data::blocks;
use crisp_protocol::nbt::Nbt;

use super::Piece;
use crate::world::level::world_gen_level::WorldGenLevel;
use crate::world::level::block::entity::BlockEntityKind;
use crate::world::level::block::mirror::Mirror;
use crate::world::level::block::rotation::Rotation;
use crate::world::level::chunk::HeightmapType;
use crate::world::level::levelgen::structure::BoundingBox;
use crate::world::level::levelgen::structure::pieces::structure_pieces_builder::StructurePiecesBuilder;
use crate::world::level::levelgen::structure::template_structure_piece::{TemplatePlaceSettings, TemplateStructurePiece};
use crate::world::level::levelgen::structure::templatesystem::structure_processor::StructureProcessor;
use crate::world::level::levelgen::structure::templatesystem::structure_template::StructureTemplate;

pub const IGLOO: &str = "minecraft:iglu";
pub const GENERATION_HEIGHT: i32 = 90;
const STRUCTURE_LOCATION_IGLOO: &str = "minecraft:igloo/top";
const STRUCTURE_LOCATION_LADDER: &str = "minecraft:igloo/middle";
const STRUCTURE_LOCATION_LABORATORY: &str = "minecraft:igloo/bottom";
const IGLOO_CHEST: &str = "minecraft:chests/igloo_chest";

static STRUCTURE_BLOCK: LazyLock<[StructureProcessor; 1]> = LazyLock::new(|| [StructureProcessor::BlockIgnore(vec![blocks::STRUCTURE_BLOCK])]);

fn pivot(template_location: &str) -> BlockPos {
    match template_location {
        STRUCTURE_LOCATION_IGLOO => BlockPos::new(3, 5, 5),
        STRUCTURE_LOCATION_LADDER => BlockPos::new(1, 3, 1),
        _ => BlockPos::new(3, 6, 7),
    }
}

fn offset(template_location: &str) -> BlockPos {
    match template_location {
        STRUCTURE_LOCATION_IGLOO => BlockPos::ZERO,
        STRUCTURE_LOCATION_LADDER => BlockPos::new(2, -3, 4),
        _ => BlockPos::new(0, -3, -2),
    }
}

pub fn add_pieces(position: BlockPos, rotation: Rotation, builder: &mut StructurePiecesBuilder, random: &mut impl RandomSource) {
    if random.next_double() < 0.5 {
        let depth = random.next_int_bounded(8) + 4;
        builder.add_piece(Piece::Igloo(IglooPiece::new(STRUCTURE_LOCATION_LABORATORY, position, rotation, depth * 3)));
        for i in 0..depth - 1 {
            builder.add_piece(Piece::Igloo(IglooPiece::new(STRUCTURE_LOCATION_LADDER, position, rotation, i * 3)));
        }
    }
    builder.add_piece(Piece::Igloo(IglooPiece::new(STRUCTURE_LOCATION_IGLOO, position, rotation, 0)));
}

#[derive(Clone)]
pub struct IglooPiece {
    pub template: TemplateStructurePiece,
}

fn make_settings(rotation: Rotation, template_location: &str) -> TemplatePlaceSettings {
    TemplatePlaceSettings { mirror: Mirror::None, rotation, rotation_pivot: pivot(template_location), apply_waterlogging: false, processors: &*STRUCTURE_BLOCK }
}

fn make_position(template_location: &str, position: BlockPos, depth: i32) -> BlockPos {
    let offset = offset(template_location);
    position.offset(offset.x, offset.y - depth, offset.z)
}

impl IglooPiece {
    pub fn new(template_location: &'static str, position: BlockPos, rotation: Rotation, depth: i32) -> IglooPiece {
        IglooPiece {
            template: TemplateStructurePiece::new(
                IGLOO,
                0,
                template_location,
                template_location.to_owned(),
                make_settings(rotation, template_location),
                make_position(template_location, position, depth),
            ),
        }
    }

    pub fn load(tag: &Nbt) -> Option<IglooPiece> {
        let rotation = tag.get("Rot").and_then(Nbt::as_str).and_then(Rotation::by_legacy_name)?;
        Some(IglooPiece { template: TemplateStructurePiece::load(IGLOO, tag, |location| make_settings(rotation, location))? })
    }

    pub fn add_additional_save_data(&self, tag: &mut Vec<(String, Nbt)>) {
        self.template.add_additional_save_data(tag);
        tag.push(("Rot".into(), Nbt::String(self.template.place_settings.rotation.legacy_name().to_owned())));
    }

    pub fn post_process(&mut self, level: &mut impl WorldGenLevel, random: &mut impl RandomSource, chunk_bb: &BoundingBox, reference_pos: BlockPos) {
        let template_location = crate::world::level::levelgen::blockpredicates::block_predicate::resource(&self.template.template_name);
        let settings = make_settings(self.template.place_settings.rotation, &template_location).settings(None);
        let offset = offset(&template_location);
        let relative = StructureTemplate::calculate_relative_position(&settings, BlockPos::new(3 - offset.x, 0, -offset.z));
        let entrance_pos = self.template.template_position.offset(relative.x, relative.y, relative.z);
        let height = level.get_height(HeightmapType::WorldSurfaceWg, entrance_pos.x, entrance_pos.z);
        let old_template_pos = self.template.template_position;
        self.template.template_position = self.template.template_position.offset(0, height - GENERATION_HEIGHT - 1, 0);
        self.template.post_process(level, random, chunk_bb, reference_pos, |marker_id, position, level, random, _chunk_bb| {
            if marker_id == "chest" {
                level.set_block_and_update(position, blocks::AIR.default_state());
                if let Some(entity) = level.get_block_entity(position.below())
                    && matches!(entity.kind, BlockEntityKind::Chest(_))
                    && let Some(base) = entity.container_base_mut()
                {
                    base.loot_table = Some(IGLOO_CHEST.to_owned());
                    base.loot_table_seed = random.next_long();
                }
            }
        });
        if template_location == STRUCTURE_LOCATION_IGLOO {
            let relative = StructureTemplate::calculate_relative_position(&settings, BlockPos::new(3, 0, 5));
            let trap_door_pos = self.template.template_position.offset(relative.x, relative.y, relative.z);
            let below_state = level.get_block_state(trap_door_pos.below());
            if !below_state.is_air() && !below_state.is_block(blocks::LADDER) {
                level.set_block_and_update(trap_door_pos, blocks::SNOW_BLOCK.default_state());
            }
        }
        self.template.template_position = old_template_pos;
    }
}
