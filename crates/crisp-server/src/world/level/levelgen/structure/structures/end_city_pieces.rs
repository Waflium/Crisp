use std::sync::LazyLock;

use crisp_core::BlockPos;
use crisp_core::random::RandomSource;
use crisp_data::blocks;
use crisp_protocol::nbt::Nbt;

use crate::world::level::block::mirror::Mirror;
use crate::world::level::block::rotation::Rotation;
use crate::world::level::levelgen::feature::monster_room_feature::set_block_entity_loot_table;
use crate::world::level::levelgen::structure::BoundingBox;
use crate::world::level::levelgen::structure::template_structure_piece::{TemplatePlaceSettings, TemplateStructurePiece};
use crate::world::level::levelgen::structure::templatesystem::structure_processor::StructureProcessor;
use crate::world::level::world_gen_level::WorldGenLevel;

pub const END_CITY_PIECE: &str = "minecraft:ecp";
const MAX_GEN_DEPTH: i32 = 8;
const END_CITY_TREASURE: &str = "minecraft:chests/end_city_treasure";
const TOWER_BRIDGES: [(Rotation, BlockPos); 4] = [
    (Rotation::None, BlockPos::new(1, -1, 0)),
    (Rotation::Clockwise90, BlockPos::new(6, -1, 1)),
    (Rotation::Counterclockwise90, BlockPos::new(0, -1, 5)),
    (Rotation::Clockwise180, BlockPos::new(5, -1, 6)),
];
const FAT_TOWER_BRIDGES: [(Rotation, BlockPos); 4] = [
    (Rotation::None, BlockPos::new(4, -1, 0)),
    (Rotation::Clockwise90, BlockPos::new(12, -1, 4)),
    (Rotation::Counterclockwise90, BlockPos::new(0, -1, 8)),
    (Rotation::Clockwise180, BlockPos::new(8, -1, 12)),
];

static STRUCTURE_BLOCK: LazyLock<[StructureProcessor; 1]> = LazyLock::new(|| [StructureProcessor::BlockIgnore(vec![blocks::STRUCTURE_BLOCK])]);
static STRUCTURE_AND_AIR: LazyLock<[StructureProcessor; 1]> = LazyLock::new(|| [StructureProcessor::BlockIgnore(vec![blocks::AIR, blocks::STRUCTURE_BLOCK])]);

#[derive(Clone, Copy)]
enum SectionGenerator {
    HouseTower,
    Tower,
    TowerBridge,
    FatTower,
}

struct Sections {
    pieces: Vec<EndCityPiece>,
    ship_created: bool,
}

impl Sections {
    fn rotation(&self, piece: usize) -> Rotation {
        self.pieces[piece].template.place_settings.rotation
    }

    fn set_gen_depth(&mut self, piece: usize, gen_depth: i32) {
        self.pieces[piece].template.piece.gen_depth = gen_depth;
    }

    fn add_piece(&mut self, parent: usize, offset: BlockPos, template_name: &'static str, rotation: Rotation, overwrite: bool) -> usize {
        let parent = &self.pieces[parent].template;
        let mut child = EndCityPiece::new(template_name, parent.template_position, rotation, overwrite);
        let origin = parent.template.calculate_connected_position(&parent.place_settings.settings(None), offset, &child.template.place_settings.settings(None), BlockPos::ZERO);
        child.template.move_by(origin.x, origin.y, origin.z);
        self.pieces.push(child);
        self.pieces.len() - 1
    }

    fn recursive_children(
        &mut self,
        generator: SectionGenerator,
        gen_depth: i32,
        parent: usize,
        offset: BlockPos,
        pieces: &mut Vec<usize>,
        random: &mut impl RandomSource,
    ) -> bool {
        if gen_depth > MAX_GEN_DEPTH {
            return false;
        }
        let mut child_pieces = Vec::new();
        if self.generate(generator, gen_depth, parent, offset, &mut child_pieces, random) {
            let mut collision = false;
            let child_tag = random.next_int();
            for &child in &child_pieces {
                self.set_gen_depth(child, child_tag);
                let bounding_box = self.pieces[child].template.piece.bounding_box;
                if let Some(&collision_piece) = pieces.iter().find(|&&piece| self.pieces[piece].template.piece.bounding_box.intersects(&bounding_box))
                    && self.pieces[collision_piece].template.piece.gen_depth != self.pieces[parent].template.piece.gen_depth
                {
                    collision = true;
                    break;
                }
            }
            if !collision {
                pieces.extend(child_pieces);
                return true;
            }
        }
        false
    }

