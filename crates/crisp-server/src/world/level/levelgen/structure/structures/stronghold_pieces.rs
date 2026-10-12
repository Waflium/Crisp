use crisp_core::Direction;
use crisp_core::random::RandomSource;
use crisp_data::properties::{DOUBLE_BLOCK_HALF, DoubleBlockHalf, EAST, EYE, HORIZONTAL_FACING, NORTH, SLAB_TYPE, SOUTH, SlabType, WEST};
use crisp_data::{BlockState, blocks};
use crisp_protocol::nbt::Nbt;

use super::Piece;
use crate::world::level::world_gen_level::WorldGenLevel;
use crate::world::level::block::entity::BlockEntityKind;
use crate::world::level::levelgen::structure::BoundingBox;
use crate::world::level::levelgen::structure::pieces::structure_pieces_builder::StructurePiecesBuilder;
use crate::world::level::levelgen::structure::structure_piece::{BlockSelector, StructurePiece, get_random_horizontal_direction, make_bounding_box};

pub const STRONGHOLD_CHEST_CORRIDOR: &str = "minecraft:shcc";
pub const STRONGHOLD_FILLER_CORRIDOR: &str = "minecraft:shfc";
pub const STRONGHOLD_FIVE_CROSSING: &str = "minecraft:sh5c";
pub const STRONGHOLD_LEFT_TURN: &str = "minecraft:shlt";
pub const STRONGHOLD_LIBRARY: &str = "minecraft:shli";
pub const STRONGHOLD_PORTAL_ROOM: &str = "minecraft:shpr";
pub const STRONGHOLD_PRISON_HALL: &str = "minecraft:shph";
pub const STRONGHOLD_RIGHT_TURN: &str = "minecraft:shrt";
pub const STRONGHOLD_ROOM_CROSSING: &str = "minecraft:shrc";
pub const STRONGHOLD_STAIRS_DOWN: &str = "minecraft:shsd";
pub const STRONGHOLD_START: &str = "minecraft:shstart";
pub const STRONGHOLD_STRAIGHT: &str = "minecraft:shs";
pub const STRONGHOLD_STRAIGHT_STAIRS_DOWN: &str = "minecraft:shssd";

const STRONGHOLD_LIBRARY_LOOT: &str = "minecraft:chests/stronghold_library";
const STRONGHOLD_CROSSING_LOOT: &str = "minecraft:chests/stronghold_crossing";
const STRONGHOLD_CORRIDOR_LOOT: &str = "minecraft:chests/stronghold_corridor";

const MAX_DEPTH: i32 = 50;
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

