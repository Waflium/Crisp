use std::sync::LazyLock;

use crisp_core::random::RandomSource;
use crisp_core::{Axis, BlockPos, Direction};
use crisp_data::properties::{EAST, HORIZONTAL_FACING, RAIL_SHAPE, RailShape, WEST};
use crisp_data::{BlockState, SupportType, blocks};
use crisp_protocol::nbt::Nbt;

use super::Piece;
use super::mineshaft_structure::Type;
use crate::world::level::world_gen_level::WorldGenLevel;
use crate::world::level::block;
use crate::world::level::block::entity::BlockEntityKind;
use crate::world::level::levelgen::structure::BoundingBox;
use crate::world::level::levelgen::structure::pieces::structure_pieces_builder::StructurePiecesBuilder;
use crate::world::level::levelgen::structure::structure_piece::{StructurePiece, is_replaceable_by_structures};

pub const MINE_SHAFT_CORRIDOR: &str = "minecraft:mscorridor";
pub const MINE_SHAFT_CROSSING: &str = "minecraft:mscrossing";
pub const MINE_SHAFT_ROOM: &str = "minecraft:msroom";
pub const MINE_SHAFT_STAIRS: &str = "minecraft:msstairs";
const ABANDONED_MINESHAFT: &str = "minecraft:chests/abandoned_mineshaft";

static MINESHAFT_BLOCKING: LazyLock<&'static [i32]> = LazyLock::new(|| crisp_data::tag("minecraft:worldgen/biome", "minecraft:mineshaft_blocking"));

fn cave_air() -> BlockState {
    blocks::CAVE_AIR.default_state()
}

fn int(tag: &Nbt, key: &str) -> i32 {
    match tag.get(key) {
        Some(Nbt::Int(v)) => *v,
        Some(Nbt::Byte(v)) => *v as i32,
        _ => 0,
    }
}

fn boolean(tag: &Nbt, key: &str) -> bool {
    matches!(tag.get(key), Some(Nbt::Byte(v)) if *v != 0)
}