    fn generate(&mut self, generator: SectionGenerator, gen_depth: i32, parent: usize, offset: BlockPos, pieces: &mut Vec<usize>, random: &mut impl RandomSource) -> bool {
        match generator {
            SectionGenerator::HouseTower => self.generate_house_tower(gen_depth, parent, offset, pieces, random),
            SectionGenerator::Tower => self.generate_tower(gen_depth, parent, pieces, random),
            SectionGenerator::TowerBridge => self.generate_tower_bridge(gen_depth, parent, pieces, random),
            SectionGenerator::FatTower => self.generate_fat_tower(gen_depth, parent, pieces, random),
        }
    }

    fn generate_house_tower(&mut self, gen_depth: i32, parent: usize, offset: BlockPos, pieces: &mut Vec<usize>, random: &mut impl RandomSource) -> bool {
        if gen_depth > MAX_GEN_DEPTH {
            return false;
        }
        let rotation = self.rotation(parent);
        let mut last_piece = add_helper(pieces, self.add_piece(parent, offset, "base_floor", rotation, true));
        let num_floors = random.next_int_bounded(3);
        if num_floors == 0 {
            add_helper(pieces, self.add_piece(last_piece, BlockPos::new(-1, 4, -1), "base_roof", rotation, true));
        } else if num_floors == 1 {
            last_piece = add_helper(pieces, self.add_piece(last_piece, BlockPos::new(-1, 0, -1), "second_floor_2", rotation, false));
            last_piece = add_helper(pieces, self.add_piece(last_piece, BlockPos::new(-1, 8, -1), "second_roof", rotation, false));
            self.recursive_children(SectionGenerator::Tower, gen_depth + 1, last_piece, BlockPos::ZERO, pieces, random);
        } else if num_floors == 2 {
            last_piece = add_helper(pieces, self.add_piece(last_piece, BlockPos::new(-1, 0, -1), "second_floor_2", rotation, false));
            last_piece = add_helper(pieces, self.add_piece(last_piece, BlockPos::new(-1, 4, -1), "third_floor_2", rotation, false));
            last_piece = add_helper(pieces, self.add_piece(last_piece, BlockPos::new(-1, 8, -1), "third_roof", rotation, true));
            self.recursive_children(SectionGenerator::Tower, gen_depth + 1, last_piece, BlockPos::ZERO, pieces, random);
        }
        true
    }

    fn generate_tower(&mut self, gen_depth: i32, parent: usize, pieces: &mut Vec<usize>, random: &mut impl RandomSource) -> bool {
        let rotation = self.rotation(parent);
        let mut last_piece = parent;
        let base_x = 3 + random.next_int_bounded(2);
        let base_z = 3 + random.next_int_bounded(2);
        last_piece = add_helper(pieces, self.add_piece(last_piece, BlockPos::new(base_x, -3, base_z), "tower_base", rotation, true));
        last_piece = add_helper(pieces, self.add_piece(last_piece, BlockPos::new(0, 7, 0), "tower_piece", rotation, true));
        let mut bridge_piece = if random.next_int_bounded(3) == 0 { Some(last_piece) } else { None };
        let tower_height = 1 + random.next_int_bounded(3);
        for i in 0..tower_height {
            last_piece = add_helper(pieces, self.add_piece(last_piece, BlockPos::new(0, 4, 0), "tower_piece", rotation, true));
            if i < tower_height - 1 && random.next_boolean() {
                bridge_piece = Some(last_piece);
            }
        }
        if let Some(bridge_piece) = bridge_piece {
            for (bridge_rotation, bridge_offset) in TOWER_BRIDGES {
                if random.next_boolean() {
                    let bridge_start = add_helper(pieces, self.add_piece(bridge_piece, bridge_offset, "bridge_end", rotation.get_rotated(bridge_rotation), true));
                    self.recursive_children(SectionGenerator::TowerBridge, gen_depth + 1, bridge_start, BlockPos::ZERO, pieces, random);
                }
            }
            add_helper(pieces, self.add_piece(last_piece, BlockPos::new(-1, 4, -1), "tower_top", rotation, true));
        } else {
            if gen_depth != 7 {
                return self.recursive_children(SectionGenerator::FatTower, gen_depth + 1, last_piece, BlockPos::ZERO, pieces, random);
            }
            add_helper(pieces, self.add_piece(last_piece, BlockPos::new(-1, 4, -1), "tower_top", rotation, true));
        }
        true
    }

