use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, Direction};
use crisp_data::blocks;
use crisp_protocol::nbt::Nbt;

use super::BoundingBox;
use super::structure_piece::StructurePiece;
use super::templatesystem::structure_place_settings::StructurePlaceSettings;
use super::templatesystem::structure_processor::StructureProcessor;
use super::templatesystem::structure_template::StructureTemplate;
use super::templatesystem::structure_template_manager;
use crate::world::level::world_gen_level::WorldGenLevel;
use crate::world::level::block::mirror::Mirror;
use crate::world::level::block::rotation::Rotation;

#[derive(Clone, Copy)]
pub struct TemplatePlaceSettings {
    pub mirror: Mirror,
    pub rotation: Rotation,
    pub rotation_pivot: BlockPos,
    pub apply_waterlogging: bool,
    pub processors: &'static [StructureProcessor],
}

impl TemplatePlaceSettings {
    pub fn settings(&self, bounding_box: Option<BoundingBox>) -> StructurePlaceSettings<'static> {
        StructurePlaceSettings {
            mirror: self.mirror,
            rotation: self.rotation,
            rotation_pivot: self.rotation_pivot,
            bounding_box,
            apply_waterlogging: self.apply_waterlogging,
            processors: self.processors.iter().collect(),
            ..StructurePlaceSettings::default()
        }
    }
}

#[derive(Clone)]
pub struct TemplateStructurePiece {
    pub piece: StructurePiece,
    pub template_name: String,
    pub template: &'static StructureTemplate,
    pub place_settings: TemplatePlaceSettings,
    pub template_position: BlockPos,
}

fn int(tag: &Nbt, key: &str) -> i32 {
    match tag.get(key) {
        Some(Nbt::Int(v)) => *v,
        _ => 0,
    }
}

impl TemplateStructurePiece {
    pub fn new(piece_type: &'static str, gen_depth: i32, template_location: &str, template_name: String, place_settings: TemplatePlaceSettings, position: BlockPos) -> TemplateStructurePiece {
        let template = structure_template_manager::get_or_create(template_location);
        let mut piece = StructurePiece::new(piece_type, gen_depth, template.get_bounding_box(&place_settings.settings(None), position));
        piece.set_orientation(Some(Direction::North));
        TemplateStructurePiece { piece, template_name, template, place_settings, template_position: position }
    }

    pub fn load(piece_type: &'static str, tag: &Nbt, place_settings: impl FnOnce(&str) -> TemplatePlaceSettings) -> Option<TemplateStructurePiece> {
        let mut piece = StructurePiece::load(piece_type, tag)?;
        piece.set_orientation(Some(Direction::North));
        let template_name = tag.get("Template").and_then(Nbt::as_str).unwrap_or("").to_owned();
        let template_position = BlockPos::new(int(tag, "TPX"), int(tag, "TPY"), int(tag, "TPZ"));
        let template_location = crate::world::level::levelgen::blockpredicates::block_predicate::resource(&template_name);
        let template = structure_template_manager::get_or_create(&template_location);
        let place_settings = place_settings(&template_location);
        piece.bounding_box = template.get_bounding_box(&place_settings.settings(None), template_position);
        Some(TemplateStructurePiece { piece, template_name, template, place_settings, template_position })
    }

    pub fn load_with_template_location(
        piece_type: &'static str,
        tag: &Nbt,
        make_template_location: impl FnOnce(&str) -> String,
        place_settings: impl FnOnce(&str) -> TemplatePlaceSettings,
    ) -> Option<TemplateStructurePiece> {
        let mut piece = StructurePiece::load(piece_type, tag)?;
        piece.set_orientation(Some(Direction::North));
        let template_name = tag.get("Template").and_then(Nbt::as_str).unwrap_or("").to_owned();
        let template_position = BlockPos::new(int(tag, "TPX"), int(tag, "TPY"), int(tag, "TPZ"));
        let template_location = make_template_location(&template_name);
        let template = structure_template_manager::get_or_create(&template_location);
        let place_settings = place_settings(&template_location);
        piece.bounding_box = template.get_bounding_box(&place_settings.settings(None), template_position);
        Some(TemplateStructurePiece { piece, template_name, template, place_settings, template_position })
    }

    pub fn add_additional_save_data(&self, tag: &mut Vec<(String, Nbt)>) {
        tag.push(("TPX".into(), Nbt::Int(self.template_position.x)));
        tag.push(("TPY".into(), Nbt::Int(self.template_position.y)));
        tag.push(("TPZ".into(), Nbt::Int(self.template_position.z)));
        tag.push(("Template".into(), Nbt::String(self.template_name.clone())));
    }

    pub fn post_process<L: WorldGenLevel, R: RandomSource>(
        &mut self,
        level: &mut L,
        random: &mut R,
        chunk_bb: &BoundingBox,
        reference_pos: BlockPos,
        mut handle_data_marker: impl FnMut(&str, BlockPos, &mut L, &mut R, &BoundingBox),
    ) {
        let settings = self.place_settings.settings(Some(*chunk_bb));
        self.piece.bounding_box = self.template.get_bounding_box(&settings, self.template_position);
        if !self.template.place_in_world(level, self.template_position, reference_pos, &settings, random, 2) {
            return;
        }
        for data_marker in self.template.filter_blocks(self.template_position, &settings, blocks::STRUCTURE_BLOCK, random) {
            if let Some(nbt) = &data_marker.nbt
                && nbt.get("mode").and_then(Nbt::as_str) == Some("DATA")
            {
                let marker_id = nbt.get("metadata").and_then(Nbt::as_str).unwrap_or("");
                handle_data_marker(marker_id, data_marker.pos, level, random, chunk_bb);
            }
        }
        for jigsaw_block in self.template.filter_blocks(self.template_position, &settings, blocks::JIGSAW, random) {
            if let Some(nbt) = &jigsaw_block.nbt {
                let state_string = nbt.get("final_state").and_then(Nbt::as_str).unwrap_or("minecraft:air");
                let utf16: Vec<u16> = state_string.encode_utf16().collect();
                let mut reader = crate::commands::brigadier::StringReader::new(&utf16);
                let target_state = match crate::commands::arguments::blocks::block_state_parser::parse_for_block(&mut reader, true) {
                    Ok(result) => result.state,
                    Err(_) => {
                        error!("Error while parsing blockstate {state_string} in jigsaw block @ {:?}", jigsaw_block.pos);
                        blocks::AIR.default_state()
                    }
                };
                level.set_block_and_update(jigsaw_block.pos, target_state);
            }
        }
    }

    pub fn move_by(&mut self, dx: i32, dy: i32, dz: i32) {
        self.piece.move_by(dx, dy, dz);
        self.template_position = self.template_position.offset(dx, dy, dz);
    }
}