#[allow(clippy::too_many_arguments)]
fn create_random_shaft_piece(
    builder: &StructurePiecesBuilder,
    random: &mut impl RandomSource,
    foot_x: i32,
    foot_y: i32,
    foot_z: i32,
    direction: Direction,
    gen_depth: i32,
    r#type: Type,
) -> Option<Piece> {
    let random_selection = random.next_int_bounded(100);
    if random_selection >= 80 {
        if let Some(crossing_box) = MineShaftCrossing::find_crossing(builder, random, foot_x, foot_y, foot_z, direction) {
            return Some(Piece::MineShaftCrossing(MineShaftCrossing::new(gen_depth, crossing_box, direction, r#type)));
        }
    } else if random_selection >= 70 {
        if let Some(stairs_box) = MineShaftStairs::find_stairs(builder, random, foot_x, foot_y, foot_z, direction) {
            return Some(Piece::MineShaftStairs(MineShaftStairs::new(gen_depth, stairs_box, direction, r#type)));
        }
    } else if let Some(corridor_box) = MineShaftCorridor::find_corridor_size(builder, random, foot_x, foot_y, foot_z, direction) {
        return Some(Piece::MineShaftCorridor(MineShaftCorridor::new(gen_depth, random, corridor_box, direction, r#type)));
    }
    None
}

#[allow(clippy::too_many_arguments)]
fn generate_and_add_piece(
    start_piece: &MineShaftPiece,
    builder: &mut StructurePiecesBuilder,
    random: &mut impl RandomSource,
    foot_x: i32,
    foot_y: i32,
    foot_z: i32,
    direction: Direction,
    depth: i32,
) -> Option<BoundingBox> {
    if depth > 8 {
        return None;
    }
    if (foot_x - start_piece.piece.bounding_box.min.x).abs() <= 80 && (foot_z - start_piece.piece.bounding_box.min.z).abs() <= 80 {
        let r#type = start_piece.r#type;
        let new_piece = create_random_shaft_piece(builder, random, foot_x, foot_y, foot_z, direction, depth + 1, r#type)?;
        builder.add_piece(new_piece.clone());
        match &new_piece {
            Piece::MineShaftCorridor(piece) => piece.add_children(start_piece, builder, random),
            Piece::MineShaftCrossing(piece) => piece.add_children(start_piece, builder, random),
            Piece::MineShaftStairs(piece) => piece.add_children(start_piece, builder, random),
            _ => {}
        }
        Some(new_piece.piece().bounding_box)
    } else {
        None
    }
}

#[derive(Clone)]
pub struct MineShaftCorridor {
    pub mine_shaft: MineShaftPiece,
    has_rails: bool,
    spider_corridor: bool,
    has_placed_spider: bool,
    num_sections: i32,
}

impl MineShaftCorridor {
    pub fn load(tag: &Nbt) -> Option<MineShaftCorridor> {
        Some(MineShaftCorridor {
            mine_shaft: MineShaftPiece::load(MINE_SHAFT_CORRIDOR, tag)?,
            has_rails: boolean(tag, "hr"),
            spider_corridor: boolean(tag, "sc"),
            has_placed_spider: boolean(tag, "hps"),
            num_sections: int(tag, "Num"),
        })
    }

    pub fn add_additional_save_data(&self, tag: &mut Vec<(String, Nbt)>) {
        self.mine_shaft.add_additional_save_data(tag);
        tag.push(("hr".into(), Nbt::Byte(self.has_rails as i8)));
        tag.push(("sc".into(), Nbt::Byte(self.spider_corridor as i8)));
        tag.push(("hps".into(), Nbt::Byte(self.has_placed_spider as i8)));
        tag.push(("Num".into(), Nbt::Int(self.num_sections)));
    }

    pub fn new(gen_depth: i32, random: &mut impl RandomSource, bounding_box: BoundingBox, direction: Direction, r#type: Type) -> MineShaftCorridor {
        let mut mine_shaft = MineShaftPiece::new(MINE_SHAFT_CORRIDOR, gen_depth, r#type, bounding_box);
        mine_shaft.piece.set_orientation(Some(direction));
        let has_rails = random.next_int_bounded(3) == 0;
        let spider_corridor = !has_rails && random.next_int_bounded(23) == 0;
        let num_sections = if direction.axis() == Axis::Z { bounding_box.z_span() / 5 } else { bounding_box.x_span() / 5 };
        MineShaftCorridor { mine_shaft, has_rails, spider_corridor, has_placed_spider: false, num_sections }
    }

    pub fn find_corridor_size(
        builder: &StructurePiecesBuilder,
        random: &mut impl RandomSource,
        foot_x: i32,
        foot_y: i32,
        foot_z: i32,
        direction: Direction,
    ) -> Option<BoundingBox> {
        let mut corridor_length = random.next_int_bounded(3) + 2;
        while corridor_length > 0 {
            let block_length = corridor_length * 5;
            let mut bounding_box = match direction {
                Direction::South => BoundingBox::new(0, 0, 0, 2, 2, block_length - 1),
                Direction::West => BoundingBox::new(-(block_length - 1), 0, 0, 0, 2, 2),
                Direction::East => BoundingBox::new(0, 0, 0, block_length - 1, 2, 2),
                _ => BoundingBox::new(0, 0, -(block_length - 1), 2, 2, 0),
            };
            bounding_box.move_in_place(foot_x, foot_y, foot_z);
            if builder.find_collision_piece(&bounding_box).is_none() {
                return Some(bounding_box);
            }
            corridor_length -= 1;
        }
        None
    }

    pub fn add_children(&self, start_piece: &MineShaftPiece, builder: &mut StructurePiecesBuilder, random: &mut impl RandomSource) {
        let bounding_box = self.mine_shaft.piece.bounding_box;
        let depth = self.mine_shaft.piece.gen_depth;
        let end_selection = random.next_int_bounded(4);
        let orientation = self.mine_shaft.piece.get_orientation();
        if let Some(orientation) = orientation {
            match orientation {
                Direction::South => {
                    if end_selection <= 1 {
                        let y = bounding_box.min.y - 1 + random.next_int_bounded(3);
                        generate_and_add_piece(start_piece, builder, random, bounding_box.min.x, y, bounding_box.max.z + 1, orientation, depth);
                    } else if end_selection == 2 {
                        let y = bounding_box.min.y - 1 + random.next_int_bounded(3);
                        generate_and_add_piece(start_piece, builder, random, bounding_box.min.x - 1, y, bounding_box.max.z - 3, Direction::West, depth);
                    } else {
                        let y = bounding_box.min.y - 1 + random.next_int_bounded(3);
                        generate_and_add_piece(start_piece, builder, random, bounding_box.max.x + 1, y, bounding_box.max.z - 3, Direction::East, depth);
                    }
                }
                Direction::West => {
                    if end_selection <= 1 {
                        let y = bounding_box.min.y - 1 + random.next_int_bounded(3);
                        generate_and_add_piece(start_piece, builder, random, bounding_box.min.x - 1, y, bounding_box.min.z, orientation, depth);
                    } else if end_selection == 2 {
                        let y = bounding_box.min.y - 1 + random.next_int_bounded(3);
                        generate_and_add_piece(start_piece, builder, random, bounding_box.min.x, y, bounding_box.min.z - 1, Direction::North, depth);
                    } else {
                        let y = bounding_box.min.y - 1 + random.next_int_bounded(3);
                        generate_and_add_piece(start_piece, builder, random, bounding_box.min.x, y, bounding_box.max.z + 1, Direction::South, depth);
                    }
                }
                Direction::East => {
                    if end_selection <= 1 {
                        let y = bounding_box.min.y - 1 + random.next_int_bounded(3);
                        generate_and_add_piece(start_piece, builder, random, bounding_box.max.x + 1, y, bounding_box.min.z, orientation, depth);
                    } else if end_selection == 2 {
                        let y = bounding_box.min.y - 1 + random.next_int_bounded(3);
                        generate_and_add_piece(start_piece, builder, random, bounding_box.max.x - 3, y, bounding_box.min.z - 1, Direction::North, depth);
                    } else {
                        let y = bounding_box.min.y - 1 + random.next_int_bounded(3);
                        generate_and_add_piece(start_piece, builder, random, bounding_box.max.x - 3, y, bounding_box.max.z + 1, Direction::South, depth);
                    }
                }
                _ => {
                    if end_selection <= 1 {
                        let y = bounding_box.min.y - 1 + random.next_int_bounded(3);
                        generate_and_add_piece(start_piece, builder, random, bounding_box.min.x, y, bounding_box.min.z - 1, orientation, depth);
                    } else if end_selection == 2 {
                        let y = bounding_box.min.y - 1 + random.next_int_bounded(3);
                        generate_and_add_piece(start_piece, builder, random, bounding_box.min.x - 1, y, bounding_box.min.z, Direction::West, depth);
                    } else {
                        let y = bounding_box.min.y - 1 + random.next_int_bounded(3);
                        generate_and_add_piece(start_piece, builder, random, bounding_box.max.x + 1, y, bounding_box.min.z, Direction::East, depth);
                    }
                }
            }
        }
        if depth < 8 {
            if orientation != Some(Direction::North) && orientation != Some(Direction::South) {
                let mut x = bounding_box.min.x + 3;
                while x + 3 <= bounding_box.max.x {
                    let selection = random.next_int_bounded(5);
                    if selection == 0 {
                        generate_and_add_piece(start_piece, builder, random, x, bounding_box.min.y, bounding_box.min.z - 1, Direction::North, depth + 1);
                    } else if selection == 1 {
                        generate_and_add_piece(start_piece, builder, random, x, bounding_box.min.y, bounding_box.max.z + 1, Direction::South, depth + 1);
                    }
                    x += 5;
                }
            } else {
                let mut z = bounding_box.min.z + 3;
                while z + 3 <= bounding_box.max.z {
                    let selection = random.next_int_bounded(5);
                    if selection == 0 {
                        generate_and_add_piece(start_piece, builder, random, bounding_box.min.x - 1, bounding_box.min.y, z, Direction::West, depth + 1);
                    } else if selection == 1 {
                        generate_and_add_piece(start_piece, builder, random, bounding_box.max.x + 1, bounding_box.min.y, z, Direction::East, depth + 1);
                    }
                    z += 5;
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn create_chest(&self, level: &mut impl WorldGenLevel, chunk_bb: &BoundingBox, random: &mut impl RandomSource, x: i32, y: i32, z: i32, _loot_table: &str) -> bool {
        let pos = self.mine_shaft.piece.get_world_pos(x, y, z);
        if chunk_bb.is_inside(pos) && level.get_block_state(pos).is_air() && !level.get_block_state(pos.below()).is_air() {
            let state = blocks::RAIL.default_state().set(RAIL_SHAPE, if random.next_boolean() { RailShape::NorthSouth } else { RailShape::EastWest });
            self.mine_shaft.place_block(level, state, x, y, z, chunk_bb);
            let _loot_table_seed = random.next_long();
            true
        } else {
            false
        }
    }

    pub fn post_process(&mut self, level: &mut impl WorldGenLevel, random: &mut impl RandomSource, chunk_bb: &BoundingBox) {
        if self.mine_shaft.is_in_invalid_location(level, chunk_bb) {
            return;
        }
        let length = self.num_sections * 5 - 1;
        let planks = self.mine_shaft.r#type.get_planks_state();
        let mine_shaft = &self.mine_shaft;
        mine_shaft.generate_box(level, chunk_bb, 0, 0, 0, 2, 1, length, cave_air(), cave_air(), false);
        mine_shaft.generate_maybe_box(level, chunk_bb, random, 0.8, 0, 2, 0, 2, 2, length, cave_air(), cave_air(), false, false);
        if self.spider_corridor {
            mine_shaft.generate_maybe_box(level, chunk_bb, random, 0.6, 0, 0, 0, 2, 1, length, blocks::COBWEB.default_state(), cave_air(), false, true);
        }
        for section in 0..self.num_sections {
            let z = 2 + section * 5;
            self.place_support(level, chunk_bb, 0, 0, z, 2, 2, random);
            self.maybe_place_cob_web(level, chunk_bb, random, 0.1, 0, 2, z - 1);
            self.maybe_place_cob_web(level, chunk_bb, random, 0.1, 2, 2, z - 1);
            self.maybe_place_cob_web(level, chunk_bb, random, 0.1, 0, 2, z + 1);
            self.maybe_place_cob_web(level, chunk_bb, random, 0.1, 2, 2, z + 1);
            self.maybe_place_cob_web(level, chunk_bb, random, 0.05, 0, 2, z - 2);
            self.maybe_place_cob_web(level, chunk_bb, random, 0.05, 2, 2, z - 2);
            self.maybe_place_cob_web(level, chunk_bb, random, 0.05, 0, 2, z + 2);
            self.maybe_place_cob_web(level, chunk_bb, random, 0.05, 2, 2, z + 2);
            if random.next_int_bounded(100) == 0 {
                self.create_chest(level, chunk_bb, random, 2, 0, z - 1, ABANDONED_MINESHAFT);
            }
            if random.next_int_bounded(100) == 0 {
                self.create_chest(level, chunk_bb, random, 0, 0, z + 1, ABANDONED_MINESHAFT);
            }
            if self.spider_corridor && !self.has_placed_spider {
                let new_z = z - 1 + random.next_int_bounded(3);
                let pos = self.mine_shaft.piece.get_world_pos(1, 0, new_z);
                if chunk_bb.is_inside(pos) && self.mine_shaft.piece.is_interior(level, 1, 0, new_z, chunk_bb) {
                    self.has_placed_spider = true;
                    level.set_block(pos, blocks::SPAWNER.default_state(), 2);
                    if let Some(entity) = level.get_block_entity(pos)
                        && let BlockEntityKind::Spawner(spawner) = &mut entity.kind
                    {
                        spawner.spawner.set_entity_id("minecraft:cave_spider", random);
                    }
                }
            }
        }
        let mine_shaft = &self.mine_shaft;
        for x in 0..=2 {
            for z in 0..=length {
                mine_shaft.set_planks_block(level, chunk_bb, planks, x, -1, z);
            }
        }
        self.place_double_lower_or_upper_support(level, chunk_bb, 0, -1, 2);
        if self.num_sections > 1 {
            let last_support_pillar = length - 2;
            self.place_double_lower_or_upper_support(level, chunk_bb, 0, -1, last_support_pillar);
        }
        if self.has_rails {
            let state = blocks::RAIL.default_state().set(RAIL_SHAPE, RailShape::NorthSouth);
            for z in 0..=length {
                let floor = mine_shaft.piece.get_block(level, 1, -1, z, chunk_bb);
                if !floor.is_air() && floor.is_solid_render() {
                    let probability = if mine_shaft.piece.is_interior(level, 1, 0, z, chunk_bb) { 0.7 } else { 0.9 };
                    mine_shaft.maybe_generate_block(level, chunk_bb, random, probability, 1, 0, z, state);
                }
            }
        }
    }

    fn place_double_lower_or_upper_support(&self, level: &mut impl WorldGenLevel, chunk_bb: &BoundingBox, x: i32, y: i32, z: i32) {
        let wood_block = self.mine_shaft.r#type.get_wood_state();
        let plank_block = self.mine_shaft.r#type.get_planks_state();
        if self.mine_shaft.piece.get_block(level, x, y, z, chunk_bb).is_block(plank_block.block()) {
            self.fill_pillar_down_or_chain_up(level, wood_block, x, y, z, chunk_bb);
        }
        if self.mine_shaft.piece.get_block(level, x + 2, y, z, chunk_bb).is_block(plank_block.block()) {
            self.fill_pillar_down_or_chain_up(level, wood_block, x + 2, y, z, chunk_bb);
        }
    }

    fn fill_pillar_down_or_chain_up(&self, level: &mut impl WorldGenLevel, pillar_state: BlockState, x: i32, y: i32, z: i32, chunk_bb: &BoundingBox) {
        let mut pos = self.mine_shaft.piece.get_world_pos(x, y, z);
        if !chunk_bb.is_inside(pos) {
            return;
        }
        let world_y = pos.y;
        let mut distance_from_world_y = 1;
        let mut check_below = true;
        let mut check_above = true;
        while check_below || check_above {
            if check_below {
                pos.y = world_y - distance_from_world_y;
                let below_state = level.get_block_state(pos);
                let empty_below = is_replaceable_by_structures(below_state) && !below_state.is_block(blocks::LAVA);
                if !empty_below && can_place_column_on_top_of(below_state) {
                    fill_column_between(level, pillar_state, pos, world_y - distance_from_world_y + 1, world_y);
                    return;
                }
                check_below = distance_from_world_y <= 20 && empty_below && pos.y > level.get_min_y() + 1;
            }
            if check_above {
                pos.y = world_y + distance_from_world_y;
                let above_state = level.get_block_state(pos);
                let empty_above = is_replaceable_by_structures(above_state);
                if !empty_above && can_hang_chain_below(level, pos, above_state) {
                    pos.y = world_y + 1;
                    level.set_block(pos, self.mine_shaft.r#type.get_fence_state(), 2);
                    fill_column_between(level, blocks::IRON_CHAIN.default_state(), pos, world_y + 2, world_y + distance_from_world_y);
                    return;
                }
                check_above = distance_from_world_y <= 50 && empty_above && pos.y < level.get_max_y();
            }
            distance_from_world_y += 1;
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn place_support(&self, level: &mut impl WorldGenLevel, chunk_bb: &BoundingBox, x0: i32, y0: i32, z: i32, y1: i32, x1: i32, random: &mut impl RandomSource) {
        let mine_shaft = &self.mine_shaft;
        if !mine_shaft.is_supporting_box(level, chunk_bb, x0, x1, y1, z) {
            return;
        }
        let planks_block = mine_shaft.r#type.get_planks_state();
        let fence_block = mine_shaft.r#type.get_fence_state();
        mine_shaft.generate_box(level, chunk_bb, x0, y0, z, x0, y1 - 1, z, fence_block.set(WEST, true), cave_air(), false);
        mine_shaft.generate_box(level, chunk_bb, x1, y0, z, x1, y1 - 1, z, fence_block.set(EAST, true), cave_air(), false);
        if random.next_int_bounded(4) == 0 {
            mine_shaft.generate_box(level, chunk_bb, x0, y1, z, x0, y1, z, planks_block, cave_air(), false);
            mine_shaft.generate_box(level, chunk_bb, x1, y1, z, x1, y1, z, planks_block, cave_air(), false);
        } else {
            mine_shaft.generate_box(level, chunk_bb, x0, y1, z, x1, y1, z, planks_block, cave_air(), false);
            let wall_torch = blocks::WALL_TORCH.default_state();
            mine_shaft.maybe_generate_block(level, chunk_bb, random, 0.05, x0 + 1, y1, z - 1, wall_torch.set(HORIZONTAL_FACING, Direction::South));
            mine_shaft.maybe_generate_block(level, chunk_bb, random, 0.05, x0 + 1, y1, z + 1, wall_torch.set(HORIZONTAL_FACING, Direction::North));
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn maybe_place_cob_web(&self, level: &mut impl WorldGenLevel, chunk_bb: &BoundingBox, random: &mut impl RandomSource, probability: f32, x: i32, y: i32, z: i32) {
        if self.mine_shaft.piece.is_interior(level, x, y, z, chunk_bb) && random.next_float() < probability && self.has_sturdy_neighbours(level, chunk_bb, x, y, z, 2) {
            self.mine_shaft.place_block(level, blocks::COBWEB.default_state(), x, y, z, chunk_bb);
        }
    }

    fn has_sturdy_neighbours(&self, level: &mut impl WorldGenLevel, chunk_bb: &BoundingBox, x: i32, y: i32, z: i32, count: i32) -> bool {
        let world_pos = self.mine_shaft.piece.get_world_pos(x, y, z);
        let mut sturdy_neighbours = 0;
        for direction in Direction::VALUES {
            let pos = world_pos.relative(direction);
            if chunk_bb.is_inside(pos) && level.get_block_state(pos).is_face_sturdy(direction.opposite(), SupportType::Full) {
                sturdy_neighbours += 1;
                if sturdy_neighbours >= count {
                    return true;
                }
            }
        }
        false
    }
}

fn fill_column_between(level: &mut impl WorldGenLevel, pillar_state: BlockState, mut pos: BlockPos, bottom_inclusive: i32, top_exclusive: i32) {
    for pillar_y in bottom_inclusive..top_exclusive {
        pos.y = pillar_y;
        level.set_block(pos, pillar_state, 2);
    }
}

fn can_place_column_on_top_of(state_below: BlockState) -> bool {
    state_below.is_face_sturdy(Direction::Up, SupportType::Full)
}

fn can_hang_chain_below(level: &mut impl WorldGenLevel, pos_above: BlockPos, state_above: BlockState) -> bool {
    block::can_support_center(level, pos_above, Direction::Down) && !state_above.block().is_instance_of("FallingBlock")
}

#[derive(Clone)]
pub struct MineShaftCrossing {
    pub mine_shaft: MineShaftPiece,
    direction: Direction,
    is_two_floored: bool,
}

impl MineShaftCrossing {
    pub fn load(tag: &Nbt) -> Option<MineShaftCrossing> {
        Some(MineShaftCrossing {
            mine_shaft: MineShaftPiece::load(MINE_SHAFT_CROSSING, tag)?,
            is_two_floored: boolean(tag, "tf"),
            direction: if tag.get("D").is_some() { Direction::from_2d_data_value(int(tag, "D")) } else { Direction::South },
        })
    }

    pub fn add_additional_save_data(&self, tag: &mut Vec<(String, Nbt)>) {
        self.mine_shaft.add_additional_save_data(tag);
        tag.push(("tf".into(), Nbt::Byte(self.is_two_floored as i8)));
        tag.push(("D".into(), Nbt::Byte(self.direction.get_2d_data_value() as i8)));
    }

    pub fn new(gen_depth: i32, bounding_box: BoundingBox, direction: Direction, r#type: Type) -> MineShaftCrossing {
        MineShaftCrossing {
            mine_shaft: MineShaftPiece::new(MINE_SHAFT_CROSSING, gen_depth, r#type, bounding_box),
            direction,
            is_two_floored: bounding_box.y_span() > 3,
        }
    }

    pub fn find_crossing(
        builder: &StructurePiecesBuilder,
        random: &mut impl RandomSource,
        foot_x: i32,
        foot_y: i32,
        foot_z: i32,
        direction: Direction,
    ) -> Option<BoundingBox> {
        let y1 = if random.next_int_bounded(4) == 0 { 6 } else { 2 };
        let mut bounding_box = match direction {
            Direction::South => BoundingBox::new(-1, 0, 0, 3, y1, 4),
            Direction::West => BoundingBox::new(-4, 0, -1, 0, y1, 3),
            Direction::East => BoundingBox::new(0, 0, -1, 4, y1, 3),
            _ => BoundingBox::new(-1, 0, -4, 3, y1, 0),
        };
        bounding_box.move_in_place(foot_x, foot_y, foot_z);
        if builder.find_collision_piece(&bounding_box).is_some() { None } else { Some(bounding_box) }
    }

    pub fn add_children(&self, start_piece: &MineShaftPiece, builder: &mut StructurePiecesBuilder, random: &mut impl RandomSource) {
        let bounding_box = self.mine_shaft.piece.bounding_box;
        let depth = self.mine_shaft.piece.gen_depth;
        let (min, max) = (bounding_box.min, bounding_box.max);
        match self.direction {
            Direction::South => {
                generate_and_add_piece(start_piece, builder, random, min.x + 1, min.y, max.z + 1, Direction::South, depth);
                generate_and_add_piece(start_piece, builder, random, min.x - 1, min.y, min.z + 1, Direction::West, depth);
                generate_and_add_piece(start_piece, builder, random, max.x + 1, min.y, min.z + 1, Direction::East, depth);
            }
            Direction::West => {
                generate_and_add_piece(start_piece, builder, random, min.x + 1, min.y, min.z - 1, Direction::North, depth);
                generate_and_add_piece(start_piece, builder, random, min.x + 1, min.y, max.z + 1, Direction::South, depth);
                generate_and_add_piece(start_piece, builder, random, min.x - 1, min.y, min.z + 1, Direction::West, depth);
            }
            Direction::East => {
                generate_and_add_piece(start_piece, builder, random, min.x + 1, min.y, min.z - 1, Direction::North, depth);
                generate_and_add_piece(start_piece, builder, random, min.x + 1, min.y, max.z + 1, Direction::South, depth);
                generate_and_add_piece(start_piece, builder, random, max.x + 1, min.y, min.z + 1, Direction::East, depth);
            }
            _ => {
                generate_and_add_piece(start_piece, builder, random, min.x + 1, min.y, min.z - 1, Direction::North, depth);
                generate_and_add_piece(start_piece, builder, random, min.x - 1, min.y, min.z + 1, Direction::West, depth);
                generate_and_add_piece(start_piece, builder, random, max.x + 1, min.y, min.z + 1, Direction::East, depth);
            }
        }
        if self.is_two_floored {
            if random.next_boolean() {
                generate_and_add_piece(start_piece, builder, random, min.x + 1, min.y + 3 + 1, min.z - 1, Direction::North, depth);
            }
            if random.next_boolean() {
                generate_and_add_piece(start_piece, builder, random, min.x - 1, min.y + 3 + 1, min.z + 1, Direction::West, depth);
            }
            if random.next_boolean() {
                generate_and_add_piece(start_piece, builder, random, max.x + 1, min.y + 3 + 1, min.z + 1, Direction::East, depth);
            }
            if random.next_boolean() {
                generate_and_add_piece(start_piece, builder, random, min.x + 1, min.y + 3 + 1, max.z + 1, Direction::South, depth);
            }
        }
    }

    pub fn post_process(&mut self, level: &mut impl WorldGenLevel, chunk_bb: &BoundingBox) {
        let mine_shaft = &self.mine_shaft;
        if mine_shaft.is_in_invalid_location(level, chunk_bb) {
            return;
        }
        let planks = mine_shaft.r#type.get_planks_state();
        let (min, max) = (mine_shaft.piece.bounding_box.min, mine_shaft.piece.bounding_box.max);
        if self.is_two_floored {
            mine_shaft.generate_box(level, chunk_bb, min.x + 1, min.y, min.z, max.x - 1, min.y + 3 - 1, max.z, cave_air(), cave_air(), false);
            mine_shaft.generate_box(level, chunk_bb, min.x, min.y, min.z + 1, max.x, min.y + 3 - 1, max.z - 1, cave_air(), cave_air(), false);
            mine_shaft.generate_box(level, chunk_bb, min.x + 1, max.y - 2, min.z, max.x - 1, max.y, max.z, cave_air(), cave_air(), false);
            mine_shaft.generate_box(level, chunk_bb, min.x, max.y - 2, min.z + 1, max.x, max.y, max.z - 1, cave_air(), cave_air(), false);
            mine_shaft.generate_box(level, chunk_bb, min.x + 1, min.y + 3, min.z + 1, max.x - 1, min.y + 3, max.z - 1, cave_air(), cave_air(), false);
        } else {
            mine_shaft.generate_box(level, chunk_bb, min.x + 1, min.y, min.z, max.x - 1, max.y, max.z, cave_air(), cave_air(), false);
            mine_shaft.generate_box(level, chunk_bb, min.x, min.y, min.z + 1, max.x, max.y, max.z - 1, cave_air(), cave_air(), false);
        }
        self.place_support_pillar(level, chunk_bb, min.x + 1, min.y, min.z + 1, max.y);
        self.place_support_pillar(level, chunk_bb, min.x + 1, min.y, max.z - 1, max.y);
        self.place_support_pillar(level, chunk_bb, max.x - 1, min.y, min.z + 1, max.y);
        self.place_support_pillar(level, chunk_bb, max.x - 1, min.y, max.z - 1, max.y);
        let y = min.y - 1;
        for x in min.x..=max.x {
            for z in min.z..=max.z {
                mine_shaft.set_planks_block(level, chunk_bb, planks, x, y, z);
            }
        }
    }

    fn place_support_pillar(&self, level: &mut impl WorldGenLevel, chunk_bb: &BoundingBox, x: i32, y0: i32, z: i32, y1: i32) {
        if !self.mine_shaft.piece.get_block(level, x, y1 + 1, z, chunk_bb).is_air() {
            self.mine_shaft.generate_box(level, chunk_bb, x, y0, z, x, y1, z, self.mine_shaft.r#type.get_planks_state(), cave_air(), false);
        }
    }
}

#[derive(Clone)]
pub struct MineShaftPiece {
    pub piece: StructurePiece,
    pub r#type: Type,
}

impl MineShaftPiece {
    pub fn new(piece_type: &'static str, gen_depth: i32, r#type: Type, bounding_box: BoundingBox) -> MineShaftPiece {
        MineShaftPiece { piece: StructurePiece::new(piece_type, gen_depth, bounding_box), r#type }
    }

    pub fn load(piece_type: &'static str, tag: &Nbt) -> Option<MineShaftPiece> {
        Some(MineShaftPiece { piece: StructurePiece::load(piece_type, tag)?, r#type: Type::by_id(int(tag, "MST")) })
    }

    fn can_be_replaced(&self, level: &mut impl WorldGenLevel, x: i32, y: i32, z: i32, chunk_bb: &BoundingBox) -> bool {
        let state = self.piece.get_block(level, x, y, z, chunk_bb);
        !state.is_block(self.r#type.get_planks_state().block())
            && !state.is_block(self.r#type.get_wood_state().block())
            && !state.is_block(self.r#type.get_fence_state().block())
            && !state.is_block(blocks::IRON_CHAIN)
    }

    pub fn add_additional_save_data(&self, tag: &mut Vec<(String, Nbt)>) {
        tag.push(("MST".into(), Nbt::Int(self.r#type.ordinal())));
    }

    fn is_supporting_box(&self, level: &mut impl WorldGenLevel, chunk_bb: &BoundingBox, x0: i32, x1: i32, y1: i32, z0: i32) -> bool {
        (x0..=x1).all(|x| !self.piece.get_block(level, x, y1 + 1, z0, chunk_bb).is_air())
    }

    fn is_in_invalid_location(&self, level: &mut impl WorldGenLevel, chunk_bb: &BoundingBox) -> bool {
        let bounding_box = self.piece.bounding_box;
        let x0 = (bounding_box.min.x - 1).max(chunk_bb.min.x);
        let y0 = (bounding_box.min.y - 1).max(chunk_bb.min.y);
        let z0 = (bounding_box.min.z - 1).max(chunk_bb.min.z);
        let x1 = (bounding_box.max.x + 1).min(chunk_bb.max.x);
        let y1 = (bounding_box.max.y + 1).min(chunk_bb.max.y);
        let z1 = (bounding_box.max.z + 1).min(chunk_bb.max.z);
        let block_pos = BlockPos::new((x0 + x1) / 2, (y0 + y1) / 2, (z0 + z1) / 2);
        if MINESHAFT_BLOCKING.contains(&(level.get_biome(block_pos).0 as i32)) {
            return true;
        }
        for x in x0..=x1 {
            for z in z0..=z1 {
                if level.get_block_state(BlockPos::new(x, y0, z)).liquid() {
                    return true;
                }
                if level.get_block_state(BlockPos::new(x, y1, z)).liquid() {
                    return true;
                }
            }
        }
        for x in x0..=x1 {
            for y in y0..=y1 {
                if level.get_block_state(BlockPos::new(x, y, z0)).liquid() {
                    return true;
                }
                if level.get_block_state(BlockPos::new(x, y, z1)).liquid() {
                    return true;
                }
            }
        }
        for z in z0..=z1 {
            for y in y0..=y1 {
                if level.get_block_state(BlockPos::new(x0, y, z)).liquid() {
                    return true;
                }
                if level.get_block_state(BlockPos::new(x1, y, z)).liquid() {
                    return true;
                }
            }
        }
        false
    }

    fn set_planks_block(&self, level: &mut impl WorldGenLevel, chunk_bb: &BoundingBox, planks_block: BlockState, x: i32, y: i32, z: i32) {
        if self.piece.is_interior(level, x, y, z, chunk_bb) {
            let pos = self.piece.get_world_pos(x, y, z);
            let existing_state = level.get_block_state(pos);
            if !existing_state.is_face_sturdy(Direction::Up, SupportType::Full) {
                level.set_block(pos, planks_block, 2);
            }
        }
    }

    fn place_block(&self, level: &mut impl WorldGenLevel, block_state: BlockState, x: i32, y: i32, z: i32, chunk_bb: &BoundingBox) {
        if chunk_bb.is_inside(self.piece.get_world_pos(x, y, z)) && self.can_be_replaced(level, x, y, z, chunk_bb) {
            self.piece.place_block(level, block_state, x, y, z, chunk_bb);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn generate_box(
        &self,
        level: &mut impl WorldGenLevel,
        chunk_bb: &BoundingBox,
        x0: i32,
        y0: i32,
        z0: i32,
        x1: i32,
        y1: i32,
        z1: i32,
        edge_block: BlockState,
        fill_block: BlockState,
        skip_air: bool,
    ) {
        for y in y0..=y1 {
            for x in x0..=x1 {
                for z in z0..=z1 {
                    if !skip_air || !self.piece.get_block(level, x, y, z, chunk_bb).is_air() {
                        if y != y0 && y != y1 && x != x0 && x != x1 && z != z0 && z != z1 {
                            self.place_block(level, fill_block, x, y, z, chunk_bb);
                        } else {
                            self.place_block(level, edge_block, x, y, z, chunk_bb);
                        }
                    }
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn generate_maybe_box(
        &self,
        level: &mut impl WorldGenLevel,
        chunk_bb: &BoundingBox,
        random: &mut impl RandomSource,
        probability: f32,
        x0: i32,
        y0: i32,
        z0: i32,
        x1: i32,
        y1: i32,
        z1: i32,
        edge_block: BlockState,
        fill_block: BlockState,
        skip_air: bool,
        has_to_be_inside: bool,
    ) {
        for y in y0..=y1 {
            for x in x0..=x1 {
                for z in z0..=z1 {
                    if random.next_float() <= probability
                        && (!skip_air || !self.piece.get_block(level, x, y, z, chunk_bb).is_air())
                        && (!has_to_be_inside || self.piece.is_interior(level, x, y, z, chunk_bb))
                    {
                        if y != y0 && y != y1 && x != x0 && x != x1 && z != z0 && z != z1 {
                            self.place_block(level, fill_block, x, y, z, chunk_bb);
                        } else {
                            self.place_block(level, edge_block, x, y, z, chunk_bb);
                        }
                    }
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn maybe_generate_block(&self, level: &mut impl WorldGenLevel, chunk_bb: &BoundingBox, random: &mut impl RandomSource, probability: f32, x: i32, y: i32, z: i32, block_state: BlockState) {
        if random.next_float() < probability {
            self.place_block(level, block_state, x, y, z, chunk_bb);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn generate_upper_half_sphere(
        &self,
        level: &mut impl WorldGenLevel,
        chunk_bb: &BoundingBox,
        x0: i32,
        y0: i32,
        z0: i32,
        x1: i32,
        y1: i32,
        z1: i32,
        fill_block: BlockState,
        skip_air: bool,
    ) {
        let diag_x = (x1 - x0 + 1) as f32;
        let diag_y = (y1 - y0 + 1) as f32;
        let diag_z = (z1 - z0 + 1) as f32;
        let cx = x0 as f32 + diag_x / 2.0;
        let cz = z0 as f32 + diag_z / 2.0;
        for y in y0..=y1 {
            let normalized_y_distance = (y - y0) as f32 / diag_y;
            for x in x0..=x1 {
                let normalized_x_distance = (x as f32 - cx) / (diag_x * 0.5);
                for z in z0..=z1 {
                    let normalized_z_distance = (z as f32 - cz) / (diag_z * 0.5);
                    if !skip_air || !self.piece.get_block(level, x, y, z, chunk_bb).is_air() {
                        let dist = normalized_x_distance * normalized_x_distance
                            + normalized_y_distance * normalized_y_distance
                            + normalized_z_distance * normalized_z_distance;
                        if dist <= 1.05 {
                            self.place_block(level, fill_block, x, y, z, chunk_bb);
                        }
                    }
                }
            }
        }
    }
}

#[derive(Clone)]
pub struct MineShaftRoom {
    pub mine_shaft: MineShaftPiece,
    child_entrance_boxes: Vec<BoundingBox>,
}

impl MineShaftRoom {
    pub fn new(gen_depth: i32, random: &mut impl RandomSource, west: i32, north: i32, r#type: Type) -> MineShaftRoom {
        let max_x = west + 7 + random.next_int_bounded(6);
        let max_y = 54 + random.next_int_bounded(6);
        let max_z = north + 7 + random.next_int_bounded(6);
        MineShaftRoom { mine_shaft: MineShaftPiece::new(MINE_SHAFT_ROOM, gen_depth, r#type, BoundingBox::new(west, 50, north, max_x, max_y, max_z)), child_entrance_boxes: Vec::new() }
    }

    pub fn load(tag: &Nbt) -> Option<MineShaftRoom> {
        let child_entrance_boxes = tag.get("Entrances").map(Nbt::as_list).unwrap_or(&[]).iter().filter_map(BoundingBox::from_nbt).collect();
        Some(MineShaftRoom { mine_shaft: MineShaftPiece::load(MINE_SHAFT_ROOM, tag)?, child_entrance_boxes })
    }

    pub fn add_children(&mut self, builder: &mut StructurePiecesBuilder, random: &mut impl RandomSource) {
        let start_piece = &self.mine_shaft;
        let bounding_box = start_piece.piece.bounding_box;
        let depth = start_piece.piece.gen_depth;
        let height_space = (bounding_box.y_span() - 3 - 1).max(1);
        let mut pos = 0;
        while pos < bounding_box.x_span() {
            pos += random.next_int_bounded(bounding_box.x_span());
            if pos + 3 > bounding_box.x_span() {
                break;
            }
            let y = bounding_box.min.y + random.next_int_bounded(height_space) + 1;
            if let Some(child_box) = generate_and_add_piece(start_piece, builder, random, bounding_box.min.x + pos, y, bounding_box.min.z - 1, Direction::North, depth) {
                self.child_entrance_boxes.push(BoundingBox::new(child_box.min.x, child_box.min.y, bounding_box.min.z, child_box.max.x, child_box.max.y, bounding_box.min.z + 1));
            }
            pos += 4;
        }
        pos = 0;
        while pos < bounding_box.x_span() {
            pos += random.next_int_bounded(bounding_box.x_span());
            if pos + 3 > bounding_box.x_span() {
                break;
            }
            let y = bounding_box.min.y + random.next_int_bounded(height_space) + 1;
            if let Some(child_box) = generate_and_add_piece(start_piece, builder, random, bounding_box.min.x + pos, y, bounding_box.max.z + 1, Direction::South, depth) {
                self.child_entrance_boxes.push(BoundingBox::new(child_box.min.x, child_box.min.y, bounding_box.max.z - 1, child_box.max.x, child_box.max.y, bounding_box.max.z));
            }
            pos += 4;
        }
        pos = 0;
        while pos < bounding_box.z_span() {
            pos += random.next_int_bounded(bounding_box.z_span());
            if pos + 3 > bounding_box.z_span() {
                break;
            }
            let y = bounding_box.min.y + random.next_int_bounded(height_space) + 1;
            if let Some(child_box) = generate_and_add_piece(start_piece, builder, random, bounding_box.min.x - 1, y, bounding_box.min.z + pos, Direction::West, depth) {
                self.child_entrance_boxes.push(BoundingBox::new(bounding_box.min.x, child_box.min.y, child_box.min.z, bounding_box.min.x + 1, child_box.max.y, child_box.max.z));
            }
            pos += 4;
        }
        pos = 0;
        while pos < bounding_box.z_span() {
            pos += random.next_int_bounded(bounding_box.z_span());
            if pos + 3 > bounding_box.z_span() {
                break;
            }
            let y = bounding_box.min.y + random.next_int_bounded(height_space) + 1;
            if let Some(child_box) = generate_and_add_piece(start_piece, builder, random, bounding_box.max.x + 1, y, bounding_box.min.z + pos, Direction::East, depth) {
                self.child_entrance_boxes.push(BoundingBox::new(bounding_box.max.x - 1, child_box.min.y, child_box.min.z, bounding_box.max.x, child_box.max.y, child_box.max.z));
            }
            pos += 4;
        }
    }

    pub fn post_process(&self, level: &mut impl WorldGenLevel, chunk_bb: &BoundingBox) {
        let mine_shaft = &self.mine_shaft;
        if mine_shaft.is_in_invalid_location(level, chunk_bb) {
            return;
        }
        let (min, max) = (mine_shaft.piece.bounding_box.min, mine_shaft.piece.bounding_box.max);
        mine_shaft.generate_box(level, chunk_bb, min.x, min.y + 1, min.z, max.x, (min.y + 3).min(max.y), max.z, cave_air(), cave_air(), false);
        for entrance_box in &self.child_entrance_boxes {
            mine_shaft.generate_box(
                level,
                chunk_bb,
                entrance_box.min.x,
                entrance_box.max.y - 2,
                entrance_box.min.z,
                entrance_box.max.x,
                entrance_box.max.y,
                entrance_box.max.z,
                cave_air(),
                cave_air(),
                false,
            );
        }
        mine_shaft.generate_upper_half_sphere(level, chunk_bb, min.x, min.y + 4, min.z, max.x, max.y, max.z, cave_air(), false);
    }

    pub fn move_by(&mut self, dx: i32, dy: i32, dz: i32) {
        self.mine_shaft.piece.move_by(dx, dy, dz);
        for bounding_box in &mut self.child_entrance_boxes {
            bounding_box.move_in_place(dx, dy, dz);
        }
    }

    pub fn add_additional_save_data(&self, tag: &mut Vec<(String, Nbt)>) {
        self.mine_shaft.add_additional_save_data(tag);
        tag.push(("Entrances".into(), Nbt::List(self.child_entrance_boxes.iter().map(BoundingBox::to_nbt).collect())));
    }
}

#[derive(Clone)]
pub struct MineShaftStairs {
    pub mine_shaft: MineShaftPiece,
}

impl MineShaftStairs {
    pub fn new(gen_depth: i32, bounding_box: BoundingBox, direction: Direction, r#type: Type) -> MineShaftStairs {
        let mut mine_shaft = MineShaftPiece::new(MINE_SHAFT_STAIRS, gen_depth, r#type, bounding_box);
        mine_shaft.piece.set_orientation(Some(direction));
        MineShaftStairs { mine_shaft }
    }

    pub fn load(tag: &Nbt) -> Option<MineShaftStairs> {
        Some(MineShaftStairs { mine_shaft: MineShaftPiece::load(MINE_SHAFT_STAIRS, tag)? })
    }

    pub fn find_stairs(
        builder: &StructurePiecesBuilder,
        _random: &mut impl RandomSource,
        foot_x: i32,
        foot_y: i32,
        foot_z: i32,
        direction: Direction,
    ) -> Option<BoundingBox> {
        let mut bounding_box = match direction {
            Direction::South => BoundingBox::new(0, -5, 0, 2, 2, 8),
            Direction::West => BoundingBox::new(-8, -5, 0, 0, 2, 2),
            Direction::East => BoundingBox::new(0, -5, 0, 8, 2, 2),
            _ => BoundingBox::new(0, -5, -8, 2, 2, 0),
        };
        bounding_box.move_in_place(foot_x, foot_y, foot_z);
        if builder.find_collision_piece(&bounding_box).is_some() { None } else { Some(bounding_box) }
    }

    pub fn add_children(&self, start_piece: &MineShaftPiece, builder: &mut StructurePiecesBuilder, random: &mut impl RandomSource) {
        let bounding_box = self.mine_shaft.piece.bounding_box;
        let depth = self.mine_shaft.piece.gen_depth;
        if let Some(orientation) = self.mine_shaft.piece.get_orientation() {
            match orientation {
                Direction::South => {
                    generate_and_add_piece(start_piece, builder, random, bounding_box.min.x, bounding_box.min.y, bounding_box.max.z + 1, Direction::South, depth);
                }
                Direction::West => {
                    generate_and_add_piece(start_piece, builder, random, bounding_box.min.x - 1, bounding_box.min.y, bounding_box.min.z, Direction::West, depth);
                }
                Direction::East => {
                    generate_and_add_piece(start_piece, builder, random, bounding_box.max.x + 1, bounding_box.min.y, bounding_box.min.z, Direction::East, depth);
                }
                _ => {
                    generate_and_add_piece(start_piece, builder, random, bounding_box.min.x, bounding_box.min.y, bounding_box.min.z - 1, Direction::North, depth);
                }
            }
        }
    }

    pub fn post_process(&self, level: &mut impl WorldGenLevel, chunk_bb: &BoundingBox) {
        let mine_shaft = &self.mine_shaft;
        if mine_shaft.is_in_invalid_location(level, chunk_bb) {
            return;
        }
        mine_shaft.generate_box(level, chunk_bb, 0, 5, 0, 2, 7, 1, cave_air(), cave_air(), false);
        mine_shaft.generate_box(level, chunk_bb, 0, 0, 7, 2, 2, 8, cave_air(), cave_air(), false);
        for i in 0..5 {
            mine_shaft.generate_box(level, chunk_bb, 0, 5 - i - if i < 4 { 1 } else { 0 }, 2 + i, 2, 7 - i, 2 + i, cave_air(), cave_air(), false);
        }
    }
}