    fn generate_tower_bridge(&mut self, gen_depth: i32, parent: usize, pieces: &mut Vec<usize>, random: &mut impl RandomSource) -> bool {
        let rotation = self.rotation(parent);
        let bridge_length = random.next_int_bounded(4) + 1;
        let mut last_piece = add_helper(pieces, self.add_piece(parent, BlockPos::new(0, 0, -4), "bridge_piece", rotation, true));
        self.set_gen_depth(last_piece, -1);
        let mut next_y = 0;
        for _ in 0..bridge_length {
            if random.next_boolean() {
                last_piece = add_helper(pieces, self.add_piece(last_piece, BlockPos::new(0, next_y, -4), "bridge_piece", rotation, true));
                next_y = 0;
            } else {
                if random.next_boolean() {
                    last_piece = add_helper(pieces, self.add_piece(last_piece, BlockPos::new(0, next_y, -4), "bridge_steep_stairs", rotation, true));
                } else {
                    last_piece = add_helper(pieces, self.add_piece(last_piece, BlockPos::new(0, next_y, -8), "bridge_gentle_stairs", rotation, true));
                }
                next_y = 4;
            }
        }
        if !self.ship_created && random.next_int_bounded(10 - gen_depth) == 0 {
            let ship_x = -8 + random.next_int_bounded(8);
            let ship_z = -70 + random.next_int_bounded(10);
            add_helper(pieces, self.add_piece(last_piece, BlockPos::new(ship_x, next_y, ship_z), "ship", rotation, true));
            self.ship_created = true;
        } else if !self.recursive_children(SectionGenerator::HouseTower, gen_depth + 1, last_piece, BlockPos::new(-3, next_y + 1, -11), pieces, random) {
            return false;
        }
        last_piece = add_helper(pieces, self.add_piece(last_piece, BlockPos::new(4, next_y, 0), "bridge_end", rotation.get_rotated(Rotation::Clockwise180), true));
        self.set_gen_depth(last_piece, -1);
        true
    }

    fn generate_fat_tower(&mut self, gen_depth: i32, parent: usize, pieces: &mut Vec<usize>, random: &mut impl RandomSource) -> bool {
        let rotation = self.rotation(parent);
        let mut last_piece = add_helper(pieces, self.add_piece(parent, BlockPos::new(-3, 4, -3), "fat_tower_base", rotation, true));
        last_piece = add_helper(pieces, self.add_piece(last_piece, BlockPos::new(0, 4, 0), "fat_tower_middle", rotation, true));
        let mut i = 0;
        while i < 2 && random.next_int_bounded(3) != 0 {
            last_piece = add_helper(pieces, self.add_piece(last_piece, BlockPos::new(0, 8, 0), "fat_tower_middle", rotation, true));
            for (bridge_rotation, bridge_offset) in FAT_TOWER_BRIDGES {
                if random.next_boolean() {
                    let bridge_start = add_helper(pieces, self.add_piece(last_piece, bridge_offset, "bridge_end", rotation.get_rotated(bridge_rotation), true));
                    self.recursive_children(SectionGenerator::TowerBridge, gen_depth + 1, bridge_start, BlockPos::ZERO, pieces, random);
                }
            }
            i += 1;
        }
        add_helper(pieces, self.add_piece(last_piece, BlockPos::new(-2, 8, -2), "fat_tower_top", rotation, true));
        true
    }
}

