use std::sync::LazyLock;

use crisp_core::BlockPos;
use crisp_core::random::RandomSource;
use crisp_data::blocks;
use crisp_protocol::nbt::Nbt;

use crate::server::level::world_gen_region::WorldGenRegion;
use crate::world::level::block::mirror::Mirror;
use crate::world::level::block::rotation::Rotation;
use crate::world::level::chunk::HeightmapType;
use crate::world::level::levelgen::structure::BoundingBox;
use crate::world::level::levelgen::structure::template_structure_piece::{TemplatePlaceSettings, TemplateStructurePiece};
use crate::world::level::levelgen::structure::templatesystem::structure_processor::StructureProcessor;

pub const SHIPWRECK_PIECE: &str = "minecraft:shipwreck";
const NUMBER_OF_BLOCKS_ALLOWED_IN_WORLD_GEN_REGION: i32 = 32;
const PIVOT: BlockPos = BlockPos::new(4, 0, 15);
const STRUCTURE_LOCATION_BEACHED: [&str; 11] = [
    "minecraft:shipwreck/with_mast",
    "minecraft:shipwreck/sideways_full",
    "minecraft:shipwreck/sideways_fronthalf",
    "minecraft:shipwreck/sideways_backhalf",
    "minecraft:shipwreck/rightsideup_full",
    "minecraft:shipwreck/rightsideup_fronthalf",
    "minecraft:shipwreck/rightsideup_backhalf",
    "minecraft:shipwreck/with_mast_degraded",
    "minecraft:shipwreck/rightsideup_full_degraded",
    "minecraft:shipwreck/rightsideup_fronthalf_degraded",
    "minecraft:shipwreck/rightsideup_backhalf_degraded",
];
const STRUCTURE_LOCATION_OCEAN: [&str; 20] = [
    "minecraft:shipwreck/with_mast",
    "minecraft:shipwreck/upsidedown_full",
    "minecraft:shipwreck/upsidedown_fronthalf",
    "minecraft:shipwreck/upsidedown_backhalf",
    "minecraft:shipwreck/sideways_full",
    "minecraft:shipwreck/sideways_fronthalf",
    "minecraft:shipwreck/sideways_backhalf",
    "minecraft:shipwreck/rightsideup_full",
    "minecraft:shipwreck/rightsideup_fronthalf",
    "minecraft:shipwreck/rightsideup_backhalf",
    "minecraft:shipwreck/with_mast_degraded",
    "minecraft:shipwreck/upsidedown_full_degraded",
    "minecraft:shipwreck/upsidedown_fronthalf_degraded",
    "minecraft:shipwreck/upsidedown_backhalf_degraded",
    "minecraft:shipwreck/sideways_full_degraded",
    "minecraft:shipwreck/sideways_fronthalf_degraded",
    "minecraft:shipwreck/sideways_backhalf_degraded",
    "minecraft:shipwreck/rightsideup_full_degraded",
    "minecraft:shipwreck/rightsideup_fronthalf_degraded",
    "minecraft:shipwreck/rightsideup_backhalf_degraded",
];

static STRUCTURE_AND_AIR: LazyLock<[StructureProcessor; 1]> = LazyLock::new(|| [StructureProcessor::BlockIgnore(vec![blocks::STRUCTURE_BLOCK, blocks::AIR])]);

fn markers_to_loot(marker_id: &str) -> Option<&'static str> {
    match marker_id {
        "map_chest" => Some("minecraft:chests/shipwreck_map"),
        "treasure_chest" => Some("minecraft:chests/shipwreck_treasure"),
        "supply_chest" => Some("minecraft:chests/shipwreck_supply"),
        _ => None,
    }
}