fn cave_air() -> BlockState {
    blocks::CAVE_AIR.default_state()
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PieceClass {
    Straight,
    PrisonHall,
    LeftTurn,
    RightTurn,
    RoomCrossing,
    StraightStairsDown,
    StairsDown,
    FiveCrossing,
    ChestCorridor,
    Library,
    PortalRoom,
}

const STRONGHOLD_PIECE_WEIGHTS: [(PieceClass, i32, i32); 11] = [
    (PieceClass::Straight, 40, 0),
    (PieceClass::PrisonHall, 5, 5),
    (PieceClass::LeftTurn, 20, 0),
    (PieceClass::RightTurn, 20, 0),
    (PieceClass::RoomCrossing, 10, 6),
    (PieceClass::StraightStairsDown, 5, 5),
    (PieceClass::StairsDown, 5, 5),
    (PieceClass::FiveCrossing, 5, 4),
    (PieceClass::ChestCorridor, 5, 4),
    (PieceClass::Library, 10, 2),
    (PieceClass::PortalRoom, 20, 1),
];

struct PieceWeight {
    piece_class: PieceClass,
    weight: i32,
    place_count: i32,
    max_place_count: i32,
}

impl PieceWeight {
    fn do_place(&self, depth: i32) -> bool {
        (self.max_place_count == 0 || self.place_count < self.max_place_count)
            && match self.piece_class {
                PieceClass::Library => depth > 4,
                PieceClass::PortalRoom => depth > 5,
                _ => true,
            }
    }

    fn is_valid(&self) -> bool {
        self.max_place_count == 0 || self.place_count < self.max_place_count
    }
}

#[derive(Default)]
pub struct StrongholdPieces {
    current_pieces: Vec<PieceWeight>,
    imposed_piece: Option<PieceClass>,
    total_weight: i32,
}

impl StrongholdPieces {
    pub fn reset_pieces(&mut self) {
        self.current_pieces.clear();
        self.current_pieces.extend(STRONGHOLD_PIECE_WEIGHTS.iter().map(|&(piece_class, weight, max_place_count)| PieceWeight { piece_class, weight, place_count: 0, max_place_count }));
        self.imposed_piece = None;
    }

    fn update_piece_weight(&mut self) -> bool {
        let mut has_any_pieces = false;
        self.total_weight = 0;
        for piece in &self.current_pieces {
            if piece.max_place_count > 0 && piece.place_count < piece.max_place_count {
                has_any_pieces = true;
            }
            self.total_weight += piece.weight;
        }
        has_any_pieces
    }

    #[allow(clippy::too_many_arguments)]
    fn find_and_create_piece_factory(
        piece_class: PieceClass,
        builder: &StructurePiecesBuilder,
        random: &mut impl RandomSource,
        foot_x: i32,
        foot_y: i32,
        foot_z: i32,
        direction: Direction,
        depth: i32,
    ) -> Option<StrongholdPieceKind> {
        match piece_class {
            PieceClass::Straight => Straight::create_piece(builder, random, foot_x, foot_y, foot_z, direction, depth).map(StrongholdPieceKind::Straight),
            PieceClass::PrisonHall => PrisonHall::create_piece(builder, random, foot_x, foot_y, foot_z, direction, depth).map(StrongholdPieceKind::PrisonHall),
            PieceClass::LeftTurn => LeftTurn::create_piece(builder, random, foot_x, foot_y, foot_z, direction, depth).map(StrongholdPieceKind::LeftTurn),
            PieceClass::RightTurn => RightTurn::create_piece(builder, random, foot_x, foot_y, foot_z, direction, depth).map(StrongholdPieceKind::RightTurn),
            PieceClass::RoomCrossing => RoomCrossing::create_piece(builder, random, foot_x, foot_y, foot_z, direction, depth).map(StrongholdPieceKind::RoomCrossing),
            PieceClass::StraightStairsDown => {
                StraightStairsDown::create_piece(builder, random, foot_x, foot_y, foot_z, direction, depth).map(StrongholdPieceKind::StraightStairsDown)
            }
            PieceClass::StairsDown => StairsDown::create_piece(builder, random, foot_x, foot_y, foot_z, direction, depth).map(StrongholdPieceKind::StairsDown),
            PieceClass::FiveCrossing => FiveCrossing::create_piece(builder, random, foot_x, foot_y, foot_z, direction, depth).map(StrongholdPieceKind::FiveCrossing),
            PieceClass::ChestCorridor => ChestCorridor::create_piece(builder, random, foot_x, foot_y, foot_z, direction, depth).map(StrongholdPieceKind::ChestCorridor),
            PieceClass::Library => Library::create_piece(builder, random, foot_x, foot_y, foot_z, direction, depth).map(StrongholdPieceKind::Library),
            PieceClass::PortalRoom => PortalRoom::create_piece(builder, foot_x, foot_y, foot_z, direction, depth).map(StrongholdPieceKind::PortalRoom),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn generate_piece_from_small_door(
        &mut self,
        start_piece: &mut StartPiece,
        builder: &StructurePiecesBuilder,
        random: &mut impl RandomSource,
        foot_x: i32,
        foot_y: i32,
        foot_z: i32,
        direction: Direction,
        depth: i32,
    ) -> Option<StrongholdPieceKind> {
        if !self.update_piece_weight() {
            return None;
        }
        if let Some(imposed_piece) = self.imposed_piece {
            let stronghold_piece = StrongholdPieces::find_and_create_piece_factory(imposed_piece, builder, random, foot_x, foot_y, foot_z, direction, depth);
            self.imposed_piece = None;
            if stronghold_piece.is_some() {
                return stronghold_piece;
            }
        }
        let mut num_attempts = 0;
        while num_attempts < 5 {
            num_attempts += 1;
            let mut weight_selection = random.next_int_bounded(self.total_weight);
            for index in 0..self.current_pieces.len() {
                let piece = &self.current_pieces[index];
                weight_selection -= piece.weight;
                if weight_selection < 0 {
                    if !piece.do_place(depth) || start_piece.previous_piece == Some(piece.piece_class) {
                        break;
                    }
                    if let Some(stronghold_piece) = StrongholdPieces::find_and_create_piece_factory(piece.piece_class, builder, random, foot_x, foot_y, foot_z, direction, depth) {
                        let piece = &mut self.current_pieces[index];
                        piece.place_count += 1;
                        start_piece.previous_piece = Some(piece.piece_class);
                        if !piece.is_valid() {
                            self.current_pieces.remove(index);
                        }
                        return Some(stronghold_piece);
                    }
                }
            }
        }
        let bounding_box = FillerCorridor::find_piece_box(builder, foot_x, foot_y, foot_z, direction)?;
        if bounding_box.min.y > 1 { Some(StrongholdPieceKind::FillerCorridor(FillerCorridor::new(depth, bounding_box, direction))) } else { None }
    }

    #[allow(clippy::too_many_arguments)]
    fn generate_and_add_piece(
        &mut self,
        start_piece: &mut StartPiece,
        builder: &mut StructurePiecesBuilder,
        random: &mut impl RandomSource,
        foot_x: i32,
        foot_y: i32,
        foot_z: i32,
        direction: Direction,
        depth: i32,
    ) {
        if depth > MAX_DEPTH {
            return;
        }
        let start_box = start_piece.stairs_down.stronghold.piece.bounding_box;
        if (foot_x - start_box.min.x).abs() <= 112
            && (foot_z - start_box.min.z).abs() <= 112
            && let Some(new_piece) = self.generate_piece_from_small_door(start_piece, builder, random, foot_x, foot_y, foot_z, direction, depth + 1)
        {
            builder.add_piece(Piece::Stronghold(new_piece.clone()));
            start_piece.pending_children.push(new_piece);
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SmallDoorType {
    Opening,
    WoodDoor,
    Grates,
    IronDoor,
}

impl SmallDoorType {
    fn name(self) -> &'static str {
        match self {
            SmallDoorType::Opening => "OPENING",
            SmallDoorType::WoodDoor => "WOOD_DOOR",
            SmallDoorType::Grates => "GRATES",
            SmallDoorType::IronDoor => "IRON_DOOR",
        }
    }

    fn by_name(name: &str) -> Option<SmallDoorType> {
        Some(match name {
            "OPENING" => SmallDoorType::Opening,
            "WOOD_DOOR" => SmallDoorType::WoodDoor,
            "GRATES" => SmallDoorType::Grates,
            "IRON_DOOR" => SmallDoorType::IronDoor,
            _ => return None,
        })
    }
}

struct SmoothStoneSelector {
    next: BlockState,
}

impl SmoothStoneSelector {
    fn new() -> SmoothStoneSelector {
        SmoothStoneSelector { next: blocks::AIR.default_state() }
    }
}

impl BlockSelector for SmoothStoneSelector {
    fn next(&mut self, random: &mut impl RandomSource, _world_x: i32, _world_y: i32, _world_z: i32, is_edge: bool) {
        self.next = if is_edge {
            let selection = random.next_float();
            if selection < 0.2 {
                blocks::CRACKED_STONE_BRICKS.default_state()
            } else if selection < 0.5 {
                blocks::MOSSY_STONE_BRICKS.default_state()
            } else if selection < 0.55 {
                blocks::INFESTED_STONE_BRICKS.default_state()
            } else {
                blocks::STONE_BRICKS.default_state()
            }
        } else {
            cave_air()
        };
    }

    fn get_next(&self) -> BlockState {
        self.next
    }
}

#[derive(Clone)]
pub struct StrongholdPiece {
    pub piece: StructurePiece,
    pub entry_door: SmallDoorType,
}

impl StrongholdPiece {
    fn new(piece_type: &'static str, gen_depth: i32, bounding_box: BoundingBox) -> StrongholdPiece {
        StrongholdPiece { piece: StructurePiece::new(piece_type, gen_depth, bounding_box), entry_door: SmallDoorType::Opening }
    }

    fn oriented(piece_type: &'static str, gen_depth: i32, bounding_box: BoundingBox, direction: Direction) -> StrongholdPiece {
        let mut stronghold = StrongholdPiece::new(piece_type, gen_depth, bounding_box);
        stronghold.piece.set_orientation(Some(direction));
        stronghold
    }

    fn load(piece_type: &'static str, tag: &Nbt) -> Option<StrongholdPiece> {
        let entry_door = SmallDoorType::by_name(tag.get("EntryDoor").and_then(Nbt::as_str)?)?;
        Some(StrongholdPiece { piece: StructurePiece::load(piece_type, tag)?, entry_door })
    }

    fn add_additional_save_data(&self, tag: &mut Vec<(String, Nbt)>) {
        tag.push(("EntryDoor".into(), Nbt::String(self.entry_door.name().to_owned())));
    }

    fn generate_small_door(&self, level: &mut impl WorldGenLevel, chunk_bb: &BoundingBox, door_type: SmallDoorType, foot_x: i32, foot_y: i32, foot_z: i32) {
        let piece = &self.piece;
        let stone_bricks = blocks::STONE_BRICKS.default_state();
        match door_type {
            SmallDoorType::Opening => {
                piece.generate_box(level, chunk_bb, foot_x, foot_y, foot_z, foot_x + 3 - 1, foot_y + 3 - 1, foot_z, cave_air(), cave_air(), false);
            }
            SmallDoorType::WoodDoor => {
                piece.place_block(level, stone_bricks, foot_x, foot_y, foot_z, chunk_bb);
                piece.place_block(level, stone_bricks, foot_x, foot_y + 1, foot_z, chunk_bb);
                piece.place_block(level, stone_bricks, foot_x, foot_y + 2, foot_z, chunk_bb);
                piece.place_block(level, stone_bricks, foot_x + 1, foot_y + 2, foot_z, chunk_bb);
                piece.place_block(level, stone_bricks, foot_x + 2, foot_y + 2, foot_z, chunk_bb);
                piece.place_block(level, stone_bricks, foot_x + 2, foot_y + 1, foot_z, chunk_bb);
                piece.place_block(level, stone_bricks, foot_x + 2, foot_y, foot_z, chunk_bb);
                piece.place_block(level, blocks::OAK_DOOR.default_state(), foot_x + 1, foot_y, foot_z, chunk_bb);
                piece.place_block(level, blocks::OAK_DOOR.default_state().set(DOUBLE_BLOCK_HALF, DoubleBlockHalf::Upper), foot_x + 1, foot_y + 1, foot_z, chunk_bb);
            }
            SmallDoorType::Grates => {
                let bars = blocks::IRON_BARS.default_state();
                piece.place_block(level, cave_air(), foot_x + 1, foot_y, foot_z, chunk_bb);
                piece.place_block(level, cave_air(), foot_x + 1, foot_y + 1, foot_z, chunk_bb);
                piece.place_block(level, bars.set(WEST, true), foot_x, foot_y, foot_z, chunk_bb);
                piece.place_block(level, bars.set(WEST, true), foot_x, foot_y + 1, foot_z, chunk_bb);
                piece.place_block(level, bars.set(EAST, true).set(WEST, true), foot_x, foot_y + 2, foot_z, chunk_bb);
                piece.place_block(level, bars.set(EAST, true).set(WEST, true), foot_x + 1, foot_y + 2, foot_z, chunk_bb);
                piece.place_block(level, bars.set(EAST, true).set(WEST, true), foot_x + 2, foot_y + 2, foot_z, chunk_bb);
                piece.place_block(level, bars.set(EAST, true), foot_x + 2, foot_y + 1, foot_z, chunk_bb);
                piece.place_block(level, bars.set(EAST, true), foot_x + 2, foot_y, foot_z, chunk_bb);
            }
            SmallDoorType::IronDoor => {
                piece.place_block(level, stone_bricks, foot_x, foot_y, foot_z, chunk_bb);
                piece.place_block(level, stone_bricks, foot_x, foot_y + 1, foot_z, chunk_bb);
                piece.place_block(level, stone_bricks, foot_x, foot_y + 2, foot_z, chunk_bb);
                piece.place_block(level, stone_bricks, foot_x + 1, foot_y + 2, foot_z, chunk_bb);
                piece.place_block(level, stone_bricks, foot_x + 2, foot_y + 2, foot_z, chunk_bb);
                piece.place_block(level, stone_bricks, foot_x + 2, foot_y + 1, foot_z, chunk_bb);
                piece.place_block(level, stone_bricks, foot_x + 2, foot_y, foot_z, chunk_bb);
                piece.place_block(level, blocks::IRON_DOOR.default_state(), foot_x + 1, foot_y, foot_z, chunk_bb);
                piece.place_block(level, blocks::IRON_DOOR.default_state().set(DOUBLE_BLOCK_HALF, DoubleBlockHalf::Upper), foot_x + 1, foot_y + 1, foot_z, chunk_bb);
                piece.place_block(level, blocks::STONE_BUTTON.default_state().set(HORIZONTAL_FACING, Direction::North), foot_x + 2, foot_y + 1, foot_z + 1, chunk_bb);
                piece.place_block(level, blocks::STONE_BUTTON.default_state().set(HORIZONTAL_FACING, Direction::South), foot_x + 2, foot_y + 1, foot_z - 1, chunk_bb);
            }
        }
    }

    fn random_small_door(random: &mut impl RandomSource) -> SmallDoorType {
        match random.next_int_bounded(5) {
            2 => SmallDoorType::WoodDoor,
            3 => SmallDoorType::Grates,
            4 => SmallDoorType::IronDoor,
            _ => SmallDoorType::Opening,
        }
    }

    fn generate_small_door_child_forward(
        &self,
        pieces: &mut StrongholdPieces,
        start_piece: &mut StartPiece,
        builder: &mut StructurePiecesBuilder,
        random: &mut impl RandomSource,
        x_off: i32,
        y_off: i32,
    ) {
        let Some(orientation) = self.piece.get_orientation() else { return };
        let bounding_box = &self.piece.bounding_box;
        let (foot_x, foot_z) = match orientation {
            Direction::North => (bounding_box.min.x + x_off, bounding_box.min.z - 1),
            Direction::South => (bounding_box.min.x + x_off, bounding_box.max.z + 1),
            Direction::West => (bounding_box.min.x - 1, bounding_box.min.z + x_off),
            Direction::East => (bounding_box.max.x + 1, bounding_box.min.z + x_off),
            _ => return,
        };
        pieces.generate_and_add_piece(start_piece, builder, random, foot_x, bounding_box.min.y + y_off, foot_z, orientation, self.piece.gen_depth);
    }

    fn generate_small_door_child_left(
        &self,
        pieces: &mut StrongholdPieces,
        start_piece: &mut StartPiece,
        builder: &mut StructurePiecesBuilder,
        random: &mut impl RandomSource,
        y_off: i32,
        z_off: i32,
    ) {
        let Some(orientation) = self.piece.get_orientation() else { return };
        let bounding_box = &self.piece.bounding_box;
        let (foot_x, foot_z, direction) = match orientation {
            Direction::North | Direction::South => (bounding_box.min.x - 1, bounding_box.min.z + z_off, Direction::West),
            Direction::West | Direction::East => (bounding_box.min.x + z_off, bounding_box.min.z - 1, Direction::North),
            _ => return,
        };
        pieces.generate_and_add_piece(start_piece, builder, random, foot_x, bounding_box.min.y + y_off, foot_z, direction, self.piece.gen_depth);
    }

    fn generate_small_door_child_right(
        &self,
        pieces: &mut StrongholdPieces,
        start_piece: &mut StartPiece,
        builder: &mut StructurePiecesBuilder,
        random: &mut impl RandomSource,
        y_off: i32,
        z_off: i32,
    ) {
        let Some(orientation) = self.piece.get_orientation() else { return };
        let bounding_box = &self.piece.bounding_box;
        let (foot_x, foot_z, direction) = match orientation {
            Direction::North | Direction::South => (bounding_box.max.x + 1, bounding_box.min.z + z_off, Direction::East),
            Direction::West | Direction::East => (bounding_box.min.x + z_off, bounding_box.max.z + 1, Direction::South),
            _ => return,
        };
        pieces.generate_and_add_piece(start_piece, builder, random, foot_x, bounding_box.min.y + y_off, foot_z, direction, self.piece.gen_depth);
    }

    fn is_ok_box(bounding_box: &BoundingBox) -> bool {
        bounding_box.min.y > LOWEST_Y_POSITION
    }
}

fn is_free_box(builder: &StructurePiecesBuilder, bounding_box: &BoundingBox) -> bool {
    StrongholdPiece::is_ok_box(bounding_box) && builder.find_collision_piece(bounding_box).is_none()
}

#[derive(Clone)]
pub struct ChestCorridor {
    pub stronghold: StrongholdPiece,
    has_placed_chest: bool,
}

impl ChestCorridor {
    fn new(gen_depth: i32, random: &mut impl RandomSource, bounding_box: BoundingBox, direction: Direction) -> ChestCorridor {
        let mut stronghold = StrongholdPiece::oriented(STRONGHOLD_CHEST_CORRIDOR, gen_depth, bounding_box, direction);
        stronghold.entry_door = StrongholdPiece::random_small_door(random);
        ChestCorridor { stronghold, has_placed_chest: false }
    }

    fn load(tag: &Nbt) -> Option<ChestCorridor> {
        Some(ChestCorridor { stronghold: StrongholdPiece::load(STRONGHOLD_CHEST_CORRIDOR, tag)?, has_placed_chest: boolean(tag, "Chest") })
    }

    fn add_additional_save_data(&self, tag: &mut Vec<(String, Nbt)>) {
        self.stronghold.add_additional_save_data(tag);
        tag.push(("Chest".into(), Nbt::Byte(self.has_placed_chest as i8)));
    }

    fn add_children(&self, pieces: &mut StrongholdPieces, start_piece: &mut StartPiece, builder: &mut StructurePiecesBuilder, random: &mut impl RandomSource) {
        self.stronghold.generate_small_door_child_forward(pieces, start_piece, builder, random, 1, 1);
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
    ) -> Option<ChestCorridor> {
        let bounding_box = BoundingBox::orient_box(foot_x, foot_y, foot_z, -1, -1, 0, 5, 5, 7, direction);
        is_free_box(builder, &bounding_box).then(|| ChestCorridor::new(gen_depth, random, bounding_box, direction))
    }

    fn post_process(&mut self, level: &mut impl WorldGenLevel, random: &mut impl RandomSource, chunk_bb: &BoundingBox) {
        let stronghold = &self.stronghold;
        let piece = &stronghold.piece;
        let selector = &mut SmoothStoneSelector::new();
        let stone_bricks = blocks::STONE_BRICKS.default_state();
        let slab = blocks::STONE_BRICK_SLAB.default_state();
        piece.generate_box_with_selector(level, chunk_bb, 0, 0, 0, 4, 4, 6, true, random, selector);
        stronghold.generate_small_door(level, chunk_bb, stronghold.entry_door, 1, 1, 0);
        stronghold.generate_small_door(level, chunk_bb, SmallDoorType::Opening, 1, 1, 6);
        piece.generate_box(level, chunk_bb, 3, 1, 2, 3, 1, 4, stone_bricks, stone_bricks, false);
        piece.place_block(level, slab, 3, 1, 1, chunk_bb);
        piece.place_block(level, slab, 3, 1, 5, chunk_bb);
        piece.place_block(level, slab, 3, 2, 2, chunk_bb);
        piece.place_block(level, slab, 3, 2, 4, chunk_bb);
        for z in 2..=4 {
            piece.place_block(level, slab, 2, 1, z, chunk_bb);
        }
        if !self.has_placed_chest && chunk_bb.is_inside(piece.get_world_pos(3, 2, 3)) {
            self.has_placed_chest = true;
            piece.create_chest(level, chunk_bb, random, piece.get_world_pos(3, 2, 3), STRONGHOLD_CORRIDOR_LOOT, None);
        }
    }
}

#[derive(Clone)]
pub struct FillerCorridor {
    pub stronghold: StrongholdPiece,
    steps: i32,
}

impl FillerCorridor {
    fn new(gen_depth: i32, bounding_box: BoundingBox, direction: Direction) -> FillerCorridor {
        let stronghold = StrongholdPiece::oriented(STRONGHOLD_FILLER_CORRIDOR, gen_depth, bounding_box, direction);
        let steps = if direction != Direction::North && direction != Direction::South { bounding_box.x_span() } else { bounding_box.z_span() };
        FillerCorridor { stronghold, steps }
    }

    fn load(tag: &Nbt) -> Option<FillerCorridor> {
        Some(FillerCorridor { stronghold: StrongholdPiece::load(STRONGHOLD_FILLER_CORRIDOR, tag)?, steps: int(tag, "Steps") })
    }

    fn add_additional_save_data(&self, tag: &mut Vec<(String, Nbt)>) {
        self.stronghold.add_additional_save_data(tag);
        tag.push(("Steps".into(), Nbt::Int(self.steps)));
    }

    fn find_piece_box(builder: &StructurePiecesBuilder, foot_x: i32, foot_y: i32, foot_z: i32, direction: Direction) -> Option<BoundingBox> {
        let bounding_box = BoundingBox::orient_box(foot_x, foot_y, foot_z, -1, -1, 0, 5, 5, 4, direction);
        let collision_box = builder.find_collision_piece(&bounding_box)?.piece().bounding_box;
        if collision_box.min.y == bounding_box.min.y {
            for depth in (1..=2).rev() {
                let bounding_box = BoundingBox::orient_box(foot_x, foot_y, foot_z, -1, -1, 0, 5, 5, depth, direction);
                if !collision_box.intersects(&bounding_box) {
                    return Some(BoundingBox::orient_box(foot_x, foot_y, foot_z, -1, -1, 0, 5, 5, depth + 1, direction));
                }
            }
        }
        None
    }

    fn post_process(&self, level: &mut impl WorldGenLevel, chunk_bb: &BoundingBox) {
        let piece = &self.stronghold.piece;
        let stone_bricks = blocks::STONE_BRICKS.default_state();
        for i in 0..self.steps {
            piece.place_block(level, stone_bricks, 0, 0, i, chunk_bb);
            piece.place_block(level, stone_bricks, 1, 0, i, chunk_bb);
            piece.place_block(level, stone_bricks, 2, 0, i, chunk_bb);
            piece.place_block(level, stone_bricks, 3, 0, i, chunk_bb);
            piece.place_block(level, stone_bricks, 4, 0, i, chunk_bb);
            for y in 1..=3 {
                piece.place_block(level, stone_bricks, 0, y, i, chunk_bb);
                piece.place_block(level, cave_air(), 1, y, i, chunk_bb);
                piece.place_block(level, cave_air(), 2, y, i, chunk_bb);
                piece.place_block(level, cave_air(), 3, y, i, chunk_bb);
                piece.place_block(level, stone_bricks, 4, y, i, chunk_bb);
            }
            piece.place_block(level, stone_bricks, 0, 4, i, chunk_bb);
            piece.place_block(level, stone_bricks, 1, 4, i, chunk_bb);
            piece.place_block(level, stone_bricks, 2, 4, i, chunk_bb);
            piece.place_block(level, stone_bricks, 3, 4, i, chunk_bb);
            piece.place_block(level, stone_bricks, 4, 4, i, chunk_bb);
        }
    }
}

#[derive(Clone)]
pub struct FiveCrossing {
    pub stronghold: StrongholdPiece,
    left_low: bool,
    left_high: bool,
    right_low: bool,
    right_high: bool,
}

impl FiveCrossing {
    fn new(gen_depth: i32, random: &mut impl RandomSource, bounding_box: BoundingBox, direction: Direction) -> FiveCrossing {
        let mut stronghold = StrongholdPiece::oriented(STRONGHOLD_FIVE_CROSSING, gen_depth, bounding_box, direction);
        stronghold.entry_door = StrongholdPiece::random_small_door(random);
        let left_low = random.next_boolean();
        let left_high = random.next_boolean();
        let right_low = random.next_boolean();
        let right_high = random.next_int_bounded(3) > 0;
        FiveCrossing { stronghold, left_low, left_high, right_low, right_high }
    }

    fn load(tag: &Nbt) -> Option<FiveCrossing> {
        Some(FiveCrossing {
            stronghold: StrongholdPiece::load(STRONGHOLD_FIVE_CROSSING, tag)?,
            left_low: boolean(tag, "leftLow"),
            left_high: boolean(tag, "leftHigh"),
            right_low: boolean(tag, "rightLow"),
            right_high: boolean(tag, "rightHigh"),
        })
    }

    fn add_additional_save_data(&self, tag: &mut Vec<(String, Nbt)>) {
        self.stronghold.add_additional_save_data(tag);
        tag.push(("leftLow".into(), Nbt::Byte(self.left_low as i8)));
        tag.push(("leftHigh".into(), Nbt::Byte(self.left_high as i8)));
        tag.push(("rightLow".into(), Nbt::Byte(self.right_low as i8)));
        tag.push(("rightHigh".into(), Nbt::Byte(self.right_high as i8)));
    }

    fn add_children(&self, pieces: &mut StrongholdPieces, start_piece: &mut StartPiece, builder: &mut StructurePiecesBuilder, random: &mut impl RandomSource) {
        let mut z_off_a = 3;
        let mut z_off_b = 5;
        let orientation = self.stronghold.piece.get_orientation();
        if orientation == Some(Direction::West) || orientation == Some(Direction::North) {
            z_off_a = 8 - z_off_a;
            z_off_b = 8 - z_off_b;
        }
        self.stronghold.generate_small_door_child_forward(pieces, start_piece, builder, random, 5, 1);
        if self.left_low {
            self.stronghold.generate_small_door_child_left(pieces, start_piece, builder, random, z_off_a, 1);
        }
        if self.left_high {
            self.stronghold.generate_small_door_child_left(pieces, start_piece, builder, random, z_off_b, 7);
        }
        if self.right_low {
            self.stronghold.generate_small_door_child_right(pieces, start_piece, builder, random, z_off_a, 1);
        }
        if self.right_high {
            self.stronghold.generate_small_door_child_right(pieces, start_piece, builder, random, z_off_b, 7);
        }
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
    ) -> Option<FiveCrossing> {
        let bounding_box = BoundingBox::orient_box(foot_x, foot_y, foot_z, -4, -3, 0, 10, 9, 11, direction);
        is_free_box(builder, &bounding_box).then(|| FiveCrossing::new(gen_depth, random, bounding_box, direction))
    }

    fn post_process(&self, level: &mut impl WorldGenLevel, random: &mut impl RandomSource, chunk_bb: &BoundingBox) {
        let stronghold = &self.stronghold;
        let piece = &stronghold.piece;
        let selector = &mut SmoothStoneSelector::new();
        let slab = blocks::SMOOTH_STONE_SLAB.default_state();
        piece.generate_box_with_selector(level, chunk_bb, 0, 0, 0, 9, 8, 10, true, random, selector);
        stronghold.generate_small_door(level, chunk_bb, stronghold.entry_door, 4, 3, 0);
        if self.left_low {
            piece.generate_box(level, chunk_bb, 0, 3, 1, 0, 5, 3, cave_air(), cave_air(), false);
        }
        if self.right_low {
            piece.generate_box(level, chunk_bb, 9, 3, 1, 9, 5, 3, cave_air(), cave_air(), false);
        }
        if self.left_high {
            piece.generate_box(level, chunk_bb, 0, 5, 7, 0, 7, 9, cave_air(), cave_air(), false);
        }
        if self.right_high {
            piece.generate_box(level, chunk_bb, 9, 5, 7, 9, 7, 9, cave_air(), cave_air(), false);
        }
        piece.generate_box(level, chunk_bb, 5, 1, 10, 7, 3, 10, cave_air(), cave_air(), false);
        piece.generate_box_with_selector(level, chunk_bb, 1, 2, 1, 8, 2, 6, false, random, selector);
        piece.generate_box_with_selector(level, chunk_bb, 4, 1, 5, 4, 4, 9, false, random, selector);
        piece.generate_box_with_selector(level, chunk_bb, 8, 1, 5, 8, 4, 9, false, random, selector);
        piece.generate_box_with_selector(level, chunk_bb, 1, 4, 7, 3, 4, 9, false, random, selector);
        piece.generate_box_with_selector(level, chunk_bb, 1, 3, 5, 3, 3, 6, false, random, selector);
        piece.generate_box(level, chunk_bb, 1, 3, 4, 3, 3, 4, slab, slab, false);
        piece.generate_box(level, chunk_bb, 1, 4, 6, 3, 4, 6, slab, slab, false);
        piece.generate_box_with_selector(level, chunk_bb, 5, 1, 7, 7, 1, 8, false, random, selector);
        piece.generate_box(level, chunk_bb, 5, 1, 9, 7, 1, 9, slab, slab, false);
        piece.generate_box(level, chunk_bb, 5, 2, 7, 7, 2, 7, slab, slab, false);
        piece.generate_box(level, chunk_bb, 4, 5, 7, 4, 5, 9, slab, slab, false);
        piece.generate_box(level, chunk_bb, 8, 5, 7, 8, 5, 9, slab, slab, false);
        let double_slab = slab.set(SLAB_TYPE, SlabType::Double);
        piece.generate_box(level, chunk_bb, 5, 5, 7, 7, 5, 9, double_slab, double_slab, false);
        piece.place_block(level, blocks::WALL_TORCH.default_state().set(HORIZONTAL_FACING, Direction::South), 6, 5, 6, chunk_bb);
    }
}

#[derive(Clone)]
pub struct Turn {
    pub stronghold: StrongholdPiece,
}

#[derive(Clone)]
pub struct LeftTurn {
    pub turn: Turn,
}

impl LeftTurn {
    fn new(gen_depth: i32, random: &mut impl RandomSource, bounding_box: BoundingBox, direction: Direction) -> LeftTurn {
        let mut stronghold = StrongholdPiece::oriented(STRONGHOLD_LEFT_TURN, gen_depth, bounding_box, direction);
        stronghold.entry_door = StrongholdPiece::random_small_door(random);
        LeftTurn { turn: Turn { stronghold } }
    }

    fn load(tag: &Nbt) -> Option<LeftTurn> {
        Some(LeftTurn { turn: Turn { stronghold: StrongholdPiece::load(STRONGHOLD_LEFT_TURN, tag)? } })
    }

    fn add_children(&self, pieces: &mut StrongholdPieces, start_piece: &mut StartPiece, builder: &mut StructurePiecesBuilder, random: &mut impl RandomSource) {
        let stronghold = &self.turn.stronghold;
        let orientation = stronghold.piece.get_orientation();
        if orientation != Some(Direction::North) && orientation != Some(Direction::East) {
            stronghold.generate_small_door_child_right(pieces, start_piece, builder, random, 1, 1);
        } else {
            stronghold.generate_small_door_child_left(pieces, start_piece, builder, random, 1, 1);
        }
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
    ) -> Option<LeftTurn> {
        let bounding_box = BoundingBox::orient_box(foot_x, foot_y, foot_z, -1, -1, 0, 5, 5, 5, direction);
        is_free_box(builder, &bounding_box).then(|| LeftTurn::new(gen_depth, random, bounding_box, direction))
    }

    fn post_process(&self, level: &mut impl WorldGenLevel, random: &mut impl RandomSource, chunk_bb: &BoundingBox) {
        let stronghold = &self.turn.stronghold;
        let piece = &stronghold.piece;
        piece.generate_box_with_selector(level, chunk_bb, 0, 0, 0, 4, 4, 4, true, random, &mut SmoothStoneSelector::new());
        stronghold.generate_small_door(level, chunk_bb, stronghold.entry_door, 1, 1, 0);
        let orientation = piece.get_orientation();
        if orientation != Some(Direction::North) && orientation != Some(Direction::East) {
            piece.generate_box(level, chunk_bb, 4, 1, 1, 4, 3, 3, cave_air(), cave_air(), false);
        } else {
            piece.generate_box(level, chunk_bb, 0, 1, 1, 0, 3, 3, cave_air(), cave_air(), false);
        }
    }
}

#[derive(Clone)]
pub struct Library {
    pub stronghold: StrongholdPiece,
    is_tall: bool,
}

impl Library {
    fn new(gen_depth: i32, random: &mut impl RandomSource, bounding_box: BoundingBox, direction: Direction) -> Library {
        let mut stronghold = StrongholdPiece::oriented(STRONGHOLD_LIBRARY, gen_depth, bounding_box, direction);
        stronghold.entry_door = StrongholdPiece::random_small_door(random);
        Library { stronghold, is_tall: bounding_box.y_span() > 6 }
    }

    fn load(tag: &Nbt) -> Option<Library> {
        Some(Library { stronghold: StrongholdPiece::load(STRONGHOLD_LIBRARY, tag)?, is_tall: boolean(tag, "Tall") })
    }

    fn add_additional_save_data(&self, tag: &mut Vec<(String, Nbt)>) {
        self.stronghold.add_additional_save_data(tag);
        tag.push(("Tall".into(), Nbt::Byte(self.is_tall as i8)));
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
    ) -> Option<Library> {
        let mut bounding_box = BoundingBox::orient_box(foot_x, foot_y, foot_z, -4, -1, 0, 14, 11, 15, direction);
        if !is_free_box(builder, &bounding_box) {
            bounding_box = BoundingBox::orient_box(foot_x, foot_y, foot_z, -4, -1, 0, 14, 6, 15, direction);
            if !is_free_box(builder, &bounding_box) {
                return None;
            }
        }
        Some(Library::new(gen_depth, random, bounding_box, direction))
    }

    fn post_process(&self, level: &mut impl WorldGenLevel, random: &mut impl RandomSource, chunk_bb: &BoundingBox) {
        let stronghold = &self.stronghold;
        let piece = &stronghold.piece;
        let current_height = if self.is_tall { 11 } else { 6 };
        let planks = blocks::OAK_PLANKS.default_state();
        let bookshelf = blocks::BOOKSHELF.default_state();
        piece.generate_box_with_selector(level, chunk_bb, 0, 0, 0, 13, current_height - 1, 14, true, random, &mut SmoothStoneSelector::new());
        stronghold.generate_small_door(level, chunk_bb, stronghold.entry_door, 4, 1, 0);
        let cobweb = blocks::COBWEB.default_state();
        piece.generate_maybe_box(level, chunk_bb, random, 0.07, 2, 1, 1, 11, 4, 13, cobweb, cobweb, false, false);
        for d in 1..=13 {
            if (d - 1) % 4 == 0 {
                piece.generate_box(level, chunk_bb, 1, 1, d, 1, 4, d, planks, planks, false);
                piece.generate_box(level, chunk_bb, 12, 1, d, 12, 4, d, planks, planks, false);
                piece.place_block(level, blocks::WALL_TORCH.default_state().set(HORIZONTAL_FACING, Direction::East), 2, 3, d, chunk_bb);
                piece.place_block(level, blocks::WALL_TORCH.default_state().set(HORIZONTAL_FACING, Direction::West), 11, 3, d, chunk_bb);
                if self.is_tall {
                    piece.generate_box(level, chunk_bb, 1, 6, d, 1, 9, d, planks, planks, false);
                    piece.generate_box(level, chunk_bb, 12, 6, d, 12, 9, d, planks, planks, false);
                }
            } else {
                piece.generate_box(level, chunk_bb, 1, 1, d, 1, 4, d, bookshelf, bookshelf, false);
                piece.generate_box(level, chunk_bb, 12, 1, d, 12, 4, d, bookshelf, bookshelf, false);
                if self.is_tall {
                    piece.generate_box(level, chunk_bb, 1, 6, d, 1, 9, d, bookshelf, bookshelf, false);
                    piece.generate_box(level, chunk_bb, 12, 6, d, 12, 9, d, bookshelf, bookshelf, false);
                }
            }
        }
        for d in (3..12).step_by(2) {
            piece.generate_box(level, chunk_bb, 3, 1, d, 4, 3, d, bookshelf, bookshelf, false);
            piece.generate_box(level, chunk_bb, 6, 1, d, 7, 3, d, bookshelf, bookshelf, false);
            piece.generate_box(level, chunk_bb, 9, 1, d, 10, 3, d, bookshelf, bookshelf, false);
        }
        if self.is_tall {
            piece.generate_box(level, chunk_bb, 1, 5, 1, 3, 5, 13, planks, planks, false);
            piece.generate_box(level, chunk_bb, 10, 5, 1, 12, 5, 13, planks, planks, false);
            piece.generate_box(level, chunk_bb, 4, 5, 1, 9, 5, 2, planks, planks, false);
            piece.generate_box(level, chunk_bb, 4, 5, 12, 9, 5, 13, planks, planks, false);
            piece.place_block(level, planks, 9, 5, 11, chunk_bb);
            piece.place_block(level, planks, 8, 5, 11, chunk_bb);
            piece.place_block(level, planks, 9, 5, 10, chunk_bb);
            let fence = blocks::OAK_FENCE.default_state();
            let we_fence = fence.set(WEST, true).set(EAST, true);
            let ns_fence = fence.set(NORTH, true).set(SOUTH, true);
            piece.generate_box(level, chunk_bb, 3, 6, 3, 3, 6, 11, ns_fence, ns_fence, false);
            piece.generate_box(level, chunk_bb, 10, 6, 3, 10, 6, 9, ns_fence, ns_fence, false);
            piece.generate_box(level, chunk_bb, 4, 6, 2, 9, 6, 2, we_fence, we_fence, false);
            piece.generate_box(level, chunk_bb, 4, 6, 12, 7, 6, 12, we_fence, we_fence, false);
            piece.place_block(level, fence.set(NORTH, true).set(EAST, true), 3, 6, 2, chunk_bb);
            piece.place_block(level, fence.set(SOUTH, true).set(EAST, true), 3, 6, 12, chunk_bb);
            piece.place_block(level, fence.set(NORTH, true).set(WEST, true), 10, 6, 2, chunk_bb);
            for i in 0..=2 {
                piece.place_block(level, fence.set(SOUTH, true).set(WEST, true), 8 + i, 6, 12 - i, chunk_bb);
                if i != 2 {
                    piece.place_block(level, fence.set(NORTH, true).set(EAST, true), 8 + i, 6, 11 - i, chunk_bb);
                }
            }
            let ladder = blocks::LADDER.default_state().set(HORIZONTAL_FACING, Direction::South);
            for y in 1..=7 {
                piece.place_block(level, ladder, 10, y, 13, chunk_bb);
            }
            let e_fence = fence.set(EAST, true);
            piece.place_block(level, e_fence, 6, 9, 7, chunk_bb);
            let w_fence = fence.set(WEST, true);
            piece.place_block(level, w_fence, 7, 9, 7, chunk_bb);
            piece.place_block(level, e_fence, 6, 8, 7, chunk_bb);
            piece.place_block(level, w_fence, 7, 8, 7, chunk_bb);
            let nswe_fence = ns_fence.set(WEST, true).set(EAST, true);
            piece.place_block(level, nswe_fence, 6, 7, 7, chunk_bb);
            piece.place_block(level, nswe_fence, 7, 7, 7, chunk_bb);
            piece.place_block(level, e_fence, 5, 7, 7, chunk_bb);
            piece.place_block(level, w_fence, 8, 7, 7, chunk_bb);
            piece.place_block(level, e_fence.set(NORTH, true), 6, 7, 6, chunk_bb);
            piece.place_block(level, e_fence.set(SOUTH, true), 6, 7, 8, chunk_bb);
            piece.place_block(level, w_fence.set(NORTH, true), 7, 7, 6, chunk_bb);
            piece.place_block(level, w_fence.set(SOUTH, true), 7, 7, 8, chunk_bb);
            let torch = blocks::TORCH.default_state();
            piece.place_block(level, torch, 5, 8, 7, chunk_bb);
            piece.place_block(level, torch, 8, 8, 7, chunk_bb);
            piece.place_block(level, torch, 6, 8, 6, chunk_bb);
            piece.place_block(level, torch, 6, 8, 8, chunk_bb);
            piece.place_block(level, torch, 7, 8, 6, chunk_bb);
            piece.place_block(level, torch, 7, 8, 8, chunk_bb);
        }
        piece.create_chest(level, chunk_bb, random, piece.get_world_pos(3, 3, 5), STRONGHOLD_LIBRARY_LOOT, None);
        if self.is_tall {
            piece.place_block(level, cave_air(), 12, 9, 1, chunk_bb);
            piece.create_chest(level, chunk_bb, random, piece.get_world_pos(12, 8, 1), STRONGHOLD_LIBRARY_LOOT, None);
        }
    }
}

#[derive(Clone)]
pub struct PortalRoom {
    pub stronghold: StrongholdPiece,
    has_placed_spawner: bool,
}

impl PortalRoom {
    fn new(gen_depth: i32, bounding_box: BoundingBox, direction: Direction) -> PortalRoom {
        PortalRoom { stronghold: StrongholdPiece::oriented(STRONGHOLD_PORTAL_ROOM, gen_depth, bounding_box, direction), has_placed_spawner: false }
    }

    fn load(tag: &Nbt) -> Option<PortalRoom> {
        Some(PortalRoom { stronghold: StrongholdPiece::load(STRONGHOLD_PORTAL_ROOM, tag)?, has_placed_spawner: boolean(tag, "Mob") })
    }

    fn add_additional_save_data(&self, tag: &mut Vec<(String, Nbt)>) {
        self.stronghold.add_additional_save_data(tag);
        tag.push(("Mob".into(), Nbt::Byte(self.has_placed_spawner as i8)));
    }

    fn add_children(&self, start_piece: &mut StartPiece) {
        start_piece.portal_room_piece = Some(self.clone());
    }

    fn create_piece(builder: &StructurePiecesBuilder, foot_x: i32, foot_y: i32, foot_z: i32, direction: Direction, gen_depth: i32) -> Option<PortalRoom> {
        let bounding_box = BoundingBox::orient_box(foot_x, foot_y, foot_z, -4, -1, 0, 11, 8, 16, direction);
        is_free_box(builder, &bounding_box).then(|| PortalRoom::new(gen_depth, bounding_box, direction))
    }

    fn post_process(&mut self, level: &mut impl WorldGenLevel, random: &mut impl RandomSource, chunk_bb: &BoundingBox) {
        let stronghold = &self.stronghold;
        let piece = &stronghold.piece;
        let selector = &mut SmoothStoneSelector::new();
        let lava = blocks::LAVA.default_state();
        piece.generate_box_with_selector(level, chunk_bb, 0, 0, 0, 10, 7, 15, false, random, selector);
        stronghold.generate_small_door(level, chunk_bb, SmallDoorType::Grates, 4, 1, 0);
        piece.generate_box_with_selector(level, chunk_bb, 1, 6, 1, 1, 6, 14, false, random, selector);
        piece.generate_box_with_selector(level, chunk_bb, 9, 6, 1, 9, 6, 14, false, random, selector);
        piece.generate_box_with_selector(level, chunk_bb, 2, 6, 1, 8, 6, 2, false, random, selector);
        piece.generate_box_with_selector(level, chunk_bb, 2, 6, 14, 8, 6, 14, false, random, selector);
        piece.generate_box_with_selector(level, chunk_bb, 1, 1, 1, 2, 1, 4, false, random, selector);
        piece.generate_box_with_selector(level, chunk_bb, 8, 1, 1, 9, 1, 4, false, random, selector);
        piece.generate_box(level, chunk_bb, 1, 1, 1, 1, 1, 3, lava, lava, false);
        piece.generate_box(level, chunk_bb, 9, 1, 1, 9, 1, 3, lava, lava, false);
        piece.generate_box_with_selector(level, chunk_bb, 3, 1, 8, 7, 1, 12, false, random, selector);
        piece.generate_box(level, chunk_bb, 4, 1, 9, 6, 1, 11, lava, lava, false);
        let ns_bars = blocks::IRON_BARS.default_state().set(NORTH, true).set(SOUTH, true);
        let we_bars = blocks::IRON_BARS.default_state().set(WEST, true).set(EAST, true);
        for z in (3..14).step_by(2) {
            piece.generate_box(level, chunk_bb, 0, 3, z, 0, 4, z, ns_bars, ns_bars, false);
            piece.generate_box(level, chunk_bb, 10, 3, z, 10, 4, z, ns_bars, ns_bars, false);
        }
        for x in (2..9).step_by(2) {
            piece.generate_box(level, chunk_bb, x, 3, 15, x, 4, 15, we_bars, we_bars, false);
        }
        let stairs = blocks::STONE_BRICK_STAIRS.default_state().set(HORIZONTAL_FACING, Direction::North);
        piece.generate_box_with_selector(level, chunk_bb, 4, 1, 5, 6, 1, 7, false, random, selector);
        piece.generate_box_with_selector(level, chunk_bb, 4, 2, 6, 6, 2, 7, false, random, selector);
        piece.generate_box_with_selector(level, chunk_bb, 4, 3, 7, 6, 3, 7, false, random, selector);
        for x in 4..=6 {
            piece.place_block(level, stairs, x, 1, 4, chunk_bb);
            piece.place_block(level, stairs, x, 2, 5, chunk_bb);
            piece.place_block(level, stairs, x, 3, 6, chunk_bb);
        }
        let frame = blocks::END_PORTAL_FRAME.default_state();
        let north_frame = frame.set(HORIZONTAL_FACING, Direction::North);
        let south_frame = frame.set(HORIZONTAL_FACING, Direction::South);
        let east_frame = frame.set(HORIZONTAL_FACING, Direction::East);
        let west_frame = frame.set(HORIZONTAL_FACING, Direction::West);
        let mut all_eyes = true;
        let mut eyes = [false; 12];
        for eye in &mut eyes {
            *eye = random.next_float() > 0.9;
            all_eyes &= *eye;
        }
        piece.place_block(level, north_frame.set(EYE, eyes[0]), 4, 3, 8, chunk_bb);
        piece.place_block(level, north_frame.set(EYE, eyes[1]), 5, 3, 8, chunk_bb);
        piece.place_block(level, north_frame.set(EYE, eyes[2]), 6, 3, 8, chunk_bb);
        piece.place_block(level, south_frame.set(EYE, eyes[3]), 4, 3, 12, chunk_bb);
        piece.place_block(level, south_frame.set(EYE, eyes[4]), 5, 3, 12, chunk_bb);
        piece.place_block(level, south_frame.set(EYE, eyes[5]), 6, 3, 12, chunk_bb);
        piece.place_block(level, east_frame.set(EYE, eyes[6]), 3, 3, 9, chunk_bb);
        piece.place_block(level, east_frame.set(EYE, eyes[7]), 3, 3, 10, chunk_bb);
        piece.place_block(level, east_frame.set(EYE, eyes[8]), 3, 3, 11, chunk_bb);
        piece.place_block(level, west_frame.set(EYE, eyes[9]), 7, 3, 9, chunk_bb);
        piece.place_block(level, west_frame.set(EYE, eyes[10]), 7, 3, 10, chunk_bb);
        piece.place_block(level, west_frame.set(EYE, eyes[11]), 7, 3, 11, chunk_bb);
        if all_eyes {
            let portal = blocks::END_PORTAL.default_state();
            for z in 9..=11 {
                for x in 4..=6 {
                    piece.place_block(level, portal, x, 3, z, chunk_bb);
                }
            }
        }
        if !self.has_placed_spawner {
            let pos = piece.get_world_pos(5, 3, 6);
            if chunk_bb.is_inside(pos) {
                self.has_placed_spawner = true;
                level.set_block(pos, blocks::SPAWNER.default_state(), 2);
                if let Some(entity) = level.get_block_entity(pos)
                    && let BlockEntityKind::Spawner(spawner) = &mut entity.kind
                {
                    spawner.spawner.set_entity_id("minecraft:silverfish", random);
                }
            }
        }
    }
}

#[derive(Clone)]
pub struct PrisonHall {
    pub stronghold: StrongholdPiece,
}

impl PrisonHall {
    fn new(gen_depth: i32, random: &mut impl RandomSource, bounding_box: BoundingBox, direction: Direction) -> PrisonHall {
        let mut stronghold = StrongholdPiece::oriented(STRONGHOLD_PRISON_HALL, gen_depth, bounding_box, direction);
        stronghold.entry_door = StrongholdPiece::random_small_door(random);
        PrisonHall { stronghold }
    }

    fn load(tag: &Nbt) -> Option<PrisonHall> {
        Some(PrisonHall { stronghold: StrongholdPiece::load(STRONGHOLD_PRISON_HALL, tag)? })
    }

    fn add_children(&self, pieces: &mut StrongholdPieces, start_piece: &mut StartPiece, builder: &mut StructurePiecesBuilder, random: &mut impl RandomSource) {
        self.stronghold.generate_small_door_child_forward(pieces, start_piece, builder, random, 1, 1);
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
    ) -> Option<PrisonHall> {
        let bounding_box = BoundingBox::orient_box(foot_x, foot_y, foot_z, -1, -1, 0, 9, 5, 11, direction);
        is_free_box(builder, &bounding_box).then(|| PrisonHall::new(gen_depth, random, bounding_box, direction))
    }

    fn post_process(&self, level: &mut impl WorldGenLevel, random: &mut impl RandomSource, chunk_bb: &BoundingBox) {
        let stronghold = &self.stronghold;
        let piece = &stronghold.piece;
        let selector = &mut SmoothStoneSelector::new();
        piece.generate_box_with_selector(level, chunk_bb, 0, 0, 0, 8, 4, 10, true, random, selector);
        stronghold.generate_small_door(level, chunk_bb, stronghold.entry_door, 1, 1, 0);
        piece.generate_box(level, chunk_bb, 1, 1, 10, 3, 3, 10, cave_air(), cave_air(), false);
        piece.generate_box_with_selector(level, chunk_bb, 4, 1, 1, 4, 3, 1, false, random, selector);
        piece.generate_box_with_selector(level, chunk_bb, 4, 1, 3, 4, 3, 3, false, random, selector);
        piece.generate_box_with_selector(level, chunk_bb, 4, 1, 7, 4, 3, 7, false, random, selector);
        piece.generate_box_with_selector(level, chunk_bb, 4, 1, 9, 4, 3, 9, false, random, selector);
        let bars = blocks::IRON_BARS.default_state();
        let ns_bars = bars.set(NORTH, true).set(SOUTH, true);
        let we_bars = bars.set(WEST, true).set(EAST, true);
        for y in 1..=3 {
            piece.place_block(level, ns_bars, 4, y, 4, chunk_bb);
            piece.place_block(level, ns_bars.set(EAST, true), 4, y, 5, chunk_bb);
            piece.place_block(level, ns_bars, 4, y, 6, chunk_bb);
            piece.place_block(level, we_bars, 5, y, 5, chunk_bb);
            piece.place_block(level, we_bars, 6, y, 5, chunk_bb);
            piece.place_block(level, we_bars, 7, y, 5, chunk_bb);
        }
        piece.place_block(level, ns_bars, 4, 3, 2, chunk_bb);
        piece.place_block(level, ns_bars, 4, 3, 8, chunk_bb);
        let door_bottom = blocks::IRON_DOOR.default_state().set(HORIZONTAL_FACING, Direction::West);
        let door_top = door_bottom.set(DOUBLE_BLOCK_HALF, DoubleBlockHalf::Upper);
        piece.place_block(level, door_bottom, 4, 1, 2, chunk_bb);
        piece.place_block(level, door_top, 4, 2, 2, chunk_bb);
        piece.place_block(level, door_bottom, 4, 1, 8, chunk_bb);
        piece.place_block(level, door_top, 4, 2, 8, chunk_bb);
    }
}

#[derive(Clone)]
pub struct RightTurn {
    pub turn: Turn,
}

impl RightTurn {
    fn new(gen_depth: i32, random: &mut impl RandomSource, bounding_box: BoundingBox, direction: Direction) -> RightTurn {
        let mut stronghold = StrongholdPiece::oriented(STRONGHOLD_RIGHT_TURN, gen_depth, bounding_box, direction);
        stronghold.entry_door = StrongholdPiece::random_small_door(random);
        RightTurn { turn: Turn { stronghold } }
    }

    fn load(tag: &Nbt) -> Option<RightTurn> {
        Some(RightTurn { turn: Turn { stronghold: StrongholdPiece::load(STRONGHOLD_RIGHT_TURN, tag)? } })
    }

    fn add_children(&self, pieces: &mut StrongholdPieces, start_piece: &mut StartPiece, builder: &mut StructurePiecesBuilder, random: &mut impl RandomSource) {
        let stronghold = &self.turn.stronghold;
        let orientation = stronghold.piece.get_orientation();
        if orientation != Some(Direction::North) && orientation != Some(Direction::East) {
            stronghold.generate_small_door_child_left(pieces, start_piece, builder, random, 1, 1);
        } else {
            stronghold.generate_small_door_child_right(pieces, start_piece, builder, random, 1, 1);
        }
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
    ) -> Option<RightTurn> {
        let bounding_box = BoundingBox::orient_box(foot_x, foot_y, foot_z, -1, -1, 0, 5, 5, 5, direction);
        is_free_box(builder, &bounding_box).then(|| RightTurn::new(gen_depth, random, bounding_box, direction))
    }

    fn post_process(&self, level: &mut impl WorldGenLevel, random: &mut impl RandomSource, chunk_bb: &BoundingBox) {
        let stronghold = &self.turn.stronghold;
        let piece = &stronghold.piece;
        piece.generate_box_with_selector(level, chunk_bb, 0, 0, 0, 4, 4, 4, true, random, &mut SmoothStoneSelector::new());
        stronghold.generate_small_door(level, chunk_bb, stronghold.entry_door, 1, 1, 0);
        let orientation = piece.get_orientation();
        if orientation != Some(Direction::North) && orientation != Some(Direction::East) {
            piece.generate_box(level, chunk_bb, 0, 1, 1, 0, 3, 3, cave_air(), cave_air(), false);
        } else {
            piece.generate_box(level, chunk_bb, 4, 1, 1, 4, 3, 3, cave_air(), cave_air(), false);
        }
    }
}

#[derive(Clone)]
pub struct RoomCrossing {
    pub stronghold: StrongholdPiece,
    room_type: i32,
}

impl RoomCrossing {
    fn new(gen_depth: i32, random: &mut impl RandomSource, bounding_box: BoundingBox, direction: Direction) -> RoomCrossing {
        let mut stronghold = StrongholdPiece::oriented(STRONGHOLD_ROOM_CROSSING, gen_depth, bounding_box, direction);
        stronghold.entry_door = StrongholdPiece::random_small_door(random);
        RoomCrossing { stronghold, room_type: random.next_int_bounded(5) }
    }

    fn load(tag: &Nbt) -> Option<RoomCrossing> {
        Some(RoomCrossing { stronghold: StrongholdPiece::load(STRONGHOLD_ROOM_CROSSING, tag)?, room_type: int(tag, "Type") })
    }

    fn add_additional_save_data(&self, tag: &mut Vec<(String, Nbt)>) {
        self.stronghold.add_additional_save_data(tag);
        tag.push(("Type".into(), Nbt::Int(self.room_type)));
    }

    fn add_children(&self, pieces: &mut StrongholdPieces, start_piece: &mut StartPiece, builder: &mut StructurePiecesBuilder, random: &mut impl RandomSource) {
        self.stronghold.generate_small_door_child_forward(pieces, start_piece, builder, random, 4, 1);
        self.stronghold.generate_small_door_child_left(pieces, start_piece, builder, random, 1, 4);
        self.stronghold.generate_small_door_child_right(pieces, start_piece, builder, random, 1, 4);
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
    ) -> Option<RoomCrossing> {
        let bounding_box = BoundingBox::orient_box(foot_x, foot_y, foot_z, -4, -1, 0, 11, 7, 11, direction);
        is_free_box(builder, &bounding_box).then(|| RoomCrossing::new(gen_depth, random, bounding_box, direction))
    }

    fn post_process(&self, level: &mut impl WorldGenLevel, random: &mut impl RandomSource, chunk_bb: &BoundingBox) {
        let stronghold = &self.stronghold;
        let piece = &stronghold.piece;
        let stone_bricks = blocks::STONE_BRICKS.default_state();
        let slab = blocks::SMOOTH_STONE_SLAB.default_state();
        let cobblestone = blocks::COBBLESTONE.default_state();
        let planks = blocks::OAK_PLANKS.default_state();
        let wall_torch = blocks::WALL_TORCH.default_state();
        piece.generate_box_with_selector(level, chunk_bb, 0, 0, 0, 10, 6, 10, true, random, &mut SmoothStoneSelector::new());
        stronghold.generate_small_door(level, chunk_bb, stronghold.entry_door, 4, 1, 0);
        piece.generate_box(level, chunk_bb, 4, 1, 10, 6, 3, 10, cave_air(), cave_air(), false);
        piece.generate_box(level, chunk_bb, 0, 1, 4, 0, 3, 6, cave_air(), cave_air(), false);
        piece.generate_box(level, chunk_bb, 10, 1, 4, 10, 3, 6, cave_air(), cave_air(), false);
        match self.room_type {
            0 => {
                piece.place_block(level, stone_bricks, 5, 1, 5, chunk_bb);
                piece.place_block(level, stone_bricks, 5, 2, 5, chunk_bb);
                piece.place_block(level, stone_bricks, 5, 3, 5, chunk_bb);
                piece.place_block(level, wall_torch.set(HORIZONTAL_FACING, Direction::West), 4, 3, 5, chunk_bb);
                piece.place_block(level, wall_torch.set(HORIZONTAL_FACING, Direction::East), 6, 3, 5, chunk_bb);
                piece.place_block(level, wall_torch.set(HORIZONTAL_FACING, Direction::South), 5, 3, 4, chunk_bb);
                piece.place_block(level, wall_torch.set(HORIZONTAL_FACING, Direction::North), 5, 3, 6, chunk_bb);
                piece.place_block(level, slab, 4, 1, 4, chunk_bb);
                piece.place_block(level, slab, 4, 1, 5, chunk_bb);
                piece.place_block(level, slab, 4, 1, 6, chunk_bb);
                piece.place_block(level, slab, 6, 1, 4, chunk_bb);
                piece.place_block(level, slab, 6, 1, 5, chunk_bb);
                piece.place_block(level, slab, 6, 1, 6, chunk_bb);
                piece.place_block(level, slab, 5, 1, 4, chunk_bb);
                piece.place_block(level, slab, 5, 1, 6, chunk_bb);
            }
            1 => {
                for i in 0..5 {
                    piece.place_block(level, stone_bricks, 3, 1, 3 + i, chunk_bb);
                    piece.place_block(level, stone_bricks, 7, 1, 3 + i, chunk_bb);
                    piece.place_block(level, stone_bricks, 3 + i, 1, 3, chunk_bb);
                    piece.place_block(level, stone_bricks, 3 + i, 1, 7, chunk_bb);
                }
                piece.place_block(level, stone_bricks, 5, 1, 5, chunk_bb);
                piece.place_block(level, stone_bricks, 5, 2, 5, chunk_bb);
                piece.place_block(level, stone_bricks, 5, 3, 5, chunk_bb);
                piece.place_block(level, blocks::WATER.default_state(), 5, 4, 5, chunk_bb);
            }
            2 => {
                for z in 1..=9 {
                    piece.place_block(level, cobblestone, 1, 3, z, chunk_bb);
                    piece.place_block(level, cobblestone, 9, 3, z, chunk_bb);
                }
                for x in 1..=9 {
                    piece.place_block(level, cobblestone, x, 3, 1, chunk_bb);
                    piece.place_block(level, cobblestone, x, 3, 9, chunk_bb);
                }
                piece.place_block(level, cobblestone, 5, 1, 4, chunk_bb);
                piece.place_block(level, cobblestone, 5, 1, 6, chunk_bb);
                piece.place_block(level, cobblestone, 5, 3, 4, chunk_bb);
                piece.place_block(level, cobblestone, 5, 3, 6, chunk_bb);
                piece.place_block(level, cobblestone, 4, 1, 5, chunk_bb);
                piece.place_block(level, cobblestone, 6, 1, 5, chunk_bb);
                piece.place_block(level, cobblestone, 4, 3, 5, chunk_bb);
                piece.place_block(level, cobblestone, 6, 3, 5, chunk_bb);
                for y in 1..=3 {
                    piece.place_block(level, cobblestone, 4, y, 4, chunk_bb);
                    piece.place_block(level, cobblestone, 6, y, 4, chunk_bb);
                    piece.place_block(level, cobblestone, 4, y, 6, chunk_bb);
                    piece.place_block(level, cobblestone, 6, y, 6, chunk_bb);
                }
                piece.place_block(level, wall_torch, 5, 3, 5, chunk_bb);
                for z in 2..=8 {
                    piece.place_block(level, planks, 2, 3, z, chunk_bb);
                    piece.place_block(level, planks, 3, 3, z, chunk_bb);
                    if z <= 3 || z >= 7 {
                        piece.place_block(level, planks, 4, 3, z, chunk_bb);
                        piece.place_block(level, planks, 5, 3, z, chunk_bb);
                        piece.place_block(level, planks, 6, 3, z, chunk_bb);
                    }
                    piece.place_block(level, planks, 7, 3, z, chunk_bb);
                    piece.place_block(level, planks, 8, 3, z, chunk_bb);
                }
                let ladder = blocks::LADDER.default_state().set(HORIZONTAL_FACING, Direction::West);
                piece.place_block(level, ladder, 9, 1, 3, chunk_bb);
                piece.place_block(level, ladder, 9, 2, 3, chunk_bb);
                piece.place_block(level, ladder, 9, 3, 3, chunk_bb);
                piece.create_chest(level, chunk_bb, random, piece.get_world_pos(3, 4, 8), STRONGHOLD_CROSSING_LOOT, None);
            }
            _ => {}
        }
    }
}

#[derive(Clone)]
pub struct StairsDown {
    pub stronghold: StrongholdPiece,
    is_source: bool,
}

impl StairsDown {
    fn new_source(piece_type: &'static str, gen_depth: i32, west: i32, north: i32, direction: Direction) -> StairsDown {
        let stronghold = StrongholdPiece::oriented(piece_type, gen_depth, make_bounding_box(west, MAGIC_START_Y, north, direction, 5, 11, 5), direction);
        StairsDown { stronghold, is_source: true }
    }

    fn new(gen_depth: i32, random: &mut impl RandomSource, bounding_box: BoundingBox, direction: Direction) -> StairsDown {
        let mut stronghold = StrongholdPiece::oriented(STRONGHOLD_STAIRS_DOWN, gen_depth, bounding_box, direction);
        stronghold.entry_door = StrongholdPiece::random_small_door(random);
        StairsDown { stronghold, is_source: false }
    }

    fn load(piece_type: &'static str, tag: &Nbt) -> Option<StairsDown> {
        Some(StairsDown { stronghold: StrongholdPiece::load(piece_type, tag)?, is_source: boolean(tag, "Source") })
    }

    fn add_additional_save_data(&self, tag: &mut Vec<(String, Nbt)>) {
        self.stronghold.add_additional_save_data(tag);
        tag.push(("Source".into(), Nbt::Byte(self.is_source as i8)));
    }

    pub fn add_children(&self, pieces: &mut StrongholdPieces, start_piece: &mut StartPiece, builder: &mut StructurePiecesBuilder, random: &mut impl RandomSource) {
        if self.is_source {
            pieces.imposed_piece = Some(PieceClass::FiveCrossing);
        }
        self.stronghold.generate_small_door_child_forward(pieces, start_piece, builder, random, 1, 1);
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
    ) -> Option<StairsDown> {
        let bounding_box = BoundingBox::orient_box(foot_x, foot_y, foot_z, -1, -7, 0, 5, 11, 5, direction);
        is_free_box(builder, &bounding_box).then(|| StairsDown::new(gen_depth, random, bounding_box, direction))
    }

    fn post_process(&self, level: &mut impl WorldGenLevel, random: &mut impl RandomSource, chunk_bb: &BoundingBox) {
        let stronghold = &self.stronghold;
        let piece = &stronghold.piece;
        let stone_bricks = blocks::STONE_BRICKS.default_state();
        let slab = blocks::SMOOTH_STONE_SLAB.default_state();
        piece.generate_box_with_selector(level, chunk_bb, 0, 0, 0, 4, 10, 4, true, random, &mut SmoothStoneSelector::new());
        stronghold.generate_small_door(level, chunk_bb, stronghold.entry_door, 1, 7, 0);
        stronghold.generate_small_door(level, chunk_bb, SmallDoorType::Opening, 1, 1, 4);
        piece.place_block(level, stone_bricks, 2, 6, 1, chunk_bb);
        piece.place_block(level, stone_bricks, 1, 5, 1, chunk_bb);
        piece.place_block(level, slab, 1, 6, 1, chunk_bb);
        piece.place_block(level, stone_bricks, 1, 5, 2, chunk_bb);
        piece.place_block(level, stone_bricks, 1, 4, 3, chunk_bb);
        piece.place_block(level, slab, 1, 5, 3, chunk_bb);
        piece.place_block(level, stone_bricks, 2, 4, 3, chunk_bb);
        piece.place_block(level, stone_bricks, 3, 3, 3, chunk_bb);
        piece.place_block(level, slab, 3, 4, 3, chunk_bb);
        piece.place_block(level, stone_bricks, 3, 3, 2, chunk_bb);
        piece.place_block(level, stone_bricks, 3, 2, 1, chunk_bb);
        piece.place_block(level, slab, 3, 3, 1, chunk_bb);
        piece.place_block(level, stone_bricks, 2, 2, 1, chunk_bb);
        piece.place_block(level, stone_bricks, 1, 1, 1, chunk_bb);
        piece.place_block(level, slab, 1, 2, 1, chunk_bb);
        piece.place_block(level, stone_bricks, 1, 1, 2, chunk_bb);
        piece.place_block(level, slab, 1, 1, 3, chunk_bb);
    }
}

#[derive(Clone)]
pub struct StartPiece {
    pub stairs_down: StairsDown,
    previous_piece: Option<PieceClass>,
    pub portal_room_piece: Option<PortalRoom>,
    pub pending_children: Vec<StrongholdPieceKind>,
}

impl StartPiece {
    pub fn new(random: &mut impl RandomSource, west: i32, north: i32) -> StartPiece {
        let direction = get_random_horizontal_direction(random);
        StartPiece {
            stairs_down: StairsDown::new_source(STRONGHOLD_START, 0, west, north, direction),
            previous_piece: None,
            portal_room_piece: None,
            pending_children: Vec::new(),
        }
    }

    fn load(tag: &Nbt) -> Option<StartPiece> {
        Some(StartPiece { stairs_down: StairsDown::load(STRONGHOLD_START, tag)?, previous_piece: None, portal_room_piece: None, pending_children: Vec::new() })
    }
}

#[derive(Clone)]
pub struct Straight {
    pub stronghold: StrongholdPiece,
    left_child: bool,
    right_child: bool,
}

impl Straight {
    fn new(gen_depth: i32, random: &mut impl RandomSource, bounding_box: BoundingBox, direction: Direction) -> Straight {
        let mut stronghold = StrongholdPiece::oriented(STRONGHOLD_STRAIGHT, gen_depth, bounding_box, direction);
        stronghold.entry_door = StrongholdPiece::random_small_door(random);
        let left_child = random.next_int_bounded(2) == 0;
        let right_child = random.next_int_bounded(2) == 0;
        Straight { stronghold, left_child, right_child }
    }

    fn load(tag: &Nbt) -> Option<Straight> {
        Some(Straight { stronghold: StrongholdPiece::load(STRONGHOLD_STRAIGHT, tag)?, left_child: boolean(tag, "Left"), right_child: boolean(tag, "Right") })
    }

    fn add_additional_save_data(&self, tag: &mut Vec<(String, Nbt)>) {
        self.stronghold.add_additional_save_data(tag);
        tag.push(("Left".into(), Nbt::Byte(self.left_child as i8)));
        tag.push(("Right".into(), Nbt::Byte(self.right_child as i8)));
    }

    fn add_children(&self, pieces: &mut StrongholdPieces, start_piece: &mut StartPiece, builder: &mut StructurePiecesBuilder, random: &mut impl RandomSource) {
        self.stronghold.generate_small_door_child_forward(pieces, start_piece, builder, random, 1, 1);
        if self.left_child {
            self.stronghold.generate_small_door_child_left(pieces, start_piece, builder, random, 1, 2);
        }
        if self.right_child {
            self.stronghold.generate_small_door_child_right(pieces, start_piece, builder, random, 1, 2);
        }
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
    ) -> Option<Straight> {
        let bounding_box = BoundingBox::orient_box(foot_x, foot_y, foot_z, -1, -1, 0, 5, 5, 7, direction);
        is_free_box(builder, &bounding_box).then(|| Straight::new(gen_depth, random, bounding_box, direction))
    }

    fn post_process(&self, level: &mut impl WorldGenLevel, random: &mut impl RandomSource, chunk_bb: &BoundingBox) {
        let stronghold = &self.stronghold;
        let piece = &stronghold.piece;
        piece.generate_box_with_selector(level, chunk_bb, 0, 0, 0, 4, 4, 6, true, random, &mut SmoothStoneSelector::new());
        stronghold.generate_small_door(level, chunk_bb, stronghold.entry_door, 1, 1, 0);
        stronghold.generate_small_door(level, chunk_bb, SmallDoorType::Opening, 1, 1, 6);
        let east_torch = blocks::WALL_TORCH.default_state().set(HORIZONTAL_FACING, Direction::East);
        let west_torch = blocks::WALL_TORCH.default_state().set(HORIZONTAL_FACING, Direction::West);
        piece.maybe_generate_block(level, chunk_bb, random, 0.1, 1, 2, 1, east_torch);
        piece.maybe_generate_block(level, chunk_bb, random, 0.1, 3, 2, 1, west_torch);
        piece.maybe_generate_block(level, chunk_bb, random, 0.1, 1, 2, 5, east_torch);
        piece.maybe_generate_block(level, chunk_bb, random, 0.1, 3, 2, 5, west_torch);
        if self.left_child {
            piece.generate_box(level, chunk_bb, 0, 1, 2, 0, 3, 4, cave_air(), cave_air(), false);
        }
        if self.right_child {
            piece.generate_box(level, chunk_bb, 4, 1, 2, 4, 3, 4, cave_air(), cave_air(), false);
        }
    }
}

#[derive(Clone)]
pub struct StraightStairsDown {
    pub stronghold: StrongholdPiece,
}

impl StraightStairsDown {
    fn new(gen_depth: i32, random: &mut impl RandomSource, bounding_box: BoundingBox, direction: Direction) -> StraightStairsDown {
        let mut stronghold = StrongholdPiece::oriented(STRONGHOLD_STRAIGHT_STAIRS_DOWN, gen_depth, bounding_box, direction);
        stronghold.entry_door = StrongholdPiece::random_small_door(random);
        StraightStairsDown { stronghold }
    }

    fn load(tag: &Nbt) -> Option<StraightStairsDown> {
        Some(StraightStairsDown { stronghold: StrongholdPiece::load(STRONGHOLD_STRAIGHT_STAIRS_DOWN, tag)? })
    }

    fn add_children(&self, pieces: &mut StrongholdPieces, start_piece: &mut StartPiece, builder: &mut StructurePiecesBuilder, random: &mut impl RandomSource) {
        self.stronghold.generate_small_door_child_forward(pieces, start_piece, builder, random, 1, 1);
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
    ) -> Option<StraightStairsDown> {
        let bounding_box = BoundingBox::orient_box(foot_x, foot_y, foot_z, -1, -7, 0, 5, 11, 8, direction);
        is_free_box(builder, &bounding_box).then(|| StraightStairsDown::new(gen_depth, random, bounding_box, direction))
    }

    fn post_process(&self, level: &mut impl WorldGenLevel, random: &mut impl RandomSource, chunk_bb: &BoundingBox) {
        let stronghold = &self.stronghold;
        let piece = &stronghold.piece;
        piece.generate_box_with_selector(level, chunk_bb, 0, 0, 0, 4, 10, 7, true, random, &mut SmoothStoneSelector::new());
        stronghold.generate_small_door(level, chunk_bb, stronghold.entry_door, 1, 7, 0);
        stronghold.generate_small_door(level, chunk_bb, SmallDoorType::Opening, 1, 1, 7);
        let stairs = blocks::COBBLESTONE_STAIRS.default_state().set(HORIZONTAL_FACING, Direction::South);
        let stone_bricks = blocks::STONE_BRICKS.default_state();
        for i in 0..6 {
            piece.place_block(level, stairs, 1, 6 - i, 1 + i, chunk_bb);
            piece.place_block(level, stairs, 2, 6 - i, 1 + i, chunk_bb);
            piece.place_block(level, stairs, 3, 6 - i, 1 + i, chunk_bb);
            if i < 5 {
                piece.place_block(level, stone_bricks, 1, 5 - i, 1 + i, chunk_bb);
                piece.place_block(level, stone_bricks, 2, 5 - i, 1 + i, chunk_bb);
                piece.place_block(level, stone_bricks, 3, 5 - i, 1 + i, chunk_bb);
            }
        }
    }
}

#[derive(Clone)]
pub enum StrongholdPieceKind {
    ChestCorridor(ChestCorridor),
    FillerCorridor(FillerCorridor),
    FiveCrossing(FiveCrossing),
    LeftTurn(LeftTurn),
    Library(Library),
    PortalRoom(PortalRoom),
    PrisonHall(PrisonHall),
    RightTurn(RightTurn),
    RoomCrossing(RoomCrossing),
    StairsDown(StairsDown),
    Start(StartPiece),
    Straight(Straight),
    StraightStairsDown(StraightStairsDown),
}

impl StrongholdPieceKind {
    fn stronghold(&self) -> &StrongholdPiece {
        match self {
            StrongholdPieceKind::ChestCorridor(piece) => &piece.stronghold,
            StrongholdPieceKind::FillerCorridor(piece) => &piece.stronghold,
            StrongholdPieceKind::FiveCrossing(piece) => &piece.stronghold,
            StrongholdPieceKind::LeftTurn(piece) => &piece.turn.stronghold,
            StrongholdPieceKind::Library(piece) => &piece.stronghold,
            StrongholdPieceKind::PortalRoom(piece) => &piece.stronghold,
            StrongholdPieceKind::PrisonHall(piece) => &piece.stronghold,
            StrongholdPieceKind::RightTurn(piece) => &piece.turn.stronghold,
            StrongholdPieceKind::RoomCrossing(piece) => &piece.stronghold,
            StrongholdPieceKind::StairsDown(piece) => &piece.stronghold,
            StrongholdPieceKind::Start(piece) => &piece.stairs_down.stronghold,
            StrongholdPieceKind::Straight(piece) => &piece.stronghold,
            StrongholdPieceKind::StraightStairsDown(piece) => &piece.stronghold,
        }
    }

    fn stronghold_mut(&mut self) -> &mut StrongholdPiece {
        match self {
            StrongholdPieceKind::ChestCorridor(piece) => &mut piece.stronghold,
            StrongholdPieceKind::FillerCorridor(piece) => &mut piece.stronghold,
            StrongholdPieceKind::FiveCrossing(piece) => &mut piece.stronghold,
            StrongholdPieceKind::LeftTurn(piece) => &mut piece.turn.stronghold,
            StrongholdPieceKind::Library(piece) => &mut piece.stronghold,
            StrongholdPieceKind::PortalRoom(piece) => &mut piece.stronghold,
            StrongholdPieceKind::PrisonHall(piece) => &mut piece.stronghold,
            StrongholdPieceKind::RightTurn(piece) => &mut piece.turn.stronghold,
            StrongholdPieceKind::RoomCrossing(piece) => &mut piece.stronghold,
            StrongholdPieceKind::StairsDown(piece) => &mut piece.stronghold,
            StrongholdPieceKind::Start(piece) => &mut piece.stairs_down.stronghold,
            StrongholdPieceKind::Straight(piece) => &mut piece.stronghold,
            StrongholdPieceKind::StraightStairsDown(piece) => &mut piece.stronghold,
        }
    }

    pub fn piece(&self) -> &StructurePiece {
        &self.stronghold().piece
    }

    pub fn piece_mut(&mut self) -> &mut StructurePiece {
        &mut self.stronghold_mut().piece
    }

    pub fn load(piece_id: &str, tag: &Nbt) -> Option<StrongholdPieceKind> {
        Some(match piece_id {
            STRONGHOLD_CHEST_CORRIDOR => StrongholdPieceKind::ChestCorridor(ChestCorridor::load(tag)?),
            STRONGHOLD_FILLER_CORRIDOR => StrongholdPieceKind::FillerCorridor(FillerCorridor::load(tag)?),
            STRONGHOLD_FIVE_CROSSING => StrongholdPieceKind::FiveCrossing(FiveCrossing::load(tag)?),
            STRONGHOLD_LEFT_TURN => StrongholdPieceKind::LeftTurn(LeftTurn::load(tag)?),
            STRONGHOLD_LIBRARY => StrongholdPieceKind::Library(Library::load(tag)?),
            STRONGHOLD_PORTAL_ROOM => StrongholdPieceKind::PortalRoom(PortalRoom::load(tag)?),
            STRONGHOLD_PRISON_HALL => StrongholdPieceKind::PrisonHall(PrisonHall::load(tag)?),
            STRONGHOLD_RIGHT_TURN => StrongholdPieceKind::RightTurn(RightTurn::load(tag)?),
            STRONGHOLD_ROOM_CROSSING => StrongholdPieceKind::RoomCrossing(RoomCrossing::load(tag)?),
            STRONGHOLD_STAIRS_DOWN => StrongholdPieceKind::StairsDown(StairsDown::load(STRONGHOLD_STAIRS_DOWN, tag)?),
            STRONGHOLD_START => StrongholdPieceKind::Start(StartPiece::load(tag)?),
            STRONGHOLD_STRAIGHT => StrongholdPieceKind::Straight(Straight::load(tag)?),
            STRONGHOLD_STRAIGHT_STAIRS_DOWN => StrongholdPieceKind::StraightStairsDown(StraightStairsDown::load(tag)?),
            _ => return None,
        })
    }

    pub fn add_additional_save_data(&self, tag: &mut Vec<(String, Nbt)>) {
        match self {
            StrongholdPieceKind::ChestCorridor(piece) => piece.add_additional_save_data(tag),
            StrongholdPieceKind::FillerCorridor(piece) => piece.add_additional_save_data(tag),
            StrongholdPieceKind::FiveCrossing(piece) => piece.add_additional_save_data(tag),
            StrongholdPieceKind::Library(piece) => piece.add_additional_save_data(tag),
            StrongholdPieceKind::PortalRoom(piece) => piece.add_additional_save_data(tag),
            StrongholdPieceKind::RoomCrossing(piece) => piece.add_additional_save_data(tag),
            StrongholdPieceKind::StairsDown(piece) => piece.add_additional_save_data(tag),
            StrongholdPieceKind::Start(piece) => piece.stairs_down.add_additional_save_data(tag),
            StrongholdPieceKind::Straight(piece) => piece.add_additional_save_data(tag),
            StrongholdPieceKind::LeftTurn(_) | StrongholdPieceKind::PrisonHall(_) | StrongholdPieceKind::RightTurn(_) | StrongholdPieceKind::StraightStairsDown(_) => {
                self.stronghold().add_additional_save_data(tag)
            }
        }
    }

    pub fn add_children(&self, pieces: &mut StrongholdPieces, start_piece: &mut StartPiece, builder: &mut StructurePiecesBuilder, random: &mut impl RandomSource) {
        match self {
            StrongholdPieceKind::ChestCorridor(piece) => piece.add_children(pieces, start_piece, builder, random),
            StrongholdPieceKind::FiveCrossing(piece) => piece.add_children(pieces, start_piece, builder, random),
            StrongholdPieceKind::LeftTurn(piece) => piece.add_children(pieces, start_piece, builder, random),
            StrongholdPieceKind::PortalRoom(piece) => piece.add_children(start_piece),
            StrongholdPieceKind::PrisonHall(piece) => piece.add_children(pieces, start_piece, builder, random),
            StrongholdPieceKind::RightTurn(piece) => piece.add_children(pieces, start_piece, builder, random),
            StrongholdPieceKind::RoomCrossing(piece) => piece.add_children(pieces, start_piece, builder, random),
            StrongholdPieceKind::StairsDown(piece) => piece.add_children(pieces, start_piece, builder, random),
            StrongholdPieceKind::Start(piece) => piece.stairs_down.add_children(pieces, start_piece, builder, random),
            StrongholdPieceKind::Straight(piece) => piece.add_children(pieces, start_piece, builder, random),
            StrongholdPieceKind::StraightStairsDown(piece) => piece.add_children(pieces, start_piece, builder, random),
            StrongholdPieceKind::FillerCorridor(_) | StrongholdPieceKind::Library(_) => {}
        }
    }

    pub fn post_process(&mut self, level: &mut impl WorldGenLevel, random: &mut impl RandomSource, chunk_bb: &BoundingBox) {
        match self {
            StrongholdPieceKind::ChestCorridor(piece) => piece.post_process(level, random, chunk_bb),
            StrongholdPieceKind::FillerCorridor(piece) => piece.post_process(level, chunk_bb),
            StrongholdPieceKind::FiveCrossing(piece) => piece.post_process(level, random, chunk_bb),
            StrongholdPieceKind::LeftTurn(piece) => piece.post_process(level, random, chunk_bb),
            StrongholdPieceKind::Library(piece) => piece.post_process(level, random, chunk_bb),
            StrongholdPieceKind::PortalRoom(piece) => piece.post_process(level, random, chunk_bb),
            StrongholdPieceKind::PrisonHall(piece) => piece.post_process(level, random, chunk_bb),
            StrongholdPieceKind::RightTurn(piece) => piece.post_process(level, random, chunk_bb),
            StrongholdPieceKind::RoomCrossing(piece) => piece.post_process(level, random, chunk_bb),
            StrongholdPieceKind::StairsDown(piece) => piece.post_process(level, random, chunk_bb),
            StrongholdPieceKind::Start(piece) => piece.stairs_down.post_process(level, random, chunk_bb),
            StrongholdPieceKind::Straight(piece) => piece.post_process(level, random, chunk_bb),
            StrongholdPieceKind::StraightStairsDown(piece) => piece.post_process(level, random, chunk_bb),
        }
    }
}
