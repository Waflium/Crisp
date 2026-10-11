use crisp_core::random::RandomSource;
use crisp_core::{Axis, BlockPos};
use serde_json::Value;

use crate::server::level::world_gen_region::WorldGenRegion;
use crate::util::random::WeightedList;
use crate::world::level::block::rotation::Rotation;
use crate::world::level::levelgen::blockpredicates::block_predicate::resource;
use crate::world::level::levelgen::structure::templatesystem::structure_place_settings::StructurePlaceSettings;
use crate::world::level::levelgen::structure::templatesystem::structure_processor::StructureProcessorListHolder;
use crate::world::level::levelgen::structure::templatesystem::structure_template::StructureTemplate;
use crate::world::level::levelgen::structure::templatesystem::structure_template_manager;

pub struct TemplateFeature {
    templates: WeightedList<TemplateEntry>,
    processors: Option<StructureProcessorListHolder>,
}

pub struct TemplateEntry {
    template: String,
    rotations: Vec<Rotation>,
}

impl TemplateEntry {
    fn parse(json: &Value) -> Result<TemplateEntry, String> {
        let template = resource(json.get("id").and_then(Value::as_str).ok_or("template entry without id")?);
        let rotations = match json.get("rotations").and_then(Value::as_array) {
            Some(rotations) => rotations
                .iter()
                .map(|rotation| rotation.as_str().and_then(Rotation::by_name).ok_or_else(|| format!("unknown rotation {rotation}")))
                .collect::<Result<_, _>>()?,
            None => Rotation::VALUES.to_vec(),
        };
        Ok(TemplateEntry { template, rotations })
    }
}

impl TemplateFeature {
    pub fn parse(json: &Value) -> Result<TemplateFeature, String> {
        Ok(TemplateFeature {
            templates: WeightedList::parse(json.get("templates").ok_or("missing templates")?, TemplateEntry::parse)?,
            processors: json.get("processors").map(StructureProcessorListHolder::parse).transpose()?,
        })
    }

    pub fn place<R: RandomSource>(&self, level: &mut WorldGenRegion, random: &mut R, origin: BlockPos) -> bool {
        let template_entry = self.templates.get_random_or_throw(random);
        let rotation = template_entry.rotations[random.next_int_bounded(template_entry.rotations.len() as i32) as usize];
        let template = structure_template_manager::get_or_create(&template_entry.template);
        let offset_x = self.get_rotated_offset(rotation, Axis::X, template);
        let offset_z = self.get_rotated_offset(rotation, Axis::Z, template);
        let pos = origin.offset(offset_x.x, offset_x.y, offset_x.z).offset(offset_z.x, offset_z.y, offset_z.z);
        let settings = StructurePlaceSettings {
            rotation,
            random: true,
            processors: self.processors.as_ref().map_or(&[], |processors| &processors.value().list),
            ..StructurePlaceSettings::default()
        };
        template.place_in_world(level, pos, pos, &settings, random, 3)
    }

    fn get_rotated_offset(&self, rotation: Rotation, axis: Axis, template: &StructureTemplate) -> BlockPos {
        let direction = rotation.rotate(axis.negative());
        let size = template.get_size().get(axis) / 2;
        BlockPos::new(direction.step_x() * size, direction.step_y() * size, direction.step_z() * size)
    }
}
