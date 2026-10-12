use crisp_core::BlockPos;
use crisp_core::random::RandomSource;
use crisp_data::blocks;
use serde_json::Value;

use crate::world::level::world_gen_level::WorldGenLevel;
use crate::world::level::block::mirror::Mirror;
use crate::world::level::block::rotation::Rotation;
use crate::world::level::chunk::HeightmapType;
use crate::world::level::levelgen::blockpredicates::block_predicate::resource;
use crate::world::level::levelgen::structure::BoundingBox;
use crate::world::level::levelgen::structure::templatesystem::structure_place_settings::StructurePlaceSettings;
use crate::world::level::levelgen::structure::templatesystem::structure_processor::StructureProcessorListHolder;
use crate::world::level::levelgen::structure::templatesystem::structure_template_manager;

pub struct FossilFeature {
    fossil_structures: Vec<String>,
    overlay_structures: Vec<String>,
    fossil_processors: StructureProcessorListHolder,
    overlay_processors: StructureProcessorListHolder,
    max_empty_corners_allowed: i32,
}

fn identifiers(json: Option<&Value>) -> Result<Vec<String>, String> {
    let list: Vec<String> = json
        .and_then(Value::as_array)
        .ok_or("missing structure list")?
        .iter()
        .map(|id| id.as_str().map(resource).ok_or_else(|| "structure ids are strings".to_owned()))
        .collect::<Result<_, _>>()?;
    if list.is_empty() { Err("empty structure list".into()) } else { Ok(list) }
}

impl FossilFeature {
    pub fn parse(json: &Value) -> Result<FossilFeature, String> {
        let fossil = FossilFeature {
            fossil_structures: identifiers(json.get("fossil_structures"))?,
            overlay_structures: identifiers(json.get("overlay_structures"))?,
            fossil_processors: StructureProcessorListHolder::parse(json.get("fossil_processors").ok_or("missing fossil_processors")?)?,
            overlay_processors: StructureProcessorListHolder::parse(json.get("overlay_processors").ok_or("missing overlay_processors")?)?,
            max_empty_corners_allowed: json.get("max_empty_corners_allowed").and_then(Value::as_i64).ok_or("missing max_empty_corners_allowed")? as i32,
        };
        if fossil.fossil_structures.len() != fossil.overlay_structures.len() {
            return Err("Fossil structure lists must be equal lengths".into());
        }
        Ok(fossil)
    }

    pub fn place<R: RandomSource>(&self, level: &mut impl WorldGenLevel, random: &mut R, origin: BlockPos) -> bool {
        let rotation = Rotation::get_random(random);
        let fossil_index = random.next_int_bounded(self.fossil_structures.len() as i32) as usize;
        let fossil_base = structure_template_manager::get_or_create(&self.fossil_structures[fossil_index]);
        let fossil_overlay = structure_template_manager::get_or_create(&self.overlay_structures[fossil_index]);
        let chunk_pos = origin.chunk();
        let bounding_box = BoundingBox::new(
            chunk_pos.min_block_x() - 16,
            level.get_min_y(),
            chunk_pos.min_block_z() - 16,
            chunk_pos.min_block_x() + 15 + 16,
            level.get_max_y(),
            chunk_pos.min_block_z() + 15 + 16,
        );
        let mut settings = StructurePlaceSettings { rotation, bounding_box: Some(bounding_box), random: true, ..StructurePlaceSettings::default() };
        let size = fossil_base.get_size_rotated(rotation);
        let low_corner = origin.offset(-size.x / 2, 0, -size.z / 2);
        let mut lowest_surface_y = origin.y;
        for xscan in 0..size.x {
            for zscan in 0..size.z {
                lowest_surface_y = lowest_surface_y.min(level.get_height(HeightmapType::OceanFloorWg, low_corner.x + xscan, low_corner.z + zscan));
            }
        }
        let target_y = (lowest_surface_y - 15 - random.next_int_bounded(10)).max(level.get_min_y() + 10);
        let target_pos = fossil_base.get_zero_position_with_transform(BlockPos::new(low_corner.x, target_y, low_corner.z), Mirror::None, rotation);
        if count_empty_corners(level, fossil_base.get_bounding_box(&settings, target_pos)) > self.max_empty_corners_allowed {
            return false;
        }
        settings.processors = self.fossil_processors.value().list.iter().collect();
        fossil_base.place_in_world(level, target_pos, target_pos, &settings, random, 260);
        settings.processors = self.overlay_processors.value().list.iter().collect();
        fossil_overlay.place_in_world(level, target_pos, target_pos, &settings, random, 260);
        true
    }
}

fn count_empty_corners(level: &mut impl WorldGenLevel, structure_bounds: BoundingBox) -> i32 {
    let mut count = 0;
    structure_bounds.for_all_corners(|pos| {
        let state = level.get_block_state(pos);
        if state.is_air() || state.is_block(blocks::LAVA) || state.is_block(blocks::WATER) {
            count += 1;
        }
    });
    count
}
