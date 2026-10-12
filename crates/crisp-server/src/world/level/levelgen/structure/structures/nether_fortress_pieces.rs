use crisp_core::Direction;
use crisp_core::random::{LegacyRandomSource, RandomSource};
use crisp_data::properties::{EAST, HORIZONTAL_FACING, NORTH, SOUTH, WEST};
use crisp_data::{BlockState, Fluid, blocks};
use crisp_protocol::nbt::Nbt;

use super::Piece;
use crate::world::level::block::entity::BlockEntityKind;
use crate::world::level::levelgen::structure::BoundingBox;
use crate::world::level::levelgen::structure::pieces::structure_pieces_builder::StructurePiecesBuilder;
use crate::world::level::levelgen::structure::structure_piece::{StructurePiece, get_random_horizontal_direction, make_bounding_box};
use crate::world::level::world_gen_level::WorldGenLevel;

pub const NETHER_FORTRESS_BRIDGE_CROSSING: &str = "minecraft:nebcr";
pub const NETHER_FORTRESS_BRIDGE_END_FILLER: &str = "minecraft:nebef";
pub const NETHER_FORTRESS_BRIDGE_STRAIGHT: &str = "minecraft:nebs";
pub const NETHER_FORTRESS_CASTLE_CORRIDOR_STAIRS: &str = "minecraft:neccs";
pub const NETHER_FORTRESS_CASTLE_CORRIDOR_T_BALCONY: &str = "minecraft:nectb";
pub const NETHER_FORTRESS_CASTLE_ENTRANCE: &str = "minecraft:nece";
pub const NETHER_FORTRESS_CASTLE_SMALL_CORRIDOR_CROSSING: &str = "minecraft:nescsc";
pub const NETHER_FORTRESS_CASTLE_SMALL_CORRIDOR_LEFT_TURN: &str = "minecraft:nesclt";
pub const NETHER_FORTRESS_CASTLE_SMALL_CORRIDOR: &str = "minecraft:nesc";
pub const NETHER_FORTRESS_CASTLE_SMALL_CORRIDOR_RIGHT_TURN: &str = "minecraft:nescrt";
pub const NETHER_FORTRESS_CASTLE_STALK_ROOM: &str = "minecraft:necsr";
pub const NETHER_FORTRESS_MONSTER_THRONE: &str = "minecraft:nemt";
pub const NETHER_FORTRESS_ROOM_CROSSING: &str = "minecraft:nerc";
pub const NETHER_FORTRESS_STAIRS_ROOM: &str = "minecraft:nesr";
pub const NETHER_FORTRESS_START: &str = "minecraft:nestart";

const NETHER_BRIDGE_LOOT: &str = "minecraft:chests/nether_bridge";

const MAX_DEPTH: i32 = 30;
const LOWEST_Y_POSITION: i32 = 10;
pub const MAGIC_START_Y: i32 = 64;

fn int(tag: &Nbt, key: &str) -> i32 {
    match tag.get(key) {
        Some(Nbt::Int(v)) => *v,
        _ => 0,
    }
}

fn boolean(tag: &Nbt, key: &str) -> bool {
    matches!(tag.get(key), Some(Nbt::Byte(v)) if *v != 0)
}

fn nether_bricks() -> BlockState {
    blocks::NETHER_BRICKS.default_state()
}

fn air() -> BlockState {
    blocks::AIR.default_state()
}

fn fence() -> BlockState {
    blocks::NETHER_BRICK_FENCE.default_state()
}

fn ns_fence() -> BlockState {
    fence().set(NORTH, true).set(SOUTH, true)
}