pub fn start_house_tower(origin: BlockPos, rotation: Rotation, random: &mut impl RandomSource) -> Vec<EndCityPiece> {
    let mut sections = Sections { pieces: vec![EndCityPiece::new("base_floor", origin, rotation, true)], ship_created: false };
    let mut pieces = Vec::new();
    let mut last_piece = add_helper(&mut pieces, 0);
    last_piece = add_helper(&mut pieces, sections.add_piece(last_piece, BlockPos::new(-1, 0, -1), "second_floor_1", rotation, false));
    last_piece = add_helper(&mut pieces, sections.add_piece(last_piece, BlockPos::new(-1, 4, -1), "third_floor_1", rotation, false));
    last_piece = add_helper(&mut pieces, sections.add_piece(last_piece, BlockPos::new(-1, 8, -1), "third_roof", rotation, true));
    sections.recursive_children(SectionGenerator::Tower, 1, last_piece, BlockPos::ZERO, &mut pieces, random);
    pieces.into_iter().map(|piece| sections.pieces[piece].clone()).collect()
}

fn add_helper(pieces: &mut Vec<usize>, piece: usize) -> usize {
    pieces.push(piece);
    piece
}

#[derive(Clone)]
pub struct EndCityPiece {
    pub template: TemplateStructurePiece,
}

fn make_settings(overwrite: bool, rotation: Rotation) -> TemplatePlaceSettings {
    let processors: &'static [StructureProcessor] = if overwrite { &*STRUCTURE_BLOCK } else { &*STRUCTURE_AND_AIR };
    TemplatePlaceSettings { mirror: Mirror::None, rotation, rotation_pivot: BlockPos::ZERO, apply_waterlogging: true, processors }
}

fn make_identifier(template_name: &str) -> String {
    format!("minecraft:end_city/{template_name}")
}

impl EndCityPiece {
    pub fn new(template_name: &str, position: BlockPos, rotation: Rotation, overwrite: bool) -> EndCityPiece {
        EndCityPiece {
            template: TemplateStructurePiece::new(END_CITY_PIECE, 0, &make_identifier(template_name), template_name.to_owned(), make_settings(overwrite, rotation), position),
        }
    }

    pub fn load(tag: &Nbt) -> Option<EndCityPiece> {
        let overwrite = matches!(tag.get("OW"), Some(Nbt::Byte(value)) if *value != 0);
        let rotation = tag.get("Rot").and_then(Nbt::as_str).and_then(Rotation::by_legacy_name)?;
        Some(EndCityPiece { template: TemplateStructurePiece::load_with_template_location(END_CITY_PIECE, tag, make_identifier, |_| make_settings(overwrite, rotation))? })
    }

    pub fn add_additional_save_data(&self, tag: &mut Vec<(String, Nbt)>) {
        self.template.add_additional_save_data(tag);
        tag.push(("Rot".into(), Nbt::String(self.template.place_settings.rotation.legacy_name().to_owned())));
        tag.push(("OW".into(), Nbt::Byte(std::ptr::eq(self.template.place_settings.processors.as_ptr(), STRUCTURE_BLOCK.as_ptr()) as i8)));
    }

    fn handle_data_marker(marker_id: &str, position: BlockPos, level: &mut impl WorldGenLevel, random: &mut impl RandomSource, chunk_bb: &BoundingBox) {
        if marker_id.starts_with("Chest") {
            let chest_position = position.below();
            if chunk_bb.is_inside(chest_position) {
                set_block_entity_loot_table(level, random, chest_position, END_CITY_TREASURE);
            }
        }
    }

    pub fn post_process(&mut self, level: &mut impl WorldGenLevel, random: &mut impl RandomSource, chunk_bb: &BoundingBox, reference_pos: BlockPos) {
        self.template.post_process(level, random, chunk_bb, reference_pos, |marker_id, position, level, random, chunk_bb| {
            EndCityPiece::handle_data_marker(marker_id, position, level, random, chunk_bb)
        });
    }
}