pub fn add_random_piece(position: BlockPos, rotation: Rotation, random: &mut impl RandomSource, is_beached: bool) -> ShipwreckPiece {
    let locations: &[&'static str] = if is_beached { &STRUCTURE_LOCATION_BEACHED } else { &STRUCTURE_LOCATION_OCEAN };
    let identifier = locations[random.next_int_bounded(locations.len() as i32) as usize];
    ShipwreckPiece::new(identifier, position, rotation, is_beached)
}

fn set_block_entity_loot_table(level: &mut WorldGenRegion, random: &mut impl RandomSource, pos: BlockPos, loot_table: &str) {
    if let Some(entity) = level.get_block_entity(pos)
        && entity.is_randomizable()
        && let Some(base) = entity.container_base_mut()
    {
        base.loot_table = Some(loot_table.to_owned());
        base.loot_table_seed = random.next_long();
    }
}

#[derive(Clone)]
pub struct ShipwreckPiece {
    pub template: TemplateStructurePiece,
    is_beached: bool,
    height_adjusted: bool,
}

fn make_settings(rotation: Rotation) -> TemplatePlaceSettings {
    TemplatePlaceSettings { mirror: Mirror::None, rotation, rotation_pivot: PIVOT, apply_waterlogging: true, processors: &*STRUCTURE_AND_AIR }
}

impl ShipwreckPiece {
    pub fn new(template_location: &'static str, position: BlockPos, rotation: Rotation, is_beached: bool) -> ShipwreckPiece {
        ShipwreckPiece {
            template: TemplateStructurePiece::new(SHIPWRECK_PIECE, 0, template_location, template_location.to_owned(), make_settings(rotation), position),
            is_beached,
            height_adjusted: false,
        }
    }

    pub fn load(tag: &Nbt) -> Option<ShipwreckPiece> {
        let rotation = tag.get("Rot").and_then(Nbt::as_str).and_then(Rotation::by_legacy_name)?;
        Some(ShipwreckPiece {
            template: TemplateStructurePiece::load(SHIPWRECK_PIECE, tag, |_| make_settings(rotation))?,
            is_beached: matches!(tag.get("isBeached"), Some(Nbt::Byte(1..))),
            height_adjusted: matches!(tag.get("height_adjusted"), Some(Nbt::Byte(1..))),
        })
    }

    pub fn add_additional_save_data(&self, tag: &mut Vec<(String, Nbt)>) {
        self.template.add_additional_save_data(tag);
        tag.push(("isBeached".into(), Nbt::Byte(self.is_beached as i8)));
        tag.push(("Rot".into(), Nbt::String(self.template.place_settings.rotation.legacy_name().to_owned())));
        tag.push(("height_adjusted".into(), Nbt::Byte(self.height_adjusted as i8)));
    }

    fn handle_data_marker(marker_id: &str, position: BlockPos, level: &mut WorldGenRegion, random: &mut impl RandomSource) {
        if let Some(loot_table) = markers_to_loot(marker_id) {
            set_block_entity_loot_table(level, random, position.below(), loot_table);
        }
    }

    pub fn post_process(&mut self, level: &mut WorldGenRegion, random: &mut impl RandomSource, chunk_bb: &BoundingBox, reference_pos: BlockPos) {
        if !self.height_adjusted && !self.is_too_big_to_fit_in_world_gen_region() {
            let mut min_y = level.get_max_y() + 1;
            let mut mean = 0;
            let template_size = self.template.template.get_size();
            let heightmap_type = if self.is_beached { HeightmapType::WorldSurfaceWg } else { HeightmapType::OceanFloorWg };
            let base_size = template_size.x * template_size.z;
            let template_position = self.template.template_position;
            if base_size == 0 {
                mean = level.get_height(heightmap_type, template_position.x, template_position.z);
            } else {
                for z in template_position.z..template_position.z + template_size.z {
                    for x in template_position.x..template_position.x + template_size.x {
                        let heightmap = level.get_height(heightmap_type, x, z);
                        mean += heightmap;
                        min_y = min_y.min(heightmap);
                    }
                }
                mean /= base_size;
            }
            let new_height = if self.is_beached { self.calculate_beached_position(min_y, random) } else { mean };
            self.adjust_position_height(new_height);
        }
        self.template.post_process(level, random, chunk_bb, reference_pos, |marker_id, position, level, random, _chunk_bb| {
            ShipwreckPiece::handle_data_marker(marker_id, position, level, random)
        });
    }

    pub fn is_too_big_to_fit_in_world_gen_region(&self) -> bool {
        let size = self.template.template.get_size();
        size.x > NUMBER_OF_BLOCKS_ALLOWED_IN_WORLD_GEN_REGION || size.y > NUMBER_OF_BLOCKS_ALLOWED_IN_WORLD_GEN_REGION
    }

    pub fn calculate_beached_position(&self, min_y: i32, random: &mut impl RandomSource) -> i32 {
        min_y - self.template.template.get_size().y / 2 - random.next_int_bounded(3)
    }

    pub fn adjust_position_height(&mut self, new_height: i32) {
        self.height_adjusted = true;
        let template_position = self.template.template_position;
        self.template.template_position = BlockPos::new(template_position.x, new_height, template_position.z);
    }
}