fn we_fence() -> BlockState {
    fence().set(WEST, true).set(EAST, true)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PieceClass {
    BridgeStraight,
    BridgeCrossing,
    RoomCrossing,
    StairsRoom,
    MonsterThrone,
    CastleEntrance,
    CastleSmallCorridorPiece,
    CastleSmallCorridorCrossingPiece,
    CastleSmallCorridorRightTurnPiece,
    CastleSmallCorridorLeftTurnPiece,
    CastleCorridorStairsPiece,
    CastleCorridorTBalconyPiece,
    CastleStalkRoom,
}

#[derive(Clone, Copy)]
struct PieceWeight {
    piece_class: PieceClass,
    weight: i32,
    place_count: i32,
    max_place_count: i32,
    allow_in_row: bool,
}

impl PieceWeight {
    const fn new(piece_class: PieceClass, weight: i32, max_place_count: i32, allow_in_row: bool) -> PieceWeight {
        PieceWeight { piece_class, weight, place_count: 0, max_place_count, allow_in_row }
    }

    fn do_place(&self, _depth: i32) -> bool {
        self.max_place_count == 0 || self.place_count < self.max_place_count
    }

    fn is_valid(&self) -> bool {
        self.max_place_count == 0 || self.place_count < self.max_place_count
    }
}

const BRIDGE_PIECE_WEIGHTS: [PieceWeight; 6] = [
    PieceWeight::new(PieceClass::BridgeStraight, 30, 0, true),
    PieceWeight::new(PieceClass::BridgeCrossing, 10, 4, false),
    PieceWeight::new(PieceClass::RoomCrossing, 10, 4, false),
    PieceWeight::new(PieceClass::StairsRoom, 10, 3, false),
    PieceWeight::new(PieceClass::MonsterThrone, 5, 2, false),
    PieceWeight::new(PieceClass::CastleEntrance, 5, 1, false),
];

const CASTLE_PIECE_WEIGHTS: [PieceWeight; 7] = [
    PieceWeight::new(PieceClass::CastleSmallCorridorPiece, 25, 0, true),
    PieceWeight::new(PieceClass::CastleSmallCorridorCrossingPiece, 15, 5, false),
    PieceWeight::new(PieceClass::CastleSmallCorridorRightTurnPiece, 5, 10, false),
    PieceWeight::new(PieceClass::CastleSmallCorridorLeftTurnPiece, 5, 10, false),
    PieceWeight::new(PieceClass::CastleCorridorStairsPiece, 10, 3, true),
    PieceWeight::new(PieceClass::CastleCorridorTBalconyPiece, 7, 2, false),
    PieceWeight::new(PieceClass::CastleStalkRoom, 5, 2, false),
];

#[allow(clippy::too_many_arguments)]
fn find_and_create_bridge_piece_factory(
    piece: PieceClass,
    builder: &StructurePiecesBuilder,
    random: &mut impl RandomSource,
    foot_x: i32,
    foot_y: i32,
    foot_z: i32,
    direction: Direction,
    depth: i32,
) -> Option<NetherBridgePieceKind> {
    match piece {
        PieceClass::BridgeStraight => BridgeStraight::create_piece(builder, random, foot_x, foot_y, foot_z, direction, depth).map(NetherBridgePieceKind::BridgeStraight),
        PieceClass::BridgeCrossing => BridgeCrossing::create_piece(builder, foot_x, foot_y, foot_z, direction, depth).map(NetherBridgePieceKind::BridgeCrossing),
        PieceClass::RoomCrossing => RoomCrossing::create_piece(builder, foot_x, foot_y, foot_z, direction, depth).map(NetherBridgePieceKind::RoomCrossing),
        PieceClass::StairsRoom => StairsRoom::create_piece(builder, foot_x, foot_y, foot_z, depth, direction).map(NetherBridgePieceKind::StairsRoom),
        PieceClass::MonsterThrone => MonsterThrone::create_piece(builder, foot_x, foot_y, foot_z, depth, direction).map(NetherBridgePieceKind::MonsterThrone),
        PieceClass::CastleEntrance => CastleEntrance::create_piece(builder, random, foot_x, foot_y, foot_z, direction, depth).map(NetherBridgePieceKind::CastleEntrance),
        PieceClass::CastleSmallCorridorPiece => {
            CastleSmallCorridorPiece::create_piece(builder, foot_x, foot_y, foot_z, direction, depth).map(NetherBridgePieceKind::CastleSmallCorridorPiece)
        }
        PieceClass::CastleSmallCorridorRightTurnPiece => CastleSmallCorridorRightTurnPiece::create_piece(builder, random, foot_x, foot_y, foot_z, direction, depth)
            .map(NetherBridgePieceKind::CastleSmallCorridorRightTurnPiece),
        PieceClass::CastleSmallCorridorLeftTurnPiece => CastleSmallCorridorLeftTurnPiece::create_piece(builder, random, foot_x, foot_y, foot_z, direction, depth)
            .map(NetherBridgePieceKind::CastleSmallCorridorLeftTurnPiece),
        PieceClass::CastleCorridorStairsPiece => {
            CastleCorridorStairsPiece::create_piece(builder, foot_x, foot_y, foot_z, direction, depth).map(NetherBridgePieceKind::CastleCorridorStairsPiece)
        }
        PieceClass::CastleCorridorTBalconyPiece => {
            CastleCorridorTBalconyPiece::create_piece(builder, foot_x, foot_y, foot_z, direction, depth).map(NetherBridgePieceKind::CastleCorridorTBalconyPiece)
        }
        PieceClass::CastleSmallCorridorCrossingPiece => {
            CastleSmallCorridorCrossingPiece::create_piece(builder, foot_x, foot_y, foot_z, direction, depth).map(NetherBridgePieceKind::CastleSmallCorridorCrossingPiece)
        }
        PieceClass::CastleStalkRoom => CastleStalkRoom::create_piece(builder, foot_x, foot_y, foot_z, direction, depth).map(NetherBridgePieceKind::CastleStalkRoom),
    }
}

fn is_free_box(builder: &StructurePiecesBuilder, bounding_box: &BoundingBox) -> bool {
    NetherBridgePiece::is_ok_box(bounding_box) && builder.find_collision_piece(bounding_box).is_none()
}

#[derive(Clone)]
pub struct NetherBridgePiece {
    pub piece: StructurePiece,
}

impl NetherBridgePiece {
    fn oriented(piece_type: &'static str, gen_depth: i32, bounding_box: BoundingBox, direction: Direction) -> NetherBridgePiece {
        let mut piece = StructurePiece::new(piece_type, gen_depth, bounding_box);
        piece.set_orientation(Some(direction));
        NetherBridgePiece { piece }
    }

    fn load(piece_type: &'static str, tag: &Nbt) -> Option<NetherBridgePiece> {
        Some(NetherBridgePiece { piece: StructurePiece::load(piece_type, tag)? })
    }

    fn update_piece_weight(current_pieces: &[PieceWeight]) -> i32 {
        let mut has_any_pieces = false;
        let mut total_weight = 0;
        for piece in current_pieces {
            if piece.max_place_count > 0 && piece.place_count < piece.max_place_count {
                has_any_pieces = true;
            }
            total_weight += piece.weight;
        }
        if has_any_pieces { total_weight } else { -1 }
    }

    #[allow(clippy::too_many_arguments)]
    fn generate_piece(
        start_piece: &mut StartPiece,
        is_castle: bool,
        builder: &StructurePiecesBuilder,
        random: &mut impl RandomSource,
        foot_x: i32,
        foot_y: i32,
        foot_z: i32,
        direction: Direction,
        depth: i32,
    ) -> Option<NetherBridgePieceKind> {
        let current_pieces = if is_castle { &mut start_piece.available_castle_pieces } else { &mut start_piece.available_bridge_pieces };
        let total_weight = NetherBridgePiece::update_piece_weight(current_pieces);
        let do_stuff = total_weight > 0 && depth <= MAX_DEPTH;
        let mut num_attempts = 0;
        while num_attempts < 5 && do_stuff {
            num_attempts += 1;
            let mut weight_selection = random.next_int_bounded(total_weight);
            for index in 0..current_pieces.len() {
                let piece = current_pieces[index];
                weight_selection -= piece.weight;
                if weight_selection < 0 {
                    if !piece.do_place(depth) || start_piece.previous_piece == Some(piece.piece_class) && !piece.allow_in_row {
                        break;
                    }
                    if let Some(structure_piece) = find_and_create_bridge_piece_factory(piece.piece_class, builder, random, foot_x, foot_y, foot_z, direction, depth) {
                        current_pieces[index].place_count += 1;
                        start_piece.previous_piece = Some(piece.piece_class);
                        if !current_pieces[index].is_valid() {
                            current_pieces.remove(index);
                        }
                        return Some(structure_piece);
                    }
                }
            }
        }
        BridgeEndFiller::create_piece(builder, random, foot_x, foot_y, foot_z, direction, depth).map(NetherBridgePieceKind::BridgeEndFiller)
    }

    #[allow(clippy::too_many_arguments)]
    fn generate_and_add_piece(
        start_piece: &mut StartPiece,
        builder: &mut StructurePiecesBuilder,
        random: &mut impl RandomSource,
        foot_x: i32,
        foot_y: i32,
        foot_z: i32,
        direction: Direction,
        depth: i32,
        is_castle: bool,
    ) {
        let start_box = start_piece.bridge_crossing.nether_bridge.piece.bounding_box;
        if (foot_x - start_box.min.x).abs() <= 112 && (foot_z - start_box.min.z).abs() <= 112 {
            if let Some(new_piece) = NetherBridgePiece::generate_piece(start_piece, is_castle, builder, random, foot_x, foot_y, foot_z, direction, depth + 1) {
                builder.add_piece(Piece::NetherFortress(new_piece.clone()));
                start_piece.pending_children.push(new_piece);
            }
        } else {
            BridgeEndFiller::create_piece(builder, random, foot_x, foot_y, foot_z, direction, depth);
        }
    }

    fn generate_child_forward(&self, start_piece: &mut StartPiece, builder: &mut StructurePiecesBuilder, random: &mut impl RandomSource, x_off: i32, y_off: i32, is_castle: bool) {
        let Some(orientation) = self.piece.get_orientation() else { return };
        let bounding_box = &self.piece.bounding_box;
        let (foot_x, foot_z) = match orientation {
            Direction::North => (bounding_box.min.x + x_off, bounding_box.min.z - 1),
            Direction::South => (bounding_box.min.x + x_off, bounding_box.max.z + 1),
            Direction::West => (bounding_box.min.x - 1, bounding_box.min.z + x_off),
            Direction::East => (bounding_box.max.x + 1, bounding_box.min.z + x_off),
            _ => return,
        };
        NetherBridgePiece::generate_and_add_piece(start_piece, builder, random, foot_x, bounding_box.min.y + y_off, foot_z, orientation, self.piece.gen_depth, is_castle);
    }

    fn generate_child_left(&self, start_piece: &mut StartPiece, builder: &mut StructurePiecesBuilder, random: &mut impl RandomSource, y_off: i32, z_off: i32, is_castle: bool) {
        let Some(orientation) = self.piece.get_orientation() else { return };
        let bounding_box = &self.piece.bounding_box;
        let (foot_x, foot_z, direction) = match orientation {
            Direction::North | Direction::South => (bounding_box.min.x - 1, bounding_box.min.z + z_off, Direction::West),
            Direction::West | Direction::East => (bounding_box.min.x + z_off, bounding_box.min.z - 1, Direction::North),
            _ => return,
        };
        NetherBridgePiece::generate_and_add_piece(start_piece, builder, random, foot_x, bounding_box.min.y + y_off, foot_z, direction, self.piece.gen_depth, is_castle);
    }

    fn generate_child_right(&self, start_piece: &mut StartPiece, builder: &mut StructurePiecesBuilder, random: &mut impl RandomSource, y_off: i32, z_off: i32, is_castle: bool) {
        let Some(orientation) = self.piece.get_orientation() else { return };
        let bounding_box = &self.piece.bounding_box;
        let (foot_x, foot_z, direction) = match orientation {
            Direction::North | Direction::South => (bounding_box.max.x + 1, bounding_box.min.z + z_off, Direction::East),
            Direction::West | Direction::East => (bounding_box.min.x + z_off, bounding_box.max.z + 1, Direction::South),
            _ => return,
        };
        NetherBridgePiece::generate_and_add_piece(start_piece, builder, random, foot_x, bounding_box.min.y + y_off, foot_z, direction, self.piece.gen_depth, is_castle);
    }

    fn is_ok_box(bounding_box: &BoundingBox) -> bool {
        bounding_box.min.y > LOWEST_Y_POSITION
    }
}

#[derive(Clone)]
pub struct BridgeCrossing {
    pub nether_bridge: NetherBridgePiece,
}

impl BridgeCrossing {
    fn new(gen_depth: i32, bounding_box: BoundingBox, direction: Direction) -> BridgeCrossing {
        BridgeCrossing { nether_bridge: NetherBridgePiece::oriented(NETHER_FORTRESS_BRIDGE_CROSSING, gen_depth, bounding_box, direction) }
    }

    fn new_source(west: i32, north: i32, direction: Direction) -> BridgeCrossing {
        BridgeCrossing::new(0, make_bounding_box(west, MAGIC_START_Y, north, direction, 19, 10, 19), direction)
    }

    fn load(piece_type: &'static str, tag: &Nbt) -> Option<BridgeCrossing> {
        Some(BridgeCrossing { nether_bridge: NetherBridgePiece::load(piece_type, tag)? })
    }

    pub fn add_children(&self, start_piece: &mut StartPiece, builder: &mut StructurePiecesBuilder, random: &mut impl RandomSource) {
        self.nether_bridge.generate_child_forward(start_piece, builder, random, 8, 3, false);
        self.nether_bridge.generate_child_left(start_piece, builder, random, 3, 8, false);
        self.nether_bridge.generate_child_right(start_piece, builder, random, 3, 8, false);
    }

    fn create_piece(builder: &StructurePiecesBuilder, foot_x: i32, foot_y: i32, foot_z: i32, direction: Direction, gen_depth: i32) -> Option<BridgeCrossing> {
        let bounding_box = BoundingBox::orient_box(foot_x, foot_y, foot_z, -8, -3, 0, 19, 10, 19, direction);
        is_free_box(builder, &bounding_box).then(|| BridgeCrossing::new(gen_depth, bounding_box, direction))
    }

    fn post_process(&self, level: &mut impl WorldGenLevel, chunk_bb: &BoundingBox) {
        let piece = &self.nether_bridge.piece;
        let bricks = nether_bricks();
        piece.generate_box(level, chunk_bb, 7, 3, 0, 11, 4, 18, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 0, 3, 7, 18, 4, 11, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 8, 5, 0, 10, 7, 18, air(), air(), false);
        piece.generate_box(level, chunk_bb, 0, 5, 8, 18, 7, 10, air(), air(), false);
        piece.generate_box(level, chunk_bb, 7, 5, 0, 7, 5, 7, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 7, 5, 11, 7, 5, 18, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 11, 5, 0, 11, 5, 7, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 11, 5, 11, 11, 5, 18, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 0, 5, 7, 7, 5, 7, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 11, 5, 7, 18, 5, 7, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 0, 5, 11, 7, 5, 11, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 11, 5, 11, 18, 5, 11, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 7, 2, 0, 11, 2, 5, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 7, 2, 13, 11, 2, 18, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 7, 0, 0, 11, 1, 3, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 7, 0, 15, 11, 1, 18, bricks, bricks, false);
        for x in 7..=11 {
            for z in 0..=2 {
                piece.fill_column_down(level, bricks, x, -1, z, chunk_bb);
                piece.fill_column_down(level, bricks, x, -1, 18 - z, chunk_bb);
            }
        }
        piece.generate_box(level, chunk_bb, 0, 2, 7, 5, 2, 11, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 13, 2, 7, 18, 2, 11, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 0, 0, 7, 3, 1, 11, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 15, 0, 7, 18, 1, 11, bricks, bricks, false);
        for x in 0..=2 {
            for z in 7..=11 {
                piece.fill_column_down(level, bricks, x, -1, z, chunk_bb);
                piece.fill_column_down(level, bricks, 18 - x, -1, z, chunk_bb);
            }
        }
    }
}

#[derive(Clone)]
pub struct BridgeEndFiller {
    pub nether_bridge: NetherBridgePiece,
    self_seed: i32,
}

impl BridgeEndFiller {
    fn new(gen_depth: i32, random: &mut impl RandomSource, bounding_box: BoundingBox, direction: Direction) -> BridgeEndFiller {
        let nether_bridge = NetherBridgePiece::oriented(NETHER_FORTRESS_BRIDGE_END_FILLER, gen_depth, bounding_box, direction);
        BridgeEndFiller { nether_bridge, self_seed: random.next_int() }
    }

    fn load(tag: &Nbt) -> Option<BridgeEndFiller> {
        Some(BridgeEndFiller { nether_bridge: NetherBridgePiece::load(NETHER_FORTRESS_BRIDGE_END_FILLER, tag)?, self_seed: int(tag, "Seed") })
    }

    #[allow(clippy::too_many_arguments)]
    fn create_piece(
        builder: &StructurePiecesBuilder,
        random: &mut impl RandomSource,
        foot_x: i32,
        foot_y: i32,
        foot_z: i32,
        direction: Direction,
        gen_depth: i32,
    ) -> Option<BridgeEndFiller> {
        let bounding_box = BoundingBox::orient_box(foot_x, foot_y, foot_z, -1, -3, 0, 5, 10, 8, direction);
        is_free_box(builder, &bounding_box).then(|| BridgeEndFiller::new(gen_depth, random, bounding_box, direction))
    }

    fn add_additional_save_data(&self, tag: &mut Vec<(String, Nbt)>) {
        tag.push(("Seed".into(), Nbt::Int(self.self_seed)));
    }

    fn post_process(&self, level: &mut impl WorldGenLevel, chunk_bb: &BoundingBox) {
        let piece = &self.nether_bridge.piece;
        let bricks = nether_bricks();
        let mut self_random = LegacyRandomSource::new(self.self_seed as i64);
        for x in 0..=4 {
            for y in 3..=4 {
                let z = self_random.next_int_bounded(8);
                piece.generate_box(level, chunk_bb, x, y, 0, x, y, z, bricks, bricks, false);
            }
        }
        let z = self_random.next_int_bounded(8);
        piece.generate_box(level, chunk_bb, 0, 5, 0, 0, 5, z, bricks, bricks, false);
        let z = self_random.next_int_bounded(8);
        piece.generate_box(level, chunk_bb, 4, 5, 0, 4, 5, z, bricks, bricks, false);
        for x in 0..=4 {
            let z = self_random.next_int_bounded(5);
            piece.generate_box(level, chunk_bb, x, 2, 0, x, 2, z, bricks, bricks, false);
        }
        for x in 0..=4 {
            for y in 0..=1 {
                let z = self_random.next_int_bounded(3);
                piece.generate_box(level, chunk_bb, x, y, 0, x, y, z, bricks, bricks, false);
            }
        }
    }
}

#[derive(Clone)]
pub struct BridgeStraight {
    pub nether_bridge: NetherBridgePiece,
}

impl BridgeStraight {
    fn new(gen_depth: i32, _random: &mut impl RandomSource, bounding_box: BoundingBox, direction: Direction) -> BridgeStraight {
        BridgeStraight { nether_bridge: NetherBridgePiece::oriented(NETHER_FORTRESS_BRIDGE_STRAIGHT, gen_depth, bounding_box, direction) }
    }

    fn load(tag: &Nbt) -> Option<BridgeStraight> {
        Some(BridgeStraight { nether_bridge: NetherBridgePiece::load(NETHER_FORTRESS_BRIDGE_STRAIGHT, tag)? })
    }

    fn add_children(&self, start_piece: &mut StartPiece, builder: &mut StructurePiecesBuilder, random: &mut impl RandomSource) {
        self.nether_bridge.generate_child_forward(start_piece, builder, random, 1, 3, false);
    }

    #[allow(clippy::too_many_arguments)]
    fn create_piece(
        builder: &StructurePiecesBuilder,
        random: &mut impl RandomSource,
        foot_x: i32,
        foot_y: i32,
        foot_z: i32,
        direction: Direction,
        gen_depth: i32,
    ) -> Option<BridgeStraight> {
        let bounding_box = BoundingBox::orient_box(foot_x, foot_y, foot_z, -1, -3, 0, 5, 10, 19, direction);
        is_free_box(builder, &bounding_box).then(|| BridgeStraight::new(gen_depth, random, bounding_box, direction))
    }

    fn post_process(&self, level: &mut impl WorldGenLevel, chunk_bb: &BoundingBox) {
        let piece = &self.nether_bridge.piece;
        let bricks = nether_bricks();
        piece.generate_box(level, chunk_bb, 0, 3, 0, 4, 4, 18, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 1, 5, 0, 3, 7, 18, air(), air(), false);
        piece.generate_box(level, chunk_bb, 0, 5, 0, 0, 5, 18, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 4, 5, 0, 4, 5, 18, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 0, 2, 0, 4, 2, 5, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 0, 2, 13, 4, 2, 18, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 0, 0, 0, 4, 1, 3, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 0, 0, 15, 4, 1, 18, bricks, bricks, false);
        for x in 0..=4 {
            for z in 0..=2 {
                piece.fill_column_down(level, bricks, x, -1, z, chunk_bb);
                piece.fill_column_down(level, bricks, x, -1, 18 - z, chunk_bb);
            }
        }
        let ns_fence = ns_fence();
        let nse_fence = ns_fence.set(EAST, true);
        let nsw_fence = ns_fence.set(WEST, true);
        piece.generate_box(level, chunk_bb, 0, 1, 1, 0, 4, 1, nse_fence, nse_fence, false);
        piece.generate_box(level, chunk_bb, 0, 3, 4, 0, 4, 4, nse_fence, nse_fence, false);
        piece.generate_box(level, chunk_bb, 0, 3, 14, 0, 4, 14, nse_fence, nse_fence, false);
        piece.generate_box(level, chunk_bb, 0, 1, 17, 0, 4, 17, nse_fence, nse_fence, false);
        piece.generate_box(level, chunk_bb, 4, 1, 1, 4, 4, 1, nsw_fence, nsw_fence, false);
        piece.generate_box(level, chunk_bb, 4, 3, 4, 4, 4, 4, nsw_fence, nsw_fence, false);
        piece.generate_box(level, chunk_bb, 4, 3, 14, 4, 4, 14, nsw_fence, nsw_fence, false);
        piece.generate_box(level, chunk_bb, 4, 1, 17, 4, 4, 17, nsw_fence, nsw_fence, false);
    }
}

#[derive(Clone)]
pub struct CastleCorridorStairsPiece {
    pub nether_bridge: NetherBridgePiece,
}

impl CastleCorridorStairsPiece {
    fn new(gen_depth: i32, bounding_box: BoundingBox, direction: Direction) -> CastleCorridorStairsPiece {
        CastleCorridorStairsPiece { nether_bridge: NetherBridgePiece::oriented(NETHER_FORTRESS_CASTLE_CORRIDOR_STAIRS, gen_depth, bounding_box, direction) }
    }

    fn load(tag: &Nbt) -> Option<CastleCorridorStairsPiece> {
        Some(CastleCorridorStairsPiece { nether_bridge: NetherBridgePiece::load(NETHER_FORTRESS_CASTLE_CORRIDOR_STAIRS, tag)? })
    }

    fn add_children(&self, start_piece: &mut StartPiece, builder: &mut StructurePiecesBuilder, random: &mut impl RandomSource) {
        self.nether_bridge.generate_child_forward(start_piece, builder, random, 1, 0, true);
    }

    fn create_piece(builder: &StructurePiecesBuilder, foot_x: i32, foot_y: i32, foot_z: i32, direction: Direction, gen_depth: i32) -> Option<CastleCorridorStairsPiece> {
        let bounding_box = BoundingBox::orient_box(foot_x, foot_y, foot_z, -1, -7, 0, 5, 14, 10, direction);
        is_free_box(builder, &bounding_box).then(|| CastleCorridorStairsPiece::new(gen_depth, bounding_box, direction))
    }

    fn post_process(&self, level: &mut impl WorldGenLevel, chunk_bb: &BoundingBox) {
        let piece = &self.nether_bridge.piece;
        let bricks = nether_bricks();
        let stairs = blocks::NETHER_BRICK_STAIRS.default_state().set(HORIZONTAL_FACING, Direction::South);
        let ns_fence = ns_fence();
        for step in 0..=9 {
            let floor = 1.max(7 - step);
            let roof = (floor + 5).max(14 - step).min(13);
            let z = step;
            piece.generate_box(level, chunk_bb, 0, 0, z, 4, floor, z, bricks, bricks, false);
            piece.generate_box(level, chunk_bb, 1, floor + 1, z, 3, roof - 1, z, air(), air(), false);
            if step <= 6 {
                piece.place_block(level, stairs, 1, floor + 1, z, chunk_bb);
                piece.place_block(level, stairs, 2, floor + 1, z, chunk_bb);
                piece.place_block(level, stairs, 3, floor + 1, z, chunk_bb);
            }
            piece.generate_box(level, chunk_bb, 0, roof, z, 4, roof, z, bricks, bricks, false);
            piece.generate_box(level, chunk_bb, 0, floor + 1, z, 0, roof - 1, z, bricks, bricks, false);
            piece.generate_box(level, chunk_bb, 4, floor + 1, z, 4, roof - 1, z, bricks, bricks, false);
            if (step & 1) == 0 {
                piece.generate_box(level, chunk_bb, 0, floor + 2, z, 0, floor + 3, z, ns_fence, ns_fence, false);
                piece.generate_box(level, chunk_bb, 4, floor + 2, z, 4, floor + 3, z, ns_fence, ns_fence, false);
            }
            for x in 0..=4 {
                piece.fill_column_down(level, bricks, x, -1, z, chunk_bb);
            }
        }
    }
}

#[derive(Clone)]
pub struct CastleCorridorTBalconyPiece {
    pub nether_bridge: NetherBridgePiece,
}

impl CastleCorridorTBalconyPiece {
    fn new(gen_depth: i32, bounding_box: BoundingBox, direction: Direction) -> CastleCorridorTBalconyPiece {
        CastleCorridorTBalconyPiece { nether_bridge: NetherBridgePiece::oriented(NETHER_FORTRESS_CASTLE_CORRIDOR_T_BALCONY, gen_depth, bounding_box, direction) }
    }

    fn load(tag: &Nbt) -> Option<CastleCorridorTBalconyPiece> {
        Some(CastleCorridorTBalconyPiece { nether_bridge: NetherBridgePiece::load(NETHER_FORTRESS_CASTLE_CORRIDOR_T_BALCONY, tag)? })
    }

    fn add_children(&self, start_piece: &mut StartPiece, builder: &mut StructurePiecesBuilder, random: &mut impl RandomSource) {
        let mut z_off = 1;
        let orientation = self.nether_bridge.piece.get_orientation();
        if orientation == Some(Direction::West) || orientation == Some(Direction::North) {
            z_off = 5;
        }
        let is_castle = random.next_int_bounded(8) > 0;
        self.nether_bridge.generate_child_left(start_piece, builder, random, 0, z_off, is_castle);
        let is_castle = random.next_int_bounded(8) > 0;
        self.nether_bridge.generate_child_right(start_piece, builder, random, 0, z_off, is_castle);
    }

    fn create_piece(builder: &StructurePiecesBuilder, foot_x: i32, foot_y: i32, foot_z: i32, direction: Direction, gen_depth: i32) -> Option<CastleCorridorTBalconyPiece> {
        let bounding_box = BoundingBox::orient_box(foot_x, foot_y, foot_z, -3, 0, 0, 9, 7, 9, direction);
        is_free_box(builder, &bounding_box).then(|| CastleCorridorTBalconyPiece::new(gen_depth, bounding_box, direction))
    }

    fn post_process(&self, level: &mut impl WorldGenLevel, chunk_bb: &BoundingBox) {
        let piece = &self.nether_bridge.piece;
        let bricks = nether_bricks();
        let ns_fence = ns_fence();
        let we_fence = we_fence();
        piece.generate_box(level, chunk_bb, 0, 0, 0, 8, 1, 8, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 0, 2, 0, 8, 5, 8, air(), air(), false);
        piece.generate_box(level, chunk_bb, 0, 6, 0, 8, 6, 5, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 0, 2, 0, 2, 5, 0, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 6, 2, 0, 8, 5, 0, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 1, 3, 0, 1, 4, 0, we_fence, we_fence, false);
        piece.generate_box(level, chunk_bb, 7, 3, 0, 7, 4, 0, we_fence, we_fence, false);
        piece.generate_box(level, chunk_bb, 0, 2, 4, 8, 2, 8, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 1, 1, 4, 2, 2, 4, air(), air(), false);
        piece.generate_box(level, chunk_bb, 6, 1, 4, 7, 2, 4, air(), air(), false);
        piece.generate_box(level, chunk_bb, 1, 3, 8, 7, 3, 8, we_fence, we_fence, false);
        piece.place_block(level, fence().set(EAST, true).set(SOUTH, true), 0, 3, 8, chunk_bb);
        piece.place_block(level, fence().set(WEST, true).set(SOUTH, true), 8, 3, 8, chunk_bb);
        piece.generate_box(level, chunk_bb, 0, 3, 6, 0, 3, 7, ns_fence, ns_fence, false);
        piece.generate_box(level, chunk_bb, 8, 3, 6, 8, 3, 7, ns_fence, ns_fence, false);
        piece.generate_box(level, chunk_bb, 0, 3, 4, 0, 5, 5, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 8, 3, 4, 8, 5, 5, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 1, 3, 5, 2, 5, 5, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 6, 3, 5, 7, 5, 5, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 1, 4, 5, 1, 5, 5, we_fence, we_fence, false);
        piece.generate_box(level, chunk_bb, 7, 4, 5, 7, 5, 5, we_fence, we_fence, false);
        for z in 0..=5 {
            for x in 0..=8 {
                piece.fill_column_down(level, bricks, x, -1, z, chunk_bb);
            }
        }
    }
}

#[derive(Clone)]
pub struct CastleEntrance {
    pub nether_bridge: NetherBridgePiece,
}

impl CastleEntrance {
    fn new(gen_depth: i32, _random: &mut impl RandomSource, bounding_box: BoundingBox, direction: Direction) -> CastleEntrance {
        CastleEntrance { nether_bridge: NetherBridgePiece::oriented(NETHER_FORTRESS_CASTLE_ENTRANCE, gen_depth, bounding_box, direction) }
    }

    fn load(tag: &Nbt) -> Option<CastleEntrance> {
        Some(CastleEntrance { nether_bridge: NetherBridgePiece::load(NETHER_FORTRESS_CASTLE_ENTRANCE, tag)? })
    }

    fn add_children(&self, start_piece: &mut StartPiece, builder: &mut StructurePiecesBuilder, random: &mut impl RandomSource) {
        self.nether_bridge.generate_child_forward(start_piece, builder, random, 5, 3, true);
    }

    #[allow(clippy::too_many_arguments)]
    fn create_piece(
        builder: &StructurePiecesBuilder,
        random: &mut impl RandomSource,
        foot_x: i32,
        foot_y: i32,
        foot_z: i32,
        direction: Direction,
        gen_depth: i32,
    ) -> Option<CastleEntrance> {
        let bounding_box = BoundingBox::orient_box(foot_x, foot_y, foot_z, -5, -3, 0, 13, 14, 13, direction);
        is_free_box(builder, &bounding_box).then(|| CastleEntrance::new(gen_depth, random, bounding_box, direction))
    }

    fn post_process(&self, level: &mut impl WorldGenLevel, chunk_bb: &BoundingBox) {
        let piece = &self.nether_bridge.piece;
        let bricks = nether_bricks();
        piece.generate_box(level, chunk_bb, 0, 3, 0, 12, 4, 12, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 0, 5, 0, 12, 13, 12, air(), air(), false);
        piece.generate_box(level, chunk_bb, 0, 5, 0, 1, 12, 12, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 11, 5, 0, 12, 12, 12, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 2, 5, 11, 4, 12, 12, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 8, 5, 11, 10, 12, 12, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 5, 9, 11, 7, 12, 12, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 2, 5, 0, 4, 12, 1, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 8, 5, 0, 10, 12, 1, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 5, 9, 0, 7, 12, 1, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 2, 11, 2, 10, 12, 10, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 5, 8, 0, 7, 8, 0, fence(), fence(), false);
        let we_fence = we_fence();
        let ns_fence = ns_fence();
        for i in (1..=11).step_by(2) {
            piece.generate_box(level, chunk_bb, i, 10, 0, i, 11, 0, we_fence, we_fence, false);
            piece.generate_box(level, chunk_bb, i, 10, 12, i, 11, 12, we_fence, we_fence, false);
            piece.generate_box(level, chunk_bb, 0, 10, i, 0, 11, i, ns_fence, ns_fence, false);
            piece.generate_box(level, chunk_bb, 12, 10, i, 12, 11, i, ns_fence, ns_fence, false);
            piece.place_block(level, bricks, i, 13, 0, chunk_bb);
            piece.place_block(level, bricks, i, 13, 12, chunk_bb);
            piece.place_block(level, bricks, 0, 13, i, chunk_bb);
            piece.place_block(level, bricks, 12, 13, i, chunk_bb);
            if i != 11 {
                piece.place_block(level, we_fence, i + 1, 13, 0, chunk_bb);
                piece.place_block(level, we_fence, i + 1, 13, 12, chunk_bb);
                piece.place_block(level, ns_fence, 0, 13, i + 1, chunk_bb);
                piece.place_block(level, ns_fence, 12, 13, i + 1, chunk_bb);
            }
        }
        piece.place_block(level, fence().set(NORTH, true).set(EAST, true), 0, 13, 0, chunk_bb);
        piece.place_block(level, fence().set(SOUTH, true).set(EAST, true), 0, 13, 12, chunk_bb);
        piece.place_block(level, fence().set(SOUTH, true).set(WEST, true), 12, 13, 12, chunk_bb);
        piece.place_block(level, fence().set(NORTH, true).set(WEST, true), 12, 13, 0, chunk_bb);
        for z in (3..=9).step_by(2) {
            piece.generate_box(level, chunk_bb, 1, 7, z, 1, 8, z, ns_fence.set(WEST, true), ns_fence.set(WEST, true), false);
            piece.generate_box(level, chunk_bb, 11, 7, z, 11, 8, z, ns_fence.set(EAST, true), ns_fence.set(EAST, true), false);
        }
        piece.generate_box(level, chunk_bb, 4, 2, 0, 8, 2, 12, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 0, 2, 4, 12, 2, 8, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 4, 0, 0, 8, 1, 3, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 4, 0, 9, 8, 1, 12, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 0, 0, 4, 3, 1, 8, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 9, 0, 4, 12, 1, 8, bricks, bricks, false);
        for x in 4..=8 {
            for z in 0..=2 {
                piece.fill_column_down(level, bricks, x, -1, z, chunk_bb);
                piece.fill_column_down(level, bricks, x, -1, 12 - z, chunk_bb);
            }
        }
        for x in 0..=2 {
            for z in 4..=8 {
                piece.fill_column_down(level, bricks, x, -1, z, chunk_bb);
                piece.fill_column_down(level, bricks, 12 - x, -1, z, chunk_bb);
            }
        }
        piece.generate_box(level, chunk_bb, 5, 5, 5, 7, 5, 7, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 6, 1, 6, 6, 4, 6, air(), air(), false);
        piece.place_block(level, bricks, 6, 0, 6, chunk_bb);
        piece.place_block(level, blocks::LAVA.default_state(), 6, 5, 6, chunk_bb);
        let pos = piece.get_world_pos(6, 5, 6);
        if chunk_bb.is_inside(pos) {
            level.schedule_fluid_tick(pos, Fluid::Lava, 0);
        }
    }
}

#[derive(Clone)]
pub struct CastleSmallCorridorCrossingPiece {
    pub nether_bridge: NetherBridgePiece,
}

impl CastleSmallCorridorCrossingPiece {
    fn new(gen_depth: i32, bounding_box: BoundingBox, direction: Direction) -> CastleSmallCorridorCrossingPiece {
        CastleSmallCorridorCrossingPiece { nether_bridge: NetherBridgePiece::oriented(NETHER_FORTRESS_CASTLE_SMALL_CORRIDOR_CROSSING, gen_depth, bounding_box, direction) }
    }

    fn load(tag: &Nbt) -> Option<CastleSmallCorridorCrossingPiece> {
        Some(CastleSmallCorridorCrossingPiece { nether_bridge: NetherBridgePiece::load(NETHER_FORTRESS_CASTLE_SMALL_CORRIDOR_CROSSING, tag)? })
    }

    fn add_children(&self, start_piece: &mut StartPiece, builder: &mut StructurePiecesBuilder, random: &mut impl RandomSource) {
        self.nether_bridge.generate_child_forward(start_piece, builder, random, 1, 0, true);
        self.nether_bridge.generate_child_left(start_piece, builder, random, 0, 1, true);
        self.nether_bridge.generate_child_right(start_piece, builder, random, 0, 1, true);
    }

    fn create_piece(builder: &StructurePiecesBuilder, foot_x: i32, foot_y: i32, foot_z: i32, direction: Direction, gen_depth: i32) -> Option<CastleSmallCorridorCrossingPiece> {
        let bounding_box = BoundingBox::orient_box(foot_x, foot_y, foot_z, -1, 0, 0, 5, 7, 5, direction);
        is_free_box(builder, &bounding_box).then(|| CastleSmallCorridorCrossingPiece::new(gen_depth, bounding_box, direction))
    }

    fn post_process(&self, level: &mut impl WorldGenLevel, chunk_bb: &BoundingBox) {
        let piece = &self.nether_bridge.piece;
        let bricks = nether_bricks();
        piece.generate_box(level, chunk_bb, 0, 0, 0, 4, 1, 4, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 0, 2, 0, 4, 5, 4, air(), air(), false);
        piece.generate_box(level, chunk_bb, 0, 2, 0, 0, 5, 0, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 4, 2, 0, 4, 5, 0, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 0, 2, 4, 0, 5, 4, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 4, 2, 4, 4, 5, 4, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 0, 6, 0, 4, 6, 4, bricks, bricks, false);
        for x in 0..=4 {
            for z in 0..=4 {
                piece.fill_column_down(level, bricks, x, -1, z, chunk_bb);
            }
        }
    }
}

#[derive(Clone)]
pub struct CastleSmallCorridorLeftTurnPiece {
    pub nether_bridge: NetherBridgePiece,
    is_needing_chest: bool,
}

impl CastleSmallCorridorLeftTurnPiece {
    fn new(gen_depth: i32, random: &mut impl RandomSource, bounding_box: BoundingBox, direction: Direction) -> CastleSmallCorridorLeftTurnPiece {
        let nether_bridge = NetherBridgePiece::oriented(NETHER_FORTRESS_CASTLE_SMALL_CORRIDOR_LEFT_TURN, gen_depth, bounding_box, direction);
        CastleSmallCorridorLeftTurnPiece { nether_bridge, is_needing_chest: random.next_int_bounded(3) == 0 }
    }

    fn load(tag: &Nbt) -> Option<CastleSmallCorridorLeftTurnPiece> {
        Some(CastleSmallCorridorLeftTurnPiece {
            nether_bridge: NetherBridgePiece::load(NETHER_FORTRESS_CASTLE_SMALL_CORRIDOR_LEFT_TURN, tag)?,
            is_needing_chest: boolean(tag, "Chest"),
        })
    }

    fn add_additional_save_data(&self, tag: &mut Vec<(String, Nbt)>) {
        tag.push(("Chest".into(), Nbt::Byte(self.is_needing_chest as i8)));
    }

    fn add_children(&self, start_piece: &mut StartPiece, builder: &mut StructurePiecesBuilder, random: &mut impl RandomSource) {
        self.nether_bridge.generate_child_left(start_piece, builder, random, 0, 1, true);
    }

    #[allow(clippy::too_many_arguments)]
    fn create_piece(
        builder: &StructurePiecesBuilder,
        random: &mut impl RandomSource,
        foot_x: i32,
        foot_y: i32,
        foot_z: i32,
        direction: Direction,
        gen_depth: i32,
    ) -> Option<CastleSmallCorridorLeftTurnPiece> {
        let bounding_box = BoundingBox::orient_box(foot_x, foot_y, foot_z, -1, 0, 0, 5, 7, 5, direction);
        is_free_box(builder, &bounding_box).then(|| CastleSmallCorridorLeftTurnPiece::new(gen_depth, random, bounding_box, direction))
    }

    fn post_process(&mut self, level: &mut impl WorldGenLevel, random: &mut impl RandomSource, chunk_bb: &BoundingBox) {
        let piece = &self.nether_bridge.piece;
        let bricks = nether_bricks();
        piece.generate_box(level, chunk_bb, 0, 0, 0, 4, 1, 4, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 0, 2, 0, 4, 5, 4, air(), air(), false);
        let we_fence = we_fence();
        let ns_fence = ns_fence();
        piece.generate_box(level, chunk_bb, 4, 2, 0, 4, 5, 4, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 4, 3, 1, 4, 4, 1, ns_fence, ns_fence, false);
        piece.generate_box(level, chunk_bb, 4, 3, 3, 4, 4, 3, ns_fence, ns_fence, false);
        piece.generate_box(level, chunk_bb, 0, 2, 0, 0, 5, 0, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 0, 2, 4, 3, 5, 4, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 1, 3, 4, 1, 4, 4, we_fence, we_fence, false);
        piece.generate_box(level, chunk_bb, 3, 3, 4, 3, 4, 4, we_fence, we_fence, false);
        if self.is_needing_chest && chunk_bb.is_inside(piece.get_world_pos(3, 2, 3)) {
            self.is_needing_chest = false;
            piece.create_chest(level, chunk_bb, random, piece.get_world_pos(3, 2, 3), NETHER_BRIDGE_LOOT, None);
        }
        piece.generate_box(level, chunk_bb, 0, 6, 0, 4, 6, 4, bricks, bricks, false);
        for x in 0..=4 {
            for z in 0..=4 {
                piece.fill_column_down(level, bricks, x, -1, z, chunk_bb);
            }
        }
    }
}

#[derive(Clone)]
pub struct CastleSmallCorridorPiece {
    pub nether_bridge: NetherBridgePiece,
}

impl CastleSmallCorridorPiece {
    fn new(gen_depth: i32, bounding_box: BoundingBox, direction: Direction) -> CastleSmallCorridorPiece {
        CastleSmallCorridorPiece { nether_bridge: NetherBridgePiece::oriented(NETHER_FORTRESS_CASTLE_SMALL_CORRIDOR, gen_depth, bounding_box, direction) }
    }

    fn load(tag: &Nbt) -> Option<CastleSmallCorridorPiece> {
        Some(CastleSmallCorridorPiece { nether_bridge: NetherBridgePiece::load(NETHER_FORTRESS_CASTLE_SMALL_CORRIDOR, tag)? })
    }

    fn add_children(&self, start_piece: &mut StartPiece, builder: &mut StructurePiecesBuilder, random: &mut impl RandomSource) {
        self.nether_bridge.generate_child_forward(start_piece, builder, random, 1, 0, true);
    }

    fn create_piece(builder: &StructurePiecesBuilder, foot_x: i32, foot_y: i32, foot_z: i32, direction: Direction, gen_depth: i32) -> Option<CastleSmallCorridorPiece> {
        let bounding_box = BoundingBox::orient_box(foot_x, foot_y, foot_z, -1, 0, 0, 5, 7, 5, direction);
        is_free_box(builder, &bounding_box).then(|| CastleSmallCorridorPiece::new(gen_depth, bounding_box, direction))
    }

    fn post_process(&self, level: &mut impl WorldGenLevel, chunk_bb: &BoundingBox) {
        let piece = &self.nether_bridge.piece;
        let bricks = nether_bricks();
        piece.generate_box(level, chunk_bb, 0, 0, 0, 4, 1, 4, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 0, 2, 0, 4, 5, 4, air(), air(), false);
        let ns_fence = ns_fence();
        piece.generate_box(level, chunk_bb, 0, 2, 0, 0, 5, 4, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 4, 2, 0, 4, 5, 4, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 0, 3, 1, 0, 4, 1, ns_fence, ns_fence, false);
        piece.generate_box(level, chunk_bb, 0, 3, 3, 0, 4, 3, ns_fence, ns_fence, false);
        piece.generate_box(level, chunk_bb, 4, 3, 1, 4, 4, 1, ns_fence, ns_fence, false);
        piece.generate_box(level, chunk_bb, 4, 3, 3, 4, 4, 3, ns_fence, ns_fence, false);
        piece.generate_box(level, chunk_bb, 0, 6, 0, 4, 6, 4, bricks, bricks, false);
        for x in 0..=4 {
            for z in 0..=4 {
                piece.fill_column_down(level, bricks, x, -1, z, chunk_bb);
            }
        }
    }
}

#[derive(Clone)]
pub struct CastleSmallCorridorRightTurnPiece {
    pub nether_bridge: NetherBridgePiece,
    is_needing_chest: bool,
}

impl CastleSmallCorridorRightTurnPiece {
    fn new(gen_depth: i32, random: &mut impl RandomSource, bounding_box: BoundingBox, direction: Direction) -> CastleSmallCorridorRightTurnPiece {
        let nether_bridge = NetherBridgePiece::oriented(NETHER_FORTRESS_CASTLE_SMALL_CORRIDOR_RIGHT_TURN, gen_depth, bounding_box, direction);
        CastleSmallCorridorRightTurnPiece { nether_bridge, is_needing_chest: random.next_int_bounded(3) == 0 }
    }

    fn load(tag: &Nbt) -> Option<CastleSmallCorridorRightTurnPiece> {
        Some(CastleSmallCorridorRightTurnPiece {
            nether_bridge: NetherBridgePiece::load(NETHER_FORTRESS_CASTLE_SMALL_CORRIDOR_RIGHT_TURN, tag)?,
            is_needing_chest: boolean(tag, "Chest"),
        })
    }

    fn add_additional_save_data(&self, tag: &mut Vec<(String, Nbt)>) {
        tag.push(("Chest".into(), Nbt::Byte(self.is_needing_chest as i8)));
    }

    fn add_children(&self, start_piece: &mut StartPiece, builder: &mut StructurePiecesBuilder, random: &mut impl RandomSource) {
        self.nether_bridge.generate_child_right(start_piece, builder, random, 0, 1, true);
    }

    #[allow(clippy::too_many_arguments)]
    fn create_piece(
        builder: &StructurePiecesBuilder,
        random: &mut impl RandomSource,
        foot_x: i32,
        foot_y: i32,
        foot_z: i32,
        direction: Direction,
        gen_depth: i32,
    ) -> Option<CastleSmallCorridorRightTurnPiece> {
        let bounding_box = BoundingBox::orient_box(foot_x, foot_y, foot_z, -1, 0, 0, 5, 7, 5, direction);
        is_free_box(builder, &bounding_box).then(|| CastleSmallCorridorRightTurnPiece::new(gen_depth, random, bounding_box, direction))
    }

    fn post_process(&mut self, level: &mut impl WorldGenLevel, random: &mut impl RandomSource, chunk_bb: &BoundingBox) {
        let piece = &self.nether_bridge.piece;
        let bricks = nether_bricks();
        piece.generate_box(level, chunk_bb, 0, 0, 0, 4, 1, 4, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 0, 2, 0, 4, 5, 4, air(), air(), false);
        let we_fence = we_fence();
        let ns_fence = ns_fence();
        piece.generate_box(level, chunk_bb, 0, 2, 0, 0, 5, 4, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 0, 3, 1, 0, 4, 1, ns_fence, ns_fence, false);
        piece.generate_box(level, chunk_bb, 0, 3, 3, 0, 4, 3, ns_fence, ns_fence, false);
        piece.generate_box(level, chunk_bb, 4, 2, 0, 4, 5, 0, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 1, 2, 4, 4, 5, 4, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 1, 3, 4, 1, 4, 4, we_fence, we_fence, false);
        piece.generate_box(level, chunk_bb, 3, 3, 4, 3, 4, 4, we_fence, we_fence, false);
        if self.is_needing_chest && chunk_bb.is_inside(piece.get_world_pos(1, 2, 3)) {
            self.is_needing_chest = false;
            piece.create_chest(level, chunk_bb, random, piece.get_world_pos(1, 2, 3), NETHER_BRIDGE_LOOT, None);
        }
        piece.generate_box(level, chunk_bb, 0, 6, 0, 4, 6, 4, bricks, bricks, false);
        for x in 0..=4 {
            for z in 0..=4 {
                piece.fill_column_down(level, bricks, x, -1, z, chunk_bb);
            }
        }
    }
}

#[derive(Clone)]
pub struct CastleStalkRoom {
    pub nether_bridge: NetherBridgePiece,
}

impl CastleStalkRoom {
    fn new(gen_depth: i32, bounding_box: BoundingBox, direction: Direction) -> CastleStalkRoom {
        CastleStalkRoom { nether_bridge: NetherBridgePiece::oriented(NETHER_FORTRESS_CASTLE_STALK_ROOM, gen_depth, bounding_box, direction) }
    }

    fn load(tag: &Nbt) -> Option<CastleStalkRoom> {
        Some(CastleStalkRoom { nether_bridge: NetherBridgePiece::load(NETHER_FORTRESS_CASTLE_STALK_ROOM, tag)? })
    }

    fn add_children(&self, start_piece: &mut StartPiece, builder: &mut StructurePiecesBuilder, random: &mut impl RandomSource) {
        self.nether_bridge.generate_child_forward(start_piece, builder, random, 5, 3, true);
        self.nether_bridge.generate_child_forward(start_piece, builder, random, 5, 11, true);
    }

    fn create_piece(builder: &StructurePiecesBuilder, foot_x: i32, foot_y: i32, foot_z: i32, direction: Direction, gen_depth: i32) -> Option<CastleStalkRoom> {
        let bounding_box = BoundingBox::orient_box(foot_x, foot_y, foot_z, -5, -3, 0, 13, 14, 13, direction);
        is_free_box(builder, &bounding_box).then(|| CastleStalkRoom::new(gen_depth, bounding_box, direction))
    }

    fn post_process(&self, level: &mut impl WorldGenLevel, chunk_bb: &BoundingBox) {
        let piece = &self.nether_bridge.piece;
        let bricks = nether_bricks();
        piece.generate_box(level, chunk_bb, 0, 3, 0, 12, 4, 12, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 0, 5, 0, 12, 13, 12, air(), air(), false);
        piece.generate_box(level, chunk_bb, 0, 5, 0, 1, 12, 12, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 11, 5, 0, 12, 12, 12, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 2, 5, 11, 4, 12, 12, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 8, 5, 11, 10, 12, 12, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 5, 9, 11, 7, 12, 12, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 2, 5, 0, 4, 12, 1, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 8, 5, 0, 10, 12, 1, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 5, 9, 0, 7, 12, 1, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 2, 11, 2, 10, 12, 10, bricks, bricks, false);
        let we_fence = we_fence();
        let ns_fence = ns_fence();
        let nsw_fence = ns_fence.set(WEST, true);
        let nse_fence = ns_fence.set(EAST, true);
        for i in (1..=11).step_by(2) {
            piece.generate_box(level, chunk_bb, i, 10, 0, i, 11, 0, we_fence, we_fence, false);
            piece.generate_box(level, chunk_bb, i, 10, 12, i, 11, 12, we_fence, we_fence, false);
            piece.generate_box(level, chunk_bb, 0, 10, i, 0, 11, i, ns_fence, ns_fence, false);
            piece.generate_box(level, chunk_bb, 12, 10, i, 12, 11, i, ns_fence, ns_fence, false);
            piece.place_block(level, bricks, i, 13, 0, chunk_bb);
            piece.place_block(level, bricks, i, 13, 12, chunk_bb);
            piece.place_block(level, bricks, 0, 13, i, chunk_bb);
            piece.place_block(level, bricks, 12, 13, i, chunk_bb);
            if i != 11 {
                piece.place_block(level, we_fence, i + 1, 13, 0, chunk_bb);
                piece.place_block(level, we_fence, i + 1, 13, 12, chunk_bb);
                piece.place_block(level, ns_fence, 0, 13, i + 1, chunk_bb);
                piece.place_block(level, ns_fence, 12, 13, i + 1, chunk_bb);
            }
        }
        piece.place_block(level, fence().set(NORTH, true).set(EAST, true), 0, 13, 0, chunk_bb);
        piece.place_block(level, fence().set(SOUTH, true).set(EAST, true), 0, 13, 12, chunk_bb);
        piece.place_block(level, fence().set(SOUTH, true).set(WEST, true), 12, 13, 12, chunk_bb);
        piece.place_block(level, fence().set(NORTH, true).set(WEST, true), 12, 13, 0, chunk_bb);
        for z in (3..=9).step_by(2) {
            piece.generate_box(level, chunk_bb, 1, 7, z, 1, 8, z, nsw_fence, nsw_fence, false);
            piece.generate_box(level, chunk_bb, 11, 7, z, 11, 8, z, nse_fence, nse_fence, false);
        }
        let stairs = blocks::NETHER_BRICK_STAIRS.default_state().set(HORIZONTAL_FACING, Direction::North);
        for i in 0..=6 {
            let z = i + 4;
            for x in 5..=7 {
                piece.place_block(level, stairs, x, 5 + i, z, chunk_bb);
            }
            if (5..=8).contains(&z) {
                piece.generate_box(level, chunk_bb, 5, 5, z, 7, i + 4, z, bricks, bricks, false);
            } else if (9..=10).contains(&z) {
                piece.generate_box(level, chunk_bb, 5, 8, z, 7, i + 4, z, bricks, bricks, false);
            }
            if i >= 1 {
                piece.generate_box(level, chunk_bb, 5, 6 + i, z, 7, 9 + i, z, air(), air(), false);
            }
        }
        for x in 5..=7 {
            piece.place_block(level, stairs, x, 12, 11, chunk_bb);
        }
        piece.generate_box(level, chunk_bb, 5, 6, 7, 5, 7, 7, nse_fence, nse_fence, false);
        piece.generate_box(level, chunk_bb, 7, 6, 7, 7, 7, 7, nsw_fence, nsw_fence, false);
        piece.generate_box(level, chunk_bb, 5, 13, 12, 7, 13, 12, air(), air(), false);
        piece.generate_box(level, chunk_bb, 2, 5, 2, 3, 5, 3, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 2, 5, 9, 3, 5, 10, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 2, 5, 4, 2, 5, 8, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 9, 5, 2, 10, 5, 3, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 9, 5, 9, 10, 5, 10, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 10, 5, 4, 10, 5, 8, bricks, bricks, false);
        let east_stairs = stairs.set(HORIZONTAL_FACING, Direction::East);
        let west_stairs = stairs.set(HORIZONTAL_FACING, Direction::West);
        piece.place_block(level, west_stairs, 4, 5, 2, chunk_bb);
        piece.place_block(level, west_stairs, 4, 5, 3, chunk_bb);
        piece.place_block(level, west_stairs, 4, 5, 9, chunk_bb);
        piece.place_block(level, west_stairs, 4, 5, 10, chunk_bb);
        piece.place_block(level, east_stairs, 8, 5, 2, chunk_bb);
        piece.place_block(level, east_stairs, 8, 5, 3, chunk_bb);
        piece.place_block(level, east_stairs, 8, 5, 9, chunk_bb);
        piece.place_block(level, east_stairs, 8, 5, 10, chunk_bb);
        let soul_sand = blocks::SOUL_SAND.default_state();
        let nether_wart = blocks::NETHER_WART.default_state();
        piece.generate_box(level, chunk_bb, 3, 4, 4, 4, 4, 8, soul_sand, soul_sand, false);
        piece.generate_box(level, chunk_bb, 8, 4, 4, 9, 4, 8, soul_sand, soul_sand, false);
        piece.generate_box(level, chunk_bb, 3, 5, 4, 4, 5, 8, nether_wart, nether_wart, false);
        piece.generate_box(level, chunk_bb, 8, 5, 4, 9, 5, 8, nether_wart, nether_wart, false);
        piece.generate_box(level, chunk_bb, 4, 2, 0, 8, 2, 12, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 0, 2, 4, 12, 2, 8, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 4, 0, 0, 8, 1, 3, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 4, 0, 9, 8, 1, 12, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 0, 0, 4, 3, 1, 8, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 9, 0, 4, 12, 1, 8, bricks, bricks, false);
        for x in 4..=8 {
            for z in 0..=2 {
                piece.fill_column_down(level, bricks, x, -1, z, chunk_bb);
                piece.fill_column_down(level, bricks, x, -1, 12 - z, chunk_bb);
            }
        }
        for x in 0..=2 {
            for z in 4..=8 {
                piece.fill_column_down(level, bricks, x, -1, z, chunk_bb);
                piece.fill_column_down(level, bricks, 12 - x, -1, z, chunk_bb);
            }
        }
    }
}

#[derive(Clone)]
pub struct MonsterThrone {
    pub nether_bridge: NetherBridgePiece,
    has_placed_spawner: bool,
}

impl MonsterThrone {
    fn new(gen_depth: i32, bounding_box: BoundingBox, direction: Direction) -> MonsterThrone {
        MonsterThrone { nether_bridge: NetherBridgePiece::oriented(NETHER_FORTRESS_MONSTER_THRONE, gen_depth, bounding_box, direction), has_placed_spawner: false }
    }

    fn load(tag: &Nbt) -> Option<MonsterThrone> {
        Some(MonsterThrone { nether_bridge: NetherBridgePiece::load(NETHER_FORTRESS_MONSTER_THRONE, tag)?, has_placed_spawner: boolean(tag, "Mob") })
    }

    fn add_additional_save_data(&self, tag: &mut Vec<(String, Nbt)>) {
        tag.push(("Mob".into(), Nbt::Byte(self.has_placed_spawner as i8)));
    }

    fn create_piece(builder: &StructurePiecesBuilder, foot_x: i32, foot_y: i32, foot_z: i32, gen_depth: i32, direction: Direction) -> Option<MonsterThrone> {
        let bounding_box = BoundingBox::orient_box(foot_x, foot_y, foot_z, -2, 0, 0, 7, 8, 9, direction);
        is_free_box(builder, &bounding_box).then(|| MonsterThrone::new(gen_depth, bounding_box, direction))
    }

    fn post_process(&mut self, level: &mut impl WorldGenLevel, random: &mut impl RandomSource, chunk_bb: &BoundingBox) {
        let piece = &self.nether_bridge.piece;
        let bricks = nether_bricks();
        piece.generate_box(level, chunk_bb, 0, 2, 0, 6, 7, 7, air(), air(), false);
        piece.generate_box(level, chunk_bb, 1, 0, 0, 5, 1, 7, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 1, 2, 1, 5, 2, 7, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 1, 3, 2, 5, 3, 7, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 1, 4, 3, 5, 4, 7, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 1, 2, 0, 1, 4, 2, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 5, 2, 0, 5, 4, 2, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 1, 5, 2, 1, 5, 3, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 5, 5, 2, 5, 5, 3, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 0, 5, 3, 0, 5, 8, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 6, 5, 3, 6, 5, 8, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 1, 5, 8, 5, 5, 8, bricks, bricks, false);
        let we_fence = we_fence();
        let ns_fence = ns_fence();
        piece.place_block(level, fence().set(WEST, true), 1, 6, 3, chunk_bb);
        piece.place_block(level, fence().set(EAST, true), 5, 6, 3, chunk_bb);
        piece.place_block(level, fence().set(EAST, true).set(NORTH, true), 0, 6, 3, chunk_bb);
        piece.place_block(level, fence().set(WEST, true).set(NORTH, true), 6, 6, 3, chunk_bb);
        piece.generate_box(level, chunk_bb, 0, 6, 4, 0, 6, 7, ns_fence, ns_fence, false);
        piece.generate_box(level, chunk_bb, 6, 6, 4, 6, 6, 7, ns_fence, ns_fence, false);
        piece.place_block(level, fence().set(EAST, true).set(SOUTH, true), 0, 6, 8, chunk_bb);
        piece.place_block(level, fence().set(WEST, true).set(SOUTH, true), 6, 6, 8, chunk_bb);
        piece.generate_box(level, chunk_bb, 1, 6, 8, 5, 6, 8, we_fence, we_fence, false);
        piece.place_block(level, fence().set(EAST, true), 1, 7, 8, chunk_bb);
        piece.generate_box(level, chunk_bb, 2, 7, 8, 4, 7, 8, we_fence, we_fence, false);
        piece.place_block(level, fence().set(WEST, true), 5, 7, 8, chunk_bb);
        piece.place_block(level, fence().set(EAST, true), 2, 8, 8, chunk_bb);
        piece.place_block(level, we_fence, 3, 8, 8, chunk_bb);
        piece.place_block(level, fence().set(WEST, true), 4, 8, 8, chunk_bb);
        if !self.has_placed_spawner {
            let pos = piece.get_world_pos(3, 5, 5);
            if chunk_bb.is_inside(pos) {
                self.has_placed_spawner = true;
                level.set_block(pos, blocks::SPAWNER.default_state(), 2);
                if let Some(entity) = level.get_block_entity(pos)
                    && let BlockEntityKind::Spawner(spawner) = &mut entity.kind
                {
                    spawner.spawner.set_entity_id("minecraft:blaze", random);
                }
            }
        }
        for x in 0..=6 {
            for z in 0..=6 {
                piece.fill_column_down(level, bricks, x, -1, z, chunk_bb);
            }
        }
    }
}

#[derive(Clone)]
pub struct RoomCrossing {
    pub nether_bridge: NetherBridgePiece,
}

impl RoomCrossing {
    fn new(gen_depth: i32, bounding_box: BoundingBox, direction: Direction) -> RoomCrossing {
        RoomCrossing { nether_bridge: NetherBridgePiece::oriented(NETHER_FORTRESS_ROOM_CROSSING, gen_depth, bounding_box, direction) }
    }

    fn load(tag: &Nbt) -> Option<RoomCrossing> {
        Some(RoomCrossing { nether_bridge: NetherBridgePiece::load(NETHER_FORTRESS_ROOM_CROSSING, tag)? })
    }

    fn add_children(&self, start_piece: &mut StartPiece, builder: &mut StructurePiecesBuilder, random: &mut impl RandomSource) {
        self.nether_bridge.generate_child_forward(start_piece, builder, random, 2, 0, false);
        self.nether_bridge.generate_child_left(start_piece, builder, random, 0, 2, false);
        self.nether_bridge.generate_child_right(start_piece, builder, random, 0, 2, false);
    }

    fn create_piece(builder: &StructurePiecesBuilder, foot_x: i32, foot_y: i32, foot_z: i32, direction: Direction, gen_depth: i32) -> Option<RoomCrossing> {
        let bounding_box = BoundingBox::orient_box(foot_x, foot_y, foot_z, -2, 0, 0, 7, 9, 7, direction);
        is_free_box(builder, &bounding_box).then(|| RoomCrossing::new(gen_depth, bounding_box, direction))
    }

    fn post_process(&self, level: &mut impl WorldGenLevel, chunk_bb: &BoundingBox) {
        let piece = &self.nether_bridge.piece;
        let bricks = nether_bricks();
        piece.generate_box(level, chunk_bb, 0, 0, 0, 6, 1, 6, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 0, 2, 0, 6, 7, 6, air(), air(), false);
        piece.generate_box(level, chunk_bb, 0, 2, 0, 1, 6, 0, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 0, 2, 6, 1, 6, 6, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 5, 2, 0, 6, 6, 0, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 5, 2, 6, 6, 6, 6, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 0, 2, 0, 0, 6, 1, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 0, 2, 5, 0, 6, 6, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 6, 2, 0, 6, 6, 1, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 6, 2, 5, 6, 6, 6, bricks, bricks, false);
        let we_fence = we_fence();
        let ns_fence = ns_fence();
        piece.generate_box(level, chunk_bb, 2, 6, 0, 4, 6, 0, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 2, 5, 0, 4, 5, 0, we_fence, we_fence, false);
        piece.generate_box(level, chunk_bb, 2, 6, 6, 4, 6, 6, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 2, 5, 6, 4, 5, 6, we_fence, we_fence, false);
        piece.generate_box(level, chunk_bb, 0, 6, 2, 0, 6, 4, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 0, 5, 2, 0, 5, 4, ns_fence, ns_fence, false);
        piece.generate_box(level, chunk_bb, 6, 6, 2, 6, 6, 4, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 6, 5, 2, 6, 5, 4, ns_fence, ns_fence, false);
        for x in 0..=6 {
            for z in 0..=6 {
                piece.fill_column_down(level, bricks, x, -1, z, chunk_bb);
            }
        }
    }
}

#[derive(Clone)]
pub struct StairsRoom {
    pub nether_bridge: NetherBridgePiece,
}

impl StairsRoom {
    fn new(gen_depth: i32, bounding_box: BoundingBox, direction: Direction) -> StairsRoom {
        StairsRoom { nether_bridge: NetherBridgePiece::oriented(NETHER_FORTRESS_STAIRS_ROOM, gen_depth, bounding_box, direction) }
    }

    fn load(tag: &Nbt) -> Option<StairsRoom> {
        Some(StairsRoom { nether_bridge: NetherBridgePiece::load(NETHER_FORTRESS_STAIRS_ROOM, tag)? })
    }

    fn add_children(&self, start_piece: &mut StartPiece, builder: &mut StructurePiecesBuilder, random: &mut impl RandomSource) {
        self.nether_bridge.generate_child_right(start_piece, builder, random, 6, 2, false);
    }

    fn create_piece(builder: &StructurePiecesBuilder, foot_x: i32, foot_y: i32, foot_z: i32, gen_depth: i32, direction: Direction) -> Option<StairsRoom> {
        let bounding_box = BoundingBox::orient_box(foot_x, foot_y, foot_z, -2, 0, 0, 7, 11, 7, direction);
        is_free_box(builder, &bounding_box).then(|| StairsRoom::new(gen_depth, bounding_box, direction))
    }

    fn post_process(&self, level: &mut impl WorldGenLevel, chunk_bb: &BoundingBox) {
        let piece = &self.nether_bridge.piece;
        let bricks = nether_bricks();
        piece.generate_box(level, chunk_bb, 0, 0, 0, 6, 1, 6, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 0, 2, 0, 6, 10, 6, air(), air(), false);
        piece.generate_box(level, chunk_bb, 0, 2, 0, 1, 8, 0, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 5, 2, 0, 6, 8, 0, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 0, 2, 1, 0, 8, 6, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 6, 2, 1, 6, 8, 6, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 1, 2, 6, 5, 8, 6, bricks, bricks, false);
        let we_fence = we_fence();
        let ns_fence = ns_fence();
        piece.generate_box(level, chunk_bb, 0, 3, 2, 0, 5, 4, ns_fence, ns_fence, false);
        piece.generate_box(level, chunk_bb, 6, 3, 2, 6, 5, 2, ns_fence, ns_fence, false);
        piece.generate_box(level, chunk_bb, 6, 3, 4, 6, 5, 4, ns_fence, ns_fence, false);
        piece.place_block(level, bricks, 5, 2, 5, chunk_bb);
        piece.generate_box(level, chunk_bb, 4, 2, 5, 4, 3, 5, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 3, 2, 5, 3, 4, 5, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 2, 2, 5, 2, 5, 5, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 1, 2, 5, 1, 6, 5, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 1, 7, 1, 5, 7, 4, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 6, 8, 2, 6, 8, 4, air(), air(), false);
        piece.generate_box(level, chunk_bb, 2, 6, 0, 4, 8, 0, bricks, bricks, false);
        piece.generate_box(level, chunk_bb, 2, 5, 0, 4, 5, 0, we_fence, we_fence, false);
        for x in 0..=6 {
            for z in 0..=6 {
                piece.fill_column_down(level, bricks, x, -1, z, chunk_bb);
            }
        }
    }
}

pub struct StartPiece {
    pub bridge_crossing: BridgeCrossing,
    previous_piece: Option<PieceClass>,
    available_bridge_pieces: Vec<PieceWeight>,
    available_castle_pieces: Vec<PieceWeight>,
    pub pending_children: Vec<NetherBridgePieceKind>,
}

impl StartPiece {
    pub fn new(random: &mut impl RandomSource, west: i32, north: i32) -> StartPiece {
        StartPiece {
            bridge_crossing: BridgeCrossing::new_source(west, north, get_random_horizontal_direction(random)),
            previous_piece: None,
            available_bridge_pieces: BRIDGE_PIECE_WEIGHTS.to_vec(),
            available_castle_pieces: CASTLE_PIECE_WEIGHTS.to_vec(),
            pending_children: Vec::new(),
        }
    }
}

#[derive(Clone)]
pub enum NetherBridgePieceKind {
    BridgeCrossing(BridgeCrossing),
    BridgeEndFiller(BridgeEndFiller),
    BridgeStraight(BridgeStraight),
    CastleCorridorStairsPiece(CastleCorridorStairsPiece),
    CastleCorridorTBalconyPiece(CastleCorridorTBalconyPiece),
    CastleEntrance(CastleEntrance),
    CastleSmallCorridorCrossingPiece(CastleSmallCorridorCrossingPiece),
    CastleSmallCorridorLeftTurnPiece(CastleSmallCorridorLeftTurnPiece),
    CastleSmallCorridorPiece(CastleSmallCorridorPiece),
    CastleSmallCorridorRightTurnPiece(CastleSmallCorridorRightTurnPiece),
    CastleStalkRoom(CastleStalkRoom),
    MonsterThrone(MonsterThrone),
    RoomCrossing(RoomCrossing),
    StairsRoom(StairsRoom),
}

impl NetherBridgePieceKind {
    fn nether_bridge(&self) -> &NetherBridgePiece {
        match self {
            NetherBridgePieceKind::BridgeCrossing(piece) => &piece.nether_bridge,
            NetherBridgePieceKind::BridgeEndFiller(piece) => &piece.nether_bridge,
            NetherBridgePieceKind::BridgeStraight(piece) => &piece.nether_bridge,
            NetherBridgePieceKind::CastleCorridorStairsPiece(piece) => &piece.nether_bridge,
            NetherBridgePieceKind::CastleCorridorTBalconyPiece(piece) => &piece.nether_bridge,
            NetherBridgePieceKind::CastleEntrance(piece) => &piece.nether_bridge,
            NetherBridgePieceKind::CastleSmallCorridorCrossingPiece(piece) => &piece.nether_bridge,
            NetherBridgePieceKind::CastleSmallCorridorLeftTurnPiece(piece) => &piece.nether_bridge,
            NetherBridgePieceKind::CastleSmallCorridorPiece(piece) => &piece.nether_bridge,
            NetherBridgePieceKind::CastleSmallCorridorRightTurnPiece(piece) => &piece.nether_bridge,
            NetherBridgePieceKind::CastleStalkRoom(piece) => &piece.nether_bridge,
            NetherBridgePieceKind::MonsterThrone(piece) => &piece.nether_bridge,
            NetherBridgePieceKind::RoomCrossing(piece) => &piece.nether_bridge,
            NetherBridgePieceKind::StairsRoom(piece) => &piece.nether_bridge,
        }
    }

    fn nether_bridge_mut(&mut self) -> &mut NetherBridgePiece {
        match self {
            NetherBridgePieceKind::BridgeCrossing(piece) => &mut piece.nether_bridge,
            NetherBridgePieceKind::BridgeEndFiller(piece) => &mut piece.nether_bridge,
            NetherBridgePieceKind::BridgeStraight(piece) => &mut piece.nether_bridge,
            NetherBridgePieceKind::CastleCorridorStairsPiece(piece) => &mut piece.nether_bridge,
            NetherBridgePieceKind::CastleCorridorTBalconyPiece(piece) => &mut piece.nether_bridge,
            NetherBridgePieceKind::CastleEntrance(piece) => &mut piece.nether_bridge,
            NetherBridgePieceKind::CastleSmallCorridorCrossingPiece(piece) => &mut piece.nether_bridge,
            NetherBridgePieceKind::CastleSmallCorridorLeftTurnPiece(piece) => &mut piece.nether_bridge,
            NetherBridgePieceKind::CastleSmallCorridorPiece(piece) => &mut piece.nether_bridge,
            NetherBridgePieceKind::CastleSmallCorridorRightTurnPiece(piece) => &mut piece.nether_bridge,
            NetherBridgePieceKind::CastleStalkRoom(piece) => &mut piece.nether_bridge,
            NetherBridgePieceKind::MonsterThrone(piece) => &mut piece.nether_bridge,
            NetherBridgePieceKind::RoomCrossing(piece) => &mut piece.nether_bridge,
            NetherBridgePieceKind::StairsRoom(piece) => &mut piece.nether_bridge,
        }
    }

    pub fn piece(&self) -> &StructurePiece {
        &self.nether_bridge().piece
    }

    pub fn piece_mut(&mut self) -> &mut StructurePiece {
        &mut self.nether_bridge_mut().piece
    }

    pub fn load(piece_id: &str, tag: &Nbt) -> Option<NetherBridgePieceKind> {
        Some(match piece_id {
            NETHER_FORTRESS_BRIDGE_CROSSING => NetherBridgePieceKind::BridgeCrossing(BridgeCrossing::load(NETHER_FORTRESS_BRIDGE_CROSSING, tag)?),
            NETHER_FORTRESS_BRIDGE_END_FILLER => NetherBridgePieceKind::BridgeEndFiller(BridgeEndFiller::load(tag)?),
            NETHER_FORTRESS_BRIDGE_STRAIGHT => NetherBridgePieceKind::BridgeStraight(BridgeStraight::load(tag)?),
            NETHER_FORTRESS_CASTLE_CORRIDOR_STAIRS => NetherBridgePieceKind::CastleCorridorStairsPiece(CastleCorridorStairsPiece::load(tag)?),
            NETHER_FORTRESS_CASTLE_CORRIDOR_T_BALCONY => NetherBridgePieceKind::CastleCorridorTBalconyPiece(CastleCorridorTBalconyPiece::load(tag)?),
            NETHER_FORTRESS_CASTLE_ENTRANCE => NetherBridgePieceKind::CastleEntrance(CastleEntrance::load(tag)?),
            NETHER_FORTRESS_CASTLE_SMALL_CORRIDOR_CROSSING => NetherBridgePieceKind::CastleSmallCorridorCrossingPiece(CastleSmallCorridorCrossingPiece::load(tag)?),
            NETHER_FORTRESS_CASTLE_SMALL_CORRIDOR_LEFT_TURN => NetherBridgePieceKind::CastleSmallCorridorLeftTurnPiece(CastleSmallCorridorLeftTurnPiece::load(tag)?),
            NETHER_FORTRESS_CASTLE_SMALL_CORRIDOR => NetherBridgePieceKind::CastleSmallCorridorPiece(CastleSmallCorridorPiece::load(tag)?),
            NETHER_FORTRESS_CASTLE_SMALL_CORRIDOR_RIGHT_TURN => NetherBridgePieceKind::CastleSmallCorridorRightTurnPiece(CastleSmallCorridorRightTurnPiece::load(tag)?),
            NETHER_FORTRESS_CASTLE_STALK_ROOM => NetherBridgePieceKind::CastleStalkRoom(CastleStalkRoom::load(tag)?),
            NETHER_FORTRESS_MONSTER_THRONE => NetherBridgePieceKind::MonsterThrone(MonsterThrone::load(tag)?),
            NETHER_FORTRESS_ROOM_CROSSING => NetherBridgePieceKind::RoomCrossing(RoomCrossing::load(tag)?),
            NETHER_FORTRESS_STAIRS_ROOM => NetherBridgePieceKind::StairsRoom(StairsRoom::load(tag)?),
            NETHER_FORTRESS_START => NetherBridgePieceKind::BridgeCrossing(BridgeCrossing::load(NETHER_FORTRESS_START, tag)?),
            _ => return None,
        })
    }

    pub fn add_additional_save_data(&self, tag: &mut Vec<(String, Nbt)>) {
        match self {
            NetherBridgePieceKind::BridgeEndFiller(piece) => piece.add_additional_save_data(tag),
            NetherBridgePieceKind::CastleSmallCorridorLeftTurnPiece(piece) => piece.add_additional_save_data(tag),
            NetherBridgePieceKind::CastleSmallCorridorRightTurnPiece(piece) => piece.add_additional_save_data(tag),
            NetherBridgePieceKind::MonsterThrone(piece) => piece.add_additional_save_data(tag),
            _ => {}
        }
    }

    pub fn add_children(&self, start_piece: &mut StartPiece, builder: &mut StructurePiecesBuilder, random: &mut impl RandomSource) {
        match self {
            NetherBridgePieceKind::BridgeCrossing(piece) => piece.add_children(start_piece, builder, random),
            NetherBridgePieceKind::BridgeStraight(piece) => piece.add_children(start_piece, builder, random),
            NetherBridgePieceKind::CastleCorridorStairsPiece(piece) => piece.add_children(start_piece, builder, random),
            NetherBridgePieceKind::CastleCorridorTBalconyPiece(piece) => piece.add_children(start_piece, builder, random),
            NetherBridgePieceKind::CastleEntrance(piece) => piece.add_children(start_piece, builder, random),
            NetherBridgePieceKind::CastleSmallCorridorCrossingPiece(piece) => piece.add_children(start_piece, builder, random),
            NetherBridgePieceKind::CastleSmallCorridorLeftTurnPiece(piece) => piece.add_children(start_piece, builder, random),
            NetherBridgePieceKind::CastleSmallCorridorPiece(piece) => piece.add_children(start_piece, builder, random),
            NetherBridgePieceKind::CastleSmallCorridorRightTurnPiece(piece) => piece.add_children(start_piece, builder, random),
            NetherBridgePieceKind::CastleStalkRoom(piece) => piece.add_children(start_piece, builder, random),
            NetherBridgePieceKind::RoomCrossing(piece) => piece.add_children(start_piece, builder, random),
            NetherBridgePieceKind::StairsRoom(piece) => piece.add_children(start_piece, builder, random),
            NetherBridgePieceKind::BridgeEndFiller(_) | NetherBridgePieceKind::MonsterThrone(_) => {}
        }
    }

    pub fn post_process(&mut self, level: &mut impl WorldGenLevel, random: &mut impl RandomSource, chunk_bb: &BoundingBox) {
        match self {
            NetherBridgePieceKind::BridgeCrossing(piece) => piece.post_process(level, chunk_bb),
            NetherBridgePieceKind::BridgeEndFiller(piece) => piece.post_process(level, chunk_bb),
            NetherBridgePieceKind::BridgeStraight(piece) => piece.post_process(level, chunk_bb),
            NetherBridgePieceKind::CastleCorridorStairsPiece(piece) => piece.post_process(level, chunk_bb),
            NetherBridgePieceKind::CastleCorridorTBalconyPiece(piece) => piece.post_process(level, chunk_bb),
            NetherBridgePieceKind::CastleEntrance(piece) => piece.post_process(level, chunk_bb),
            NetherBridgePieceKind::CastleSmallCorridorCrossingPiece(piece) => piece.post_process(level, chunk_bb),
            NetherBridgePieceKind::CastleSmallCorridorLeftTurnPiece(piece) => piece.post_process(level, random, chunk_bb),
            NetherBridgePieceKind::CastleSmallCorridorPiece(piece) => piece.post_process(level, chunk_bb),
            NetherBridgePieceKind::CastleSmallCorridorRightTurnPiece(piece) => piece.post_process(level, random, chunk_bb),
            NetherBridgePieceKind::CastleStalkRoom(piece) => piece.post_process(level, chunk_bb),
            NetherBridgePieceKind::MonsterThrone(piece) => piece.post_process(level, random, chunk_bb),
            NetherBridgePieceKind::RoomCrossing(piece) => piece.post_process(level, chunk_bb),
            NetherBridgePieceKind::StairsRoom(piece) => piece.post_process(level, chunk_bb),
        }
    }
}
