use crisp_core::Direction;
use crisp_core::random::RandomSource;
use crisp_data::{Block, BlockState, blocks};
use crisp_protocol::nbt::Nbt;

use crate::world::level::world_gen_level::WorldGenLevel;
use crate::util::shuffle;
use crate::world::level::levelgen::structure::BoundingBox;
use crate::world::level::levelgen::structure::structure_piece::{self, StructurePiece};

pub const OCEAN_MONUMENT_BUILDING: &str = "minecraft:omb";
const OCEAN_MONUMENT_CORE_ROOM: &str = "minecraft:omcr";
const OCEAN_MONUMENT_DOUBLE_X_ROOM: &str = "minecraft:omdxr";
const OCEAN_MONUMENT_DOUBLE_XY_ROOM: &str = "minecraft:omdxyr";
const OCEAN_MONUMENT_DOUBLE_Y_ROOM: &str = "minecraft:omdyr";
const OCEAN_MONUMENT_DOUBLE_YZ_ROOM: &str = "minecraft:omdyzr";
const OCEAN_MONUMENT_DOUBLE_Z_ROOM: &str = "minecraft:omdzr";
const OCEAN_MONUMENT_ENTRY_ROOM: &str = "minecraft:omentry";
const OCEAN_MONUMENT_PENTHOUSE: &str = "minecraft:ompenthouse";
const OCEAN_MONUMENT_SIMPLE_ROOM: &str = "minecraft:omsimple";
const OCEAN_MONUMENT_SIMPLE_TOP_ROOM: &str = "minecraft:omsimplet";
const OCEAN_MONUMENT_WING_ROOM: &str = "minecraft:omwr";

const DOWN: usize = Direction::Down.ordinal();
const UP: usize = Direction::Up.ordinal();
const NORTH: usize = Direction::North.ordinal();
const SOUTH: usize = Direction::South.ordinal();
const WEST: usize = Direction::West.ordinal();
const EAST: usize = Direction::East.ordinal();

fn base_gray() -> BlockState {
    blocks::PRISMARINE.default_state()
}

fn base_light() -> BlockState {
    blocks::PRISMARINE_BRICKS.default_state()
}

fn base_black() -> BlockState {
    blocks::DARK_PRISMARINE.default_state()
}

fn lamp_block() -> BlockState {
    blocks::SEA_LANTERN.default_state()
}

fn fill_block() -> BlockState {
    blocks::WATER.default_state()
}

const FILL_KEEP: [Block; 4] = [blocks::ICE, blocks::PACKED_ICE, blocks::BLUE_ICE, blocks::WATER];
const GRIDROOM_SOURCE_INDEX: i32 = get_room_index(2, 0, 0);
const GRIDROOM_TOP_CONNECT_INDEX: i32 = get_room_index(2, 2, 0);
const GRIDROOM_LEFTWING_CONNECT_INDEX: i32 = get_room_index(0, 1, 0);
const GRIDROOM_RIGHTWING_CONNECT_INDEX: i32 = get_room_index(4, 1, 0);
const LEFTWING_INDEX: i32 = 1001;
const RIGHTWING_INDEX: i32 = 1002;
const PENTHOUSE_INDEX: i32 = 1003;

const fn get_room_index(room_x: i32, room_y: i32, room_z: i32) -> i32 {
    room_y * 25 + room_z * 5 + room_x
}

#[derive(Clone, Copy)]
enum MonumentRoomFitter {
    FitDoubleXYRoom,
    FitDoubleYZRoom,
    FitDoubleZRoom,
    FitDoubleXRoom,
    FitDoubleYRoom,
    FitSimpleTopRoom,
    FitSimpleRoom,
}

impl MonumentRoomFitter {
    fn fits(self, rooms: &[RoomDefinition], definition: u8) -> bool {
        let definition = &rooms[definition as usize];
        match self {
            MonumentRoomFitter::FitDoubleXYRoom => {
                if definition.has_opening[EAST] && !definition.connection(rooms, EAST).claimed && definition.has_opening[UP] && !definition.connection(rooms, UP).claimed {
                    let east = definition.connection(rooms, EAST);
                    east.has_opening[UP] && !east.connection(rooms, UP).claimed
                } else {
                    false
                }
            }
            MonumentRoomFitter::FitDoubleYZRoom => {
                if definition.has_opening[NORTH] && !definition.connection(rooms, NORTH).claimed && definition.has_opening[UP] && !definition.connection(rooms, UP).claimed {
                    let north = definition.connection(rooms, NORTH);
                    north.has_opening[UP] && !north.connection(rooms, UP).claimed
                } else {
                    false
                }
            }
            MonumentRoomFitter::FitDoubleZRoom => definition.has_opening[NORTH] && !definition.connection(rooms, NORTH).claimed,
            MonumentRoomFitter::FitDoubleXRoom => definition.has_opening[EAST] && !definition.connection(rooms, EAST).claimed,
            MonumentRoomFitter::FitDoubleYRoom => definition.has_opening[UP] && !definition.connection(rooms, UP).claimed,
            MonumentRoomFitter::FitSimpleTopRoom => {
                !definition.has_opening[WEST] && !definition.has_opening[EAST] && !definition.has_opening[NORTH] && !definition.has_opening[SOUTH] && !definition.has_opening[UP]
            }
            MonumentRoomFitter::FitSimpleRoom => true,
        }
    }

    fn create(self, orientation: Direction, rooms: &mut [RoomDefinition], definition: u8, random: &mut impl RandomSource) -> MonumentChildPiece {
        let connection = |rooms: &[RoomDefinition], room: u8, direction: usize| rooms[room as usize].connections[direction].unwrap();
        match self {
            MonumentRoomFitter::FitDoubleXYRoom => {
                rooms[definition as usize].claimed = true;
                let east = connection(rooms, definition, EAST);
                rooms[east as usize].claimed = true;
                let up = connection(rooms, definition, UP);
                rooms[up as usize].claimed = true;
                let east_up = connection(rooms, east, UP);
                rooms[east_up as usize].claimed = true;
                MonumentChildPiece::DoubleXYRoom(OceanMonumentDoubleXYRoom::new(orientation, rooms, definition))
            }
            MonumentRoomFitter::FitDoubleYZRoom => {
                rooms[definition as usize].claimed = true;
                let north = connection(rooms, definition, NORTH);
                rooms[north as usize].claimed = true;
                let up = connection(rooms, definition, UP);
                rooms[up as usize].claimed = true;
                let north_up = connection(rooms, north, UP);
                rooms[north_up as usize].claimed = true;
                MonumentChildPiece::DoubleYZRoom(OceanMonumentDoubleYZRoom::new(orientation, rooms, definition))
            }
            MonumentRoomFitter::FitDoubleZRoom => {
                let mut source = definition;
                if !rooms[definition as usize].has_opening[NORTH] || rooms[connection(rooms, definition, NORTH) as usize].claimed {
                    source = connection(rooms, definition, SOUTH);
                }
                rooms[source as usize].claimed = true;
                let north = connection(rooms, source, NORTH);
                rooms[north as usize].claimed = true;
                MonumentChildPiece::DoubleZRoom(OceanMonumentDoubleZRoom::new(orientation, rooms, source))
            }
            MonumentRoomFitter::FitDoubleXRoom => {
                rooms[definition as usize].claimed = true;
                let east = connection(rooms, definition, EAST);
                rooms[east as usize].claimed = true;
                MonumentChildPiece::DoubleXRoom(OceanMonumentDoubleXRoom::new(orientation, rooms, definition))
            }
            MonumentRoomFitter::FitDoubleYRoom => {
                rooms[definition as usize].claimed = true;
                let up = connection(rooms, definition, UP);
                rooms[up as usize].claimed = true;
                MonumentChildPiece::DoubleYRoom(OceanMonumentDoubleYRoom::new(orientation, rooms, definition))
            }
            MonumentRoomFitter::FitSimpleTopRoom => {
                rooms[definition as usize].claimed = true;
                MonumentChildPiece::SimpleTopRoom(OceanMonumentSimpleTopRoom::new(orientation, rooms, definition))
            }
            MonumentRoomFitter::FitSimpleRoom => {
                rooms[definition as usize].claimed = true;
                MonumentChildPiece::SimpleRoom(OceanMonumentSimpleRoom::new(orientation, rooms, definition, random))
            }
        }
    }
}

const FITTERS: [MonumentRoomFitter; 7] = [
    MonumentRoomFitter::FitDoubleXYRoom,
    MonumentRoomFitter::FitDoubleYZRoom,
    MonumentRoomFitter::FitDoubleZRoom,
    MonumentRoomFitter::FitDoubleXRoom,
    MonumentRoomFitter::FitDoubleYRoom,
    MonumentRoomFitter::FitSimpleTopRoom,
    MonumentRoomFitter::FitSimpleRoom,
];

#[derive(Clone)]
enum MonumentChildPiece {
    CoreRoom(OceanMonumentCoreRoom),
    DoubleXRoom(OceanMonumentDoubleXRoom),
    DoubleXYRoom(OceanMonumentDoubleXYRoom),
    DoubleYRoom(OceanMonumentDoubleYRoom),
    DoubleYZRoom(OceanMonumentDoubleYZRoom),
    DoubleZRoom(OceanMonumentDoubleZRoom),
    EntryRoom(OceanMonumentEntryRoom),
    Penthouse(OceanMonumentPenthouse),
    SimpleRoom(OceanMonumentSimpleRoom),
    SimpleTopRoom(OceanMonumentSimpleTopRoom),
    WingRoom(OceanMonumentWingRoom),
}

impl MonumentChildPiece {
    fn monument_mut(&mut self) -> &mut OceanMonumentPiece {
        match self {
            MonumentChildPiece::CoreRoom(piece) => &mut piece.monument,
            MonumentChildPiece::DoubleXRoom(piece) => &mut piece.monument,
            MonumentChildPiece::DoubleXYRoom(piece) => &mut piece.monument,
            MonumentChildPiece::DoubleYRoom(piece) => &mut piece.monument,
            MonumentChildPiece::DoubleYZRoom(piece) => &mut piece.monument,
            MonumentChildPiece::DoubleZRoom(piece) => &mut piece.monument,
            MonumentChildPiece::EntryRoom(piece) => &mut piece.monument,
            MonumentChildPiece::Penthouse(piece) => &mut piece.monument,
            MonumentChildPiece::SimpleRoom(piece) => &mut piece.monument,
            MonumentChildPiece::SimpleTopRoom(piece) => &mut piece.monument,
            MonumentChildPiece::WingRoom(piece) => &mut piece.monument,
        }
    }

    fn get_bounding_box(&self) -> BoundingBox {
        match self {
            MonumentChildPiece::CoreRoom(piece) => piece.monument.piece.bounding_box,
            MonumentChildPiece::DoubleXRoom(piece) => piece.monument.piece.bounding_box,
            MonumentChildPiece::DoubleXYRoom(piece) => piece.monument.piece.bounding_box,
            MonumentChildPiece::DoubleYRoom(piece) => piece.monument.piece.bounding_box,
            MonumentChildPiece::DoubleYZRoom(piece) => piece.monument.piece.bounding_box,
            MonumentChildPiece::DoubleZRoom(piece) => piece.monument.piece.bounding_box,
            MonumentChildPiece::EntryRoom(piece) => piece.monument.piece.bounding_box,
            MonumentChildPiece::Penthouse(piece) => piece.monument.piece.bounding_box,
            MonumentChildPiece::SimpleRoom(piece) => piece.monument.piece.bounding_box,
            MonumentChildPiece::SimpleTopRoom(piece) => piece.monument.piece.bounding_box,
            MonumentChildPiece::WingRoom(piece) => piece.monument.piece.bounding_box,
        }
    }

    fn post_process(&self, level: &mut impl WorldGenLevel, random: &mut impl RandomSource, chunk_bb: &BoundingBox, rooms: &[RoomDefinition]) {
        match self {
            MonumentChildPiece::CoreRoom(piece) => piece.post_process(level, chunk_bb),
            MonumentChildPiece::DoubleXRoom(piece) => piece.post_process(level, chunk_bb, rooms),
            MonumentChildPiece::DoubleXYRoom(piece) => piece.post_process(level, chunk_bb, rooms),
            MonumentChildPiece::DoubleYRoom(piece) => piece.post_process(level, chunk_bb, rooms),
            MonumentChildPiece::DoubleYZRoom(piece) => piece.post_process(level, chunk_bb, rooms),
            MonumentChildPiece::DoubleZRoom(piece) => piece.post_process(level, chunk_bb, rooms),
            MonumentChildPiece::EntryRoom(piece) => piece.post_process(level, chunk_bb, rooms),
            MonumentChildPiece::Penthouse(piece) => piece.post_process(level, chunk_bb),
            MonumentChildPiece::SimpleRoom(piece) => piece.post_process(level, random, chunk_bb, rooms),
            MonumentChildPiece::SimpleTopRoom(piece) => piece.post_process(level, random, chunk_bb, rooms),
            MonumentChildPiece::WingRoom(piece) => piece.post_process(level, chunk_bb),
        }
    }
}

#[derive(Clone)]
pub struct MonumentBuilding {
    pub monument: OceanMonumentPiece,
    rooms: Vec<RoomDefinition>,
    child_pieces: Vec<MonumentChildPiece>,
}

impl MonumentBuilding {
    pub fn new(random: &mut impl RandomSource, west: i32, north: i32, direction: Direction) -> MonumentBuilding {
        let mut monument =
            OceanMonumentPiece::new(OCEAN_MONUMENT_BUILDING, direction, 0, structure_piece::make_bounding_box(west, 39, north, direction, 58, 23, 58));
        monument.piece.set_orientation(Some(direction));
        let mut rooms = Vec::new();
        let (room_definitions, source_room, core_room) = MonumentBuilding::generate_room_graph(&mut rooms, random);
        rooms[source_room as usize].claimed = true;
        let mut child_pieces = vec![
            MonumentChildPiece::EntryRoom(OceanMonumentEntryRoom::new(direction, &rooms, source_room)),
            MonumentChildPiece::CoreRoom(OceanMonumentCoreRoom::new(direction, &rooms, core_room)),
        ];
        for definition in room_definitions {
            if !rooms[definition as usize].claimed && !rooms[definition as usize].is_special() {
                for fitter in FITTERS {
                    if fitter.fits(&rooms, definition) {
                        child_pieces.push(fitter.create(direction, &mut rooms, definition, random));
                        break;
                    }
                }
            }
        }
        let piece = &monument.piece;
        let offset = piece.get_world_pos(9, 0, 22);
        for child in &mut child_pieces {
            child.monument_mut().piece.move_by(offset.x, offset.y, offset.z);
        }
        let left_wing = BoundingBox::from_corners(piece.get_world_pos(1, 1, 1), piece.get_world_pos(23, 8, 21));
        let right_wing = BoundingBox::from_corners(piece.get_world_pos(34, 1, 1), piece.get_world_pos(56, 8, 21));
        let penthouse = BoundingBox::from_corners(piece.get_world_pos(22, 13, 22), piece.get_world_pos(35, 17, 35));
        let mut wing_random = random.next_int();
        child_pieces.push(MonumentChildPiece::WingRoom(OceanMonumentWingRoom::new(direction, left_wing, wing_random)));
        wing_random = wing_random.wrapping_add(1);
        child_pieces.push(MonumentChildPiece::WingRoom(OceanMonumentWingRoom::new(direction, right_wing, wing_random)));
        child_pieces.push(MonumentChildPiece::Penthouse(OceanMonumentPenthouse::new(direction, penthouse)));
        MonumentBuilding { monument, rooms, child_pieces }
    }

    pub fn load(tag: &Nbt) -> Option<MonumentBuilding> {
        Some(MonumentBuilding { monument: OceanMonumentPiece::load(OCEAN_MONUMENT_BUILDING, tag)?, rooms: Vec::new(), child_pieces: Vec::new() })
    }

    fn generate_room_graph(rooms: &mut Vec<RoomDefinition>, random: &mut impl RandomSource) -> (Vec<u8>, u8, u8) {
        let mut room_grid: [Option<u8>; 75] = [None; 75];
        for x in 0..5 {
            for z in 0..4 {
                let pos = get_room_index(x, 0, z);
                room_grid[pos as usize] = Some(RoomDefinition::create(rooms, pos));
            }
        }
        for x in 0..5 {
            for z in 0..4 {
                let pos = get_room_index(x, 1, z);
                room_grid[pos as usize] = Some(RoomDefinition::create(rooms, pos));
            }
        }
        for x in 1..4 {
            for z in 0..2 {
                let pos = get_room_index(x, 2, z);
                room_grid[pos as usize] = Some(RoomDefinition::create(rooms, pos));
            }
        }
        let source_room = room_grid[GRIDROOM_SOURCE_INDEX as usize].unwrap();
        for x in 0..5 {
            for z in 0..5 {
                for y in 0..3 {
                    let pos = get_room_index(x, y, z);
                    if let Some(room) = room_grid[pos as usize] {
                        for direction in Direction::VALUES {
                            let neigh_x = x + direction.step_x();
                            let neigh_y = y + direction.step_y();
                            let neigh_z = z + direction.step_z();
                            if (0..5).contains(&neigh_x) && (0..5).contains(&neigh_z) && (0..3).contains(&neigh_y) {
                                let neigh_pos = get_room_index(neigh_x, neigh_y, neigh_z);
                                if let Some(neighbor) = room_grid[neigh_pos as usize] {
                                    if neigh_z == z {
                                        RoomDefinition::set_connection(rooms, room, direction, neighbor);
                                    } else {
                                        RoomDefinition::set_connection(rooms, room, direction.opposite(), neighbor);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        let roof_room = RoomDefinition::create(rooms, PENTHOUSE_INDEX);
        let left_wing = RoomDefinition::create(rooms, LEFTWING_INDEX);
        let right_wing = RoomDefinition::create(rooms, RIGHTWING_INDEX);
        RoomDefinition::set_connection(rooms, room_grid[GRIDROOM_TOP_CONNECT_INDEX as usize].unwrap(), Direction::Up, roof_room);
        RoomDefinition::set_connection(rooms, room_grid[GRIDROOM_LEFTWING_CONNECT_INDEX as usize].unwrap(), Direction::South, left_wing);
        RoomDefinition::set_connection(rooms, room_grid[GRIDROOM_RIGHTWING_CONNECT_INDEX as usize].unwrap(), Direction::South, right_wing);
        rooms[roof_room as usize].claimed = true;
        rooms[left_wing as usize].claimed = true;
        rooms[right_wing as usize].claimed = true;
        rooms[source_room as usize].is_source = true;
        let core_room = room_grid[get_room_index(random.next_int_bounded(4), 0, 2) as usize].unwrap();
        let connection = |rooms: &[RoomDefinition], room: u8, direction: usize| rooms[room as usize].connections[direction].unwrap();
        let core_east = connection(rooms, core_room, EAST);
        let core_north = connection(rooms, core_room, NORTH);
        let core_east_north = connection(rooms, core_east, NORTH);
        let core_up = connection(rooms, core_room, UP);
        let core_east_up = connection(rooms, core_east, UP);
        let core_north_up = connection(rooms, core_north, UP);
        let core_east_north_up = connection(rooms, core_east_north, UP);
        for claimed in [core_room, core_east, core_north, core_east_north, core_up, core_east_up, core_north_up, core_east_north_up] {
            rooms[claimed as usize].claimed = true;
        }
        let mut room_defs = Vec::with_capacity(rooms.len());
        for definition in room_grid.into_iter().flatten() {
            rooms[definition as usize].update_openings();
            room_defs.push(definition);
        }
        rooms[roof_room as usize].update_openings();
        shuffle(&mut room_defs, random);
        let mut scan_index = 1;
        for &definition in &room_defs {
            let mut close_count = 0;
            let mut attempt_count = 0;
            while close_count < 2 && attempt_count < 5 {
                attempt_count += 1;
                let f = random.next_int_bounded(6) as usize;
                if rooms[definition as usize].has_opening[f] {
                    let of = Direction::from_3d_data_value(f as i32).opposite().ordinal();
                    let neighbor = connection(rooms, definition, f);
                    rooms[definition as usize].has_opening[f] = false;
                    rooms[neighbor as usize].has_opening[of] = false;
                    let mut found = RoomDefinition::find_source(rooms, definition, scan_index);
                    scan_index += 1;
                    if found {
                        found = RoomDefinition::find_source(rooms, neighbor, scan_index);
                        scan_index += 1;
                    }
                    if found {
                        close_count += 1;
                    } else {
                        rooms[definition as usize].has_opening[f] = true;
                        rooms[neighbor as usize].has_opening[of] = true;
                    }
                }
            }
        }
        room_defs.push(roof_room);
        room_defs.push(left_wing);
        room_defs.push(right_wing);
        (room_defs, source_room, core_room)
    }

    pub fn post_process(&self, level: &mut impl WorldGenLevel, random: &mut impl RandomSource, chunk_bb: &BoundingBox) {
        let monument = &self.monument;
        let piece = &monument.piece;
        let base_light = base_light();
        let water_height = level.get_sea_level().max(64) - piece.bounding_box.min.y;
        monument.generate_water_box(level, chunk_bb, 0, 0, 0, 58, water_height, 58);
        self.generate_wing(false, 0, level, chunk_bb);
        self.generate_wing(true, 33, level, chunk_bb);
        self.generate_entrance_archs(level, chunk_bb);
        self.generate_entrance_wall(level, chunk_bb);
        self.generate_roof_piece(level, chunk_bb);
        self.generate_lower_wall(level, chunk_bb);
        self.generate_middle_wall(level, chunk_bb);
        self.generate_upper_wall(level, chunk_bb);
        for pillar_x in 0..7 {
            let mut pillar_z = 0;
            while pillar_z < 7 {
                if pillar_z == 0 && pillar_x == 3 {
                    pillar_z = 6;
                }
                let bx = pillar_x * 9;
                let bz = pillar_z * 9;
                for w in 0..4 {
                    for d in 0..4 {
                        piece.place_block(level, base_light, bx + w, 0, bz + d, chunk_bb);
                        piece.fill_column_down(level, base_light, bx + w, -1, bz + d, chunk_bb);
                    }
                }
                if pillar_x != 0 && pillar_x != 6 {
                    pillar_z += 6;
                } else {
                    pillar_z += 1;
                }
            }
        }
        for i in 0..5 {
            monument.generate_water_box(level, chunk_bb, -1 - i, i * 2, -1 - i, -1 - i, 23, 58 + i);
            monument.generate_water_box(level, chunk_bb, 58 + i, i * 2, -1 - i, 58 + i, 23, 58 + i);
            monument.generate_water_box(level, chunk_bb, -i, i * 2, -1 - i, 57 + i, 23, -1 - i);
            monument.generate_water_box(level, chunk_bb, -i, i * 2, 58 + i, 57 + i, 23, 58 + i);
        }
        for child in &self.child_pieces {
            if child.get_bounding_box().intersects(chunk_bb) {
                child.post_process(level, random, chunk_bb, &self.rooms);
            }
        }
    }

    fn generate_wing(&self, is_flipped: bool, xoff: i32, level: &mut impl WorldGenLevel, chunk_bb: &BoundingBox) {
        let monument = &self.monument;
        let piece = &monument.piece;
        let (base_gray, base_light) = (base_gray(), base_light());
        let dot_deco_data = base_light;
        if monument.chunk_intersects(chunk_bb, xoff, 0, xoff + 23, 20) {
            piece.generate_box(level, chunk_bb, xoff, 0, 0, xoff + 24, 0, 20, base_gray, base_gray, false);
            monument.generate_water_box(level, chunk_bb, xoff, 1, 0, xoff + 24, 10, 20);
            for i in 0..4 {
                piece.generate_box(level, chunk_bb, xoff + i, i + 1, i, xoff + i, i + 1, 20, base_light, base_light, false);
                piece.generate_box(level, chunk_bb, xoff + i + 7, i + 5, i + 7, xoff + i + 7, i + 5, 20, base_light, base_light, false);
                piece.generate_box(level, chunk_bb, xoff + 17 - i, i + 5, i + 7, xoff + 17 - i, i + 5, 20, base_light, base_light, false);
                piece.generate_box(level, chunk_bb, xoff + 24 - i, i + 1, i, xoff + 24 - i, i + 1, 20, base_light, base_light, false);
                piece.generate_box(level, chunk_bb, xoff + i + 1, i + 1, i, xoff + 23 - i, i + 1, i, base_light, base_light, false);
                piece.generate_box(level, chunk_bb, xoff + i + 8, i + 5, i + 7, xoff + 16 - i, i + 5, i + 7, base_light, base_light, false);
            }
            piece.generate_box(level, chunk_bb, xoff + 4, 4, 4, xoff + 6, 4, 20, base_gray, base_gray, false);
            piece.generate_box(level, chunk_bb, xoff + 7, 4, 4, xoff + 17, 4, 6, base_gray, base_gray, false);
            piece.generate_box(level, chunk_bb, xoff + 18, 4, 4, xoff + 20, 4, 20, base_gray, base_gray, false);
            piece.generate_box(level, chunk_bb, xoff + 11, 8, 11, xoff + 13, 8, 20, base_gray, base_gray, false);
            piece.place_block(level, dot_deco_data, xoff + 12, 9, 12, chunk_bb);
            piece.place_block(level, dot_deco_data, xoff + 12, 9, 15, chunk_bb);
            piece.place_block(level, dot_deco_data, xoff + 12, 9, 18, chunk_bb);
            let left_pos = xoff + if is_flipped { 19 } else { 5 };
            let right_pos = xoff + if is_flipped { 5 } else { 19 };
            for z in (5..=20).rev().step_by(3) {
                piece.place_block(level, dot_deco_data, left_pos, 5, z, chunk_bb);
            }
            for z in (7..=19).rev().step_by(3) {
                piece.place_block(level, dot_deco_data, right_pos, 5, z, chunk_bb);
            }
            for i in 0..4 {
                let pos = if is_flipped { xoff + 24 - (17 - i * 3) } else { xoff + 17 - i * 3 };
                piece.place_block(level, dot_deco_data, pos, 5, 5, chunk_bb);
            }
            piece.place_block(level, dot_deco_data, right_pos, 5, 5, chunk_bb);
            piece.generate_box(level, chunk_bb, xoff + 11, 1, 12, xoff + 13, 7, 12, base_gray, base_gray, false);
            piece.generate_box(level, chunk_bb, xoff + 12, 1, 11, xoff + 12, 7, 13, base_gray, base_gray, false);
        }
    }

    fn generate_entrance_archs(&self, level: &mut impl WorldGenLevel, chunk_bb: &BoundingBox) {
        let monument = &self.monument;
        let piece = &monument.piece;
        let (base_gray, base_light, lamp_block) = (base_gray(), base_light(), lamp_block());
        if monument.chunk_intersects(chunk_bb, 22, 5, 35, 17) {
            monument.generate_water_box(level, chunk_bb, 25, 0, 0, 32, 8, 20);
            for i in 0..4 {
                piece.generate_box(level, chunk_bb, 24, 2, 5 + i * 4, 24, 4, 5 + i * 4, base_light, base_light, false);
                piece.generate_box(level, chunk_bb, 22, 4, 5 + i * 4, 23, 4, 5 + i * 4, base_light, base_light, false);
                piece.place_block(level, base_light, 25, 5, 5 + i * 4, chunk_bb);
                piece.place_block(level, base_light, 26, 6, 5 + i * 4, chunk_bb);
                piece.place_block(level, lamp_block, 26, 5, 5 + i * 4, chunk_bb);
                piece.generate_box(level, chunk_bb, 33, 2, 5 + i * 4, 33, 4, 5 + i * 4, base_light, base_light, false);
                piece.generate_box(level, chunk_bb, 34, 4, 5 + i * 4, 35, 4, 5 + i * 4, base_light, base_light, false);
                piece.place_block(level, base_light, 32, 5, 5 + i * 4, chunk_bb);
                piece.place_block(level, base_light, 31, 6, 5 + i * 4, chunk_bb);
                piece.place_block(level, lamp_block, 31, 5, 5 + i * 4, chunk_bb);
                piece.generate_box(level, chunk_bb, 27, 6, 5 + i * 4, 30, 6, 5 + i * 4, base_gray, base_gray, false);
            }
        }
    }

    fn generate_entrance_wall(&self, level: &mut impl WorldGenLevel, chunk_bb: &BoundingBox) {
        let monument = &self.monument;
        let piece = &monument.piece;
        let (base_gray, base_light, base_black) = (base_gray(), base_light(), base_black());
        if monument.chunk_intersects(chunk_bb, 15, 20, 42, 21) {
            piece.generate_box(level, chunk_bb, 15, 0, 21, 42, 0, 21, base_gray, base_gray, false);
            monument.generate_water_box(level, chunk_bb, 26, 1, 21, 31, 3, 21);
            piece.generate_box(level, chunk_bb, 21, 12, 21, 36, 12, 21, base_gray, base_gray, false);
            piece.generate_box(level, chunk_bb, 17, 11, 21, 40, 11, 21, base_gray, base_gray, false);
            piece.generate_box(level, chunk_bb, 16, 10, 21, 41, 10, 21, base_gray, base_gray, false);
            piece.generate_box(level, chunk_bb, 15, 7, 21, 42, 9, 21, base_gray, base_gray, false);
            piece.generate_box(level, chunk_bb, 16, 6, 21, 41, 6, 21, base_gray, base_gray, false);
            piece.generate_box(level, chunk_bb, 17, 5, 21, 40, 5, 21, base_gray, base_gray, false);
            piece.generate_box(level, chunk_bb, 21, 4, 21, 36, 4, 21, base_gray, base_gray, false);
            piece.generate_box(level, chunk_bb, 22, 3, 21, 26, 3, 21, base_gray, base_gray, false);
            piece.generate_box(level, chunk_bb, 31, 3, 21, 35, 3, 21, base_gray, base_gray, false);
            piece.generate_box(level, chunk_bb, 23, 2, 21, 25, 2, 21, base_gray, base_gray, false);
            piece.generate_box(level, chunk_bb, 32, 2, 21, 34, 2, 21, base_gray, base_gray, false);
            piece.generate_box(level, chunk_bb, 28, 4, 20, 29, 4, 21, base_light, base_light, false);
            piece.place_block(level, base_light, 27, 3, 21, chunk_bb);
            piece.place_block(level, base_light, 30, 3, 21, chunk_bb);
            piece.place_block(level, base_light, 26, 2, 21, chunk_bb);
            piece.place_block(level, base_light, 31, 2, 21, chunk_bb);
            piece.place_block(level, base_light, 25, 1, 21, chunk_bb);
            piece.place_block(level, base_light, 32, 1, 21, chunk_bb);
            for i in 0..7 {
                piece.place_block(level, base_black, 28 - i, 6 + i, 21, chunk_bb);
                piece.place_block(level, base_black, 29 + i, 6 + i, 21, chunk_bb);
            }
            for i in 0..4 {
                piece.place_block(level, base_black, 28 - i, 9 + i, 21, chunk_bb);
                piece.place_block(level, base_black, 29 + i, 9 + i, 21, chunk_bb);
            }
            piece.place_block(level, base_black, 28, 12, 21, chunk_bb);
            piece.place_block(level, base_black, 29, 12, 21, chunk_bb);
            for i in 0..3 {
                piece.place_block(level, base_black, 22 - i * 2, 8, 21, chunk_bb);
                piece.place_block(level, base_black, 22 - i * 2, 9, 21, chunk_bb);
                piece.place_block(level, base_black, 35 + i * 2, 8, 21, chunk_bb);
                piece.place_block(level, base_black, 35 + i * 2, 9, 21, chunk_bb);
            }
            monument.generate_water_box(level, chunk_bb, 15, 13, 21, 42, 15, 21);
            monument.generate_water_box(level, chunk_bb, 15, 1, 21, 15, 6, 21);
            monument.generate_water_box(level, chunk_bb, 16, 1, 21, 16, 5, 21);
            monument.generate_water_box(level, chunk_bb, 17, 1, 21, 20, 4, 21);
            monument.generate_water_box(level, chunk_bb, 21, 1, 21, 21, 3, 21);
            monument.generate_water_box(level, chunk_bb, 22, 1, 21, 22, 2, 21);
            monument.generate_water_box(level, chunk_bb, 23, 1, 21, 24, 1, 21);
            monument.generate_water_box(level, chunk_bb, 42, 1, 21, 42, 6, 21);
            monument.generate_water_box(level, chunk_bb, 41, 1, 21, 41, 5, 21);
            monument.generate_water_box(level, chunk_bb, 37, 1, 21, 40, 4, 21);
            monument.generate_water_box(level, chunk_bb, 36, 1, 21, 36, 3, 21);
            monument.generate_water_box(level, chunk_bb, 33, 1, 21, 34, 1, 21);
            monument.generate_water_box(level, chunk_bb, 35, 1, 21, 35, 2, 21);
        }
    }

    fn generate_roof_piece(&self, level: &mut impl WorldGenLevel, chunk_bb: &BoundingBox) {
        let monument = &self.monument;
        let piece = &monument.piece;
        let (base_gray, base_light, lamp_block) = (base_gray(), base_light(), lamp_block());
        if monument.chunk_intersects(chunk_bb, 21, 21, 36, 36) {
            piece.generate_box(level, chunk_bb, 21, 0, 22, 36, 0, 36, base_gray, base_gray, false);
            monument.generate_water_box(level, chunk_bb, 21, 1, 22, 36, 23, 36);
            for i in 0..4 {
                piece.generate_box(level, chunk_bb, 21 + i, 13 + i, 21 + i, 36 - i, 13 + i, 21 + i, base_light, base_light, false);
                piece.generate_box(level, chunk_bb, 21 + i, 13 + i, 36 - i, 36 - i, 13 + i, 36 - i, base_light, base_light, false);
                piece.generate_box(level, chunk_bb, 21 + i, 13 + i, 22 + i, 21 + i, 13 + i, 35 - i, base_light, base_light, false);
                piece.generate_box(level, chunk_bb, 36 - i, 13 + i, 22 + i, 36 - i, 13 + i, 35 - i, base_light, base_light, false);
            }
            piece.generate_box(level, chunk_bb, 25, 16, 25, 32, 16, 32, base_gray, base_gray, false);
            piece.generate_box(level, chunk_bb, 25, 17, 25, 25, 19, 25, base_light, base_light, false);
            piece.generate_box(level, chunk_bb, 32, 17, 25, 32, 19, 25, base_light, base_light, false);
            piece.generate_box(level, chunk_bb, 25, 17, 32, 25, 19, 32, base_light, base_light, false);
            piece.generate_box(level, chunk_bb, 32, 17, 32, 32, 19, 32, base_light, base_light, false);
            piece.place_block(level, base_light, 26, 20, 26, chunk_bb);
            piece.place_block(level, base_light, 27, 21, 27, chunk_bb);
            piece.place_block(level, lamp_block, 27, 20, 27, chunk_bb);
            piece.place_block(level, base_light, 26, 20, 31, chunk_bb);
            piece.place_block(level, base_light, 27, 21, 30, chunk_bb);
            piece.place_block(level, lamp_block, 27, 20, 30, chunk_bb);
            piece.place_block(level, base_light, 31, 20, 31, chunk_bb);
            piece.place_block(level, base_light, 30, 21, 30, chunk_bb);
            piece.place_block(level, lamp_block, 30, 20, 30, chunk_bb);
            piece.place_block(level, base_light, 31, 20, 26, chunk_bb);
            piece.place_block(level, base_light, 30, 21, 27, chunk_bb);
            piece.place_block(level, lamp_block, 30, 20, 27, chunk_bb);
            piece.generate_box(level, chunk_bb, 28, 21, 27, 29, 21, 27, base_gray, base_gray, false);
            piece.generate_box(level, chunk_bb, 27, 21, 28, 27, 21, 29, base_gray, base_gray, false);
            piece.generate_box(level, chunk_bb, 28, 21, 30, 29, 21, 30, base_gray, base_gray, false);
            piece.generate_box(level, chunk_bb, 30, 21, 28, 30, 21, 29, base_gray, base_gray, false);
        }
    }

    fn generate_lower_wall(&self, level: &mut impl WorldGenLevel, chunk_bb: &BoundingBox) {
        let monument = &self.monument;
        let piece = &monument.piece;
        let (base_gray, base_light) = (base_gray(), base_light());
        let dot_deco_data = base_light;
        if monument.chunk_intersects(chunk_bb, 0, 21, 6, 58) {
            piece.generate_box(level, chunk_bb, 0, 0, 21, 6, 0, 57, base_gray, base_gray, false);
            monument.generate_water_box(level, chunk_bb, 0, 1, 21, 6, 7, 57);
            piece.generate_box(level, chunk_bb, 4, 4, 21, 6, 4, 53, base_gray, base_gray, false);
            for i in 0..4 {
                piece.generate_box(level, chunk_bb, i, i + 1, 21, i, i + 1, 57 - i, base_light, base_light, false);
            }
            for z in (23..53).step_by(3) {
                piece.place_block(level, dot_deco_data, 5, 5, z, chunk_bb);
            }
            piece.place_block(level, dot_deco_data, 5, 5, 52, chunk_bb);
            for i in 0..4 {
                piece.generate_box(level, chunk_bb, i, i + 1, 21, i, i + 1, 57 - i, base_light, base_light, false);
            }
            piece.generate_box(level, chunk_bb, 4, 1, 52, 6, 3, 52, base_gray, base_gray, false);
            piece.generate_box(level, chunk_bb, 5, 1, 51, 5, 3, 53, base_gray, base_gray, false);
        }
        if monument.chunk_intersects(chunk_bb, 51, 21, 58, 58) {
            piece.generate_box(level, chunk_bb, 51, 0, 21, 57, 0, 57, base_gray, base_gray, false);
            monument.generate_water_box(level, chunk_bb, 51, 1, 21, 57, 7, 57);
            piece.generate_box(level, chunk_bb, 51, 4, 21, 53, 4, 53, base_gray, base_gray, false);
            for i in 0..4 {
                piece.generate_box(level, chunk_bb, 57 - i, i + 1, 21, 57 - i, i + 1, 57 - i, base_light, base_light, false);
            }
            for z in (23..53).step_by(3) {
                piece.place_block(level, dot_deco_data, 52, 5, z, chunk_bb);
            }
            piece.place_block(level, dot_deco_data, 52, 5, 52, chunk_bb);
            piece.generate_box(level, chunk_bb, 51, 1, 52, 53, 3, 52, base_gray, base_gray, false);
            piece.generate_box(level, chunk_bb, 52, 1, 51, 52, 3, 53, base_gray, base_gray, false);
        }
        if monument.chunk_intersects(chunk_bb, 0, 51, 57, 57) {
            piece.generate_box(level, chunk_bb, 7, 0, 51, 50, 0, 57, base_gray, base_gray, false);
            monument.generate_water_box(level, chunk_bb, 7, 1, 51, 50, 10, 57);
            for i in 0..4 {
                piece.generate_box(level, chunk_bb, i + 1, i + 1, 57 - i, 56 - i, i + 1, 57 - i, base_light, base_light, false);
            }
        }
    }

    fn generate_middle_wall(&self, level: &mut impl WorldGenLevel, chunk_bb: &BoundingBox) {
        let monument = &self.monument;
        let piece = &monument.piece;
        let (base_gray, base_light) = (base_gray(), base_light());
        let dot_deco_data = base_light;
        if monument.chunk_intersects(chunk_bb, 7, 21, 13, 50) {
            piece.generate_box(level, chunk_bb, 7, 0, 21, 13, 0, 50, base_gray, base_gray, false);
            monument.generate_water_box(level, chunk_bb, 7, 1, 21, 13, 10, 50);
            piece.generate_box(level, chunk_bb, 11, 8, 21, 13, 8, 53, base_gray, base_gray, false);
            for i in 0..4 {
                piece.generate_box(level, chunk_bb, i + 7, i + 5, 21, i + 7, i + 5, 54, base_light, base_light, false);
            }
            for z in (21..=45).step_by(3) {
                piece.place_block(level, dot_deco_data, 12, 9, z, chunk_bb);
            }
        }
        if monument.chunk_intersects(chunk_bb, 44, 21, 50, 54) {
            piece.generate_box(level, chunk_bb, 44, 0, 21, 50, 0, 50, base_gray, base_gray, false);
            monument.generate_water_box(level, chunk_bb, 44, 1, 21, 50, 10, 50);
            piece.generate_box(level, chunk_bb, 44, 8, 21, 46, 8, 53, base_gray, base_gray, false);
            for i in 0..4 {
                piece.generate_box(level, chunk_bb, 50 - i, i + 5, 21, 50 - i, i + 5, 54, base_light, base_light, false);
            }
            for z in (21..=45).step_by(3) {
                piece.place_block(level, dot_deco_data, 45, 9, z, chunk_bb);
            }
        }
        if monument.chunk_intersects(chunk_bb, 8, 44, 49, 54) {
            piece.generate_box(level, chunk_bb, 14, 0, 44, 43, 0, 50, base_gray, base_gray, false);
            monument.generate_water_box(level, chunk_bb, 14, 1, 44, 43, 10, 50);
            for x in (12..=45).step_by(3) {
                piece.place_block(level, dot_deco_data, x, 9, 45, chunk_bb);
                piece.place_block(level, dot_deco_data, x, 9, 52, chunk_bb);
                if x == 12 || x == 18 || x == 24 || x == 33 || x == 39 || x == 45 {
                    piece.place_block(level, dot_deco_data, x, 9, 47, chunk_bb);
                    piece.place_block(level, dot_deco_data, x, 9, 50, chunk_bb);
                    piece.place_block(level, dot_deco_data, x, 10, 45, chunk_bb);
                    piece.place_block(level, dot_deco_data, x, 10, 46, chunk_bb);
                    piece.place_block(level, dot_deco_data, x, 10, 51, chunk_bb);
                    piece.place_block(level, dot_deco_data, x, 10, 52, chunk_bb);
                    piece.place_block(level, dot_deco_data, x, 11, 47, chunk_bb);
                    piece.place_block(level, dot_deco_data, x, 11, 50, chunk_bb);
                    piece.place_block(level, dot_deco_data, x, 12, 48, chunk_bb);
                    piece.place_block(level, dot_deco_data, x, 12, 49, chunk_bb);
                }
            }
            for i in 0..3 {
                piece.generate_box(level, chunk_bb, 8 + i, 5 + i, 54, 49 - i, 5 + i, 54, base_gray, base_gray, false);
            }
            piece.generate_box(level, chunk_bb, 11, 8, 54, 46, 8, 54, base_light, base_light, false);
            piece.generate_box(level, chunk_bb, 14, 8, 44, 43, 8, 53, base_gray, base_gray, false);
        }
    }

    fn generate_upper_wall(&self, level: &mut impl WorldGenLevel, chunk_bb: &BoundingBox) {
        let monument = &self.monument;
        let piece = &monument.piece;
        let (base_gray, base_light) = (base_gray(), base_light());
        let dot_deco_data = base_light;
        if monument.chunk_intersects(chunk_bb, 14, 21, 20, 43) {
            piece.generate_box(level, chunk_bb, 14, 0, 21, 20, 0, 43, base_gray, base_gray, false);
            monument.generate_water_box(level, chunk_bb, 14, 1, 22, 20, 14, 43);
            piece.generate_box(level, chunk_bb, 18, 12, 22, 20, 12, 39, base_gray, base_gray, false);
            piece.generate_box(level, chunk_bb, 18, 12, 21, 20, 12, 21, base_light, base_light, false);
            for i in 0..4 {
                piece.generate_box(level, chunk_bb, i + 14, i + 9, 21, i + 14, i + 9, 43 - i, base_light, base_light, false);
            }
            for z in (23..=39).step_by(3) {
                piece.place_block(level, dot_deco_data, 19, 13, z, chunk_bb);
            }
        }
        if monument.chunk_intersects(chunk_bb, 37, 21, 43, 43) {
            piece.generate_box(level, chunk_bb, 37, 0, 21, 43, 0, 43, base_gray, base_gray, false);
            monument.generate_water_box(level, chunk_bb, 37, 1, 22, 43, 14, 43);
            piece.generate_box(level, chunk_bb, 37, 12, 22, 39, 12, 39, base_gray, base_gray, false);
            piece.generate_box(level, chunk_bb, 37, 12, 21, 39, 12, 21, base_light, base_light, false);
            for i in 0..4 {
                piece.generate_box(level, chunk_bb, 43 - i, i + 9, 21, 43 - i, i + 9, 43 - i, base_light, base_light, false);
            }
            for z in (23..=39).step_by(3) {
                piece.place_block(level, dot_deco_data, 38, 13, z, chunk_bb);
            }
        }
        if monument.chunk_intersects(chunk_bb, 15, 37, 42, 43) {
            piece.generate_box(level, chunk_bb, 21, 0, 37, 36, 0, 43, base_gray, base_gray, false);
            monument.generate_water_box(level, chunk_bb, 21, 1, 37, 36, 14, 43);
            piece.generate_box(level, chunk_bb, 21, 12, 37, 36, 12, 39, base_gray, base_gray, false);
            for i in 0..4 {
                piece.generate_box(level, chunk_bb, 15 + i, i + 9, 43 - i, 42 - i, i + 9, 43 - i, base_light, base_light, false);
            }
            for x in (21..=36).step_by(3) {
                piece.place_block(level, dot_deco_data, x, 13, 38, chunk_bb);
            }
        }
    }
}

#[derive(Clone)]
struct OceanMonumentCoreRoom {
    monument: OceanMonumentPiece,
}

impl OceanMonumentCoreRoom {
    fn new(orientation: Direction, rooms: &[RoomDefinition], definition: u8) -> OceanMonumentCoreRoom {
        OceanMonumentCoreRoom { monument: OceanMonumentPiece::new_room(OCEAN_MONUMENT_CORE_ROOM, 1, orientation, rooms, definition, 2, 2, 2) }
    }

    fn post_process(&self, level: &mut impl WorldGenLevel, chunk_bb: &BoundingBox) {
        let monument = &self.monument;
        let piece = &monument.piece;
        let (base_gray, base_light, base_black, lamp_block) = (base_gray(), base_light(), base_black(), lamp_block());
        monument.generate_box_on_fill_only(level, chunk_bb, 1, 8, 0, 14, 8, 14, base_gray);
        let mut block = base_light;
        piece.generate_box(level, chunk_bb, 0, 7, 0, 0, 7, 15, block, block, false);
        piece.generate_box(level, chunk_bb, 15, 7, 0, 15, 7, 15, block, block, false);
        piece.generate_box(level, chunk_bb, 1, 7, 0, 15, 7, 0, block, block, false);
        piece.generate_box(level, chunk_bb, 1, 7, 15, 14, 7, 15, block, block, false);
        for yx in 1..=6 {
            block = base_light;
            if yx == 2 || yx == 6 {
                block = base_gray;
            }
            for x in (0..=15).step_by(15) {
                piece.generate_box(level, chunk_bb, x, yx, 0, x, yx, 1, block, block, false);
                piece.generate_box(level, chunk_bb, x, yx, 6, x, yx, 9, block, block, false);
                piece.generate_box(level, chunk_bb, x, yx, 14, x, yx, 15, block, block, false);
            }
            piece.generate_box(level, chunk_bb, 1, yx, 0, 1, yx, 0, block, block, false);
            piece.generate_box(level, chunk_bb, 6, yx, 0, 9, yx, 0, block, block, false);
            piece.generate_box(level, chunk_bb, 14, yx, 0, 14, yx, 0, block, block, false);
            piece.generate_box(level, chunk_bb, 1, yx, 15, 14, yx, 15, block, block, false);
        }
        piece.generate_box(level, chunk_bb, 6, 3, 6, 9, 6, 9, base_black, base_black, false);
        piece.generate_box(level, chunk_bb, 7, 4, 7, 8, 5, 8, blocks::GOLD_BLOCK.default_state(), blocks::GOLD_BLOCK.default_state(), false);
        for yx in (3..=6).step_by(3) {
            for x in (6..=9).step_by(3) {
                piece.place_block(level, lamp_block, x, yx, 6, chunk_bb);
                piece.place_block(level, lamp_block, x, yx, 9, chunk_bb);
            }
        }
        piece.generate_box(level, chunk_bb, 5, 1, 6, 5, 2, 6, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 5, 1, 9, 5, 2, 9, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 10, 1, 6, 10, 2, 6, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 10, 1, 9, 10, 2, 9, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 6, 1, 5, 6, 2, 5, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 9, 1, 5, 9, 2, 5, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 6, 1, 10, 6, 2, 10, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 9, 1, 10, 9, 2, 10, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 5, 2, 5, 5, 6, 5, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 5, 2, 10, 5, 6, 10, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 10, 2, 5, 10, 6, 5, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 10, 2, 10, 10, 6, 10, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 5, 7, 1, 5, 7, 6, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 10, 7, 1, 10, 7, 6, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 5, 7, 9, 5, 7, 14, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 10, 7, 9, 10, 7, 14, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 1, 7, 5, 6, 7, 5, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 1, 7, 10, 6, 7, 10, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 9, 7, 5, 14, 7, 5, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 9, 7, 10, 14, 7, 10, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 2, 1, 2, 2, 1, 3, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 3, 1, 2, 3, 1, 2, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 13, 1, 2, 13, 1, 3, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 12, 1, 2, 12, 1, 2, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 2, 1, 12, 2, 1, 13, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 3, 1, 13, 3, 1, 13, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 13, 1, 12, 13, 1, 13, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 12, 1, 13, 12, 1, 13, base_light, base_light, false);
    }
}

#[derive(Clone)]
struct OceanMonumentDoubleXRoom {
    monument: OceanMonumentPiece,
}

impl OceanMonumentDoubleXRoom {
    fn new(orientation: Direction, rooms: &[RoomDefinition], definition: u8) -> OceanMonumentDoubleXRoom {
        OceanMonumentDoubleXRoom { monument: OceanMonumentPiece::new_room(OCEAN_MONUMENT_DOUBLE_X_ROOM, 1, orientation, rooms, definition, 2, 1, 1) }
    }

    fn post_process(&self, level: &mut impl WorldGenLevel, chunk_bb: &BoundingBox, rooms: &[RoomDefinition]) {
        let monument = &self.monument;
        let piece = &monument.piece;
        let (base_gray, base_light, lamp_block) = (base_gray(), base_light(), lamp_block());
        let definition = monument.room_definition(rooms);
        let east = definition.connection(rooms, EAST);
        let west = definition;
        if definition.index / 25 > 0 {
            monument.generate_default_floor(level, chunk_bb, 8, 0, east.has_opening[DOWN]);
            monument.generate_default_floor(level, chunk_bb, 0, 0, west.has_opening[DOWN]);
        }
        if west.connections[UP].is_none() {
            monument.generate_box_on_fill_only(level, chunk_bb, 1, 4, 1, 7, 4, 6, base_gray);
        }
        if east.connections[UP].is_none() {
            monument.generate_box_on_fill_only(level, chunk_bb, 8, 4, 1, 14, 4, 6, base_gray);
        }
        piece.generate_box(level, chunk_bb, 0, 3, 0, 0, 3, 7, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 15, 3, 0, 15, 3, 7, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 1, 3, 0, 15, 3, 0, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 1, 3, 7, 14, 3, 7, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 0, 2, 0, 0, 2, 7, base_gray, base_gray, false);
        piece.generate_box(level, chunk_bb, 15, 2, 0, 15, 2, 7, base_gray, base_gray, false);
        piece.generate_box(level, chunk_bb, 1, 2, 0, 15, 2, 0, base_gray, base_gray, false);
        piece.generate_box(level, chunk_bb, 1, 2, 7, 14, 2, 7, base_gray, base_gray, false);
        piece.generate_box(level, chunk_bb, 0, 1, 0, 0, 1, 7, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 15, 1, 0, 15, 1, 7, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 1, 1, 0, 15, 1, 0, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 1, 1, 7, 14, 1, 7, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 5, 1, 0, 10, 1, 4, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 6, 2, 0, 9, 2, 3, base_gray, base_gray, false);
        piece.generate_box(level, chunk_bb, 5, 3, 0, 10, 3, 4, base_light, base_light, false);
        piece.place_block(level, lamp_block, 6, 2, 3, chunk_bb);
        piece.place_block(level, lamp_block, 9, 2, 3, chunk_bb);
        if west.has_opening[SOUTH] {
            monument.generate_water_box(level, chunk_bb, 3, 1, 0, 4, 2, 0);
        }
        if west.has_opening[NORTH] {
            monument.generate_water_box(level, chunk_bb, 3, 1, 7, 4, 2, 7);
        }
        if west.has_opening[WEST] {
            monument.generate_water_box(level, chunk_bb, 0, 1, 3, 0, 2, 4);
        }
        if east.has_opening[SOUTH] {
            monument.generate_water_box(level, chunk_bb, 11, 1, 0, 12, 2, 0);
        }
        if east.has_opening[NORTH] {
            monument.generate_water_box(level, chunk_bb, 11, 1, 7, 12, 2, 7);
        }
        if east.has_opening[EAST] {
            monument.generate_water_box(level, chunk_bb, 15, 1, 3, 15, 2, 4);
        }
    }
}

#[derive(Clone)]
struct OceanMonumentDoubleXYRoom {
    monument: OceanMonumentPiece,
}

impl OceanMonumentDoubleXYRoom {
    fn new(orientation: Direction, rooms: &[RoomDefinition], definition: u8) -> OceanMonumentDoubleXYRoom {
        OceanMonumentDoubleXYRoom { monument: OceanMonumentPiece::new_room(OCEAN_MONUMENT_DOUBLE_XY_ROOM, 1, orientation, rooms, definition, 2, 2, 1) }
    }

    fn post_process(&self, level: &mut impl WorldGenLevel, chunk_bb: &BoundingBox, rooms: &[RoomDefinition]) {
        let monument = &self.monument;
        let piece = &monument.piece;
        let (base_gray, base_light, lamp_block) = (base_gray(), base_light(), lamp_block());
        let definition = monument.room_definition(rooms);
        let east = definition.connection(rooms, EAST);
        let west = definition;
        let west_up = west.connection(rooms, UP);
        let east_up = east.connection(rooms, UP);
        if definition.index / 25 > 0 {
            monument.generate_default_floor(level, chunk_bb, 8, 0, east.has_opening[DOWN]);
            monument.generate_default_floor(level, chunk_bb, 0, 0, west.has_opening[DOWN]);
        }
        if west_up.connections[UP].is_none() {
            monument.generate_box_on_fill_only(level, chunk_bb, 1, 8, 1, 7, 8, 6, base_gray);
        }
        if east_up.connections[UP].is_none() {
            monument.generate_box_on_fill_only(level, chunk_bb, 8, 8, 1, 14, 8, 6, base_gray);
        }
        for y in 1..=7 {
            let mut block = base_light;
            if y == 2 || y == 6 {
                block = base_gray;
            }
            piece.generate_box(level, chunk_bb, 0, y, 0, 0, y, 7, block, block, false);
            piece.generate_box(level, chunk_bb, 15, y, 0, 15, y, 7, block, block, false);
            piece.generate_box(level, chunk_bb, 1, y, 0, 15, y, 0, block, block, false);
            piece.generate_box(level, chunk_bb, 1, y, 7, 14, y, 7, block, block, false);
        }
        piece.generate_box(level, chunk_bb, 2, 1, 3, 2, 7, 4, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 3, 1, 2, 4, 7, 2, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 3, 1, 5, 4, 7, 5, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 13, 1, 3, 13, 7, 4, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 11, 1, 2, 12, 7, 2, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 11, 1, 5, 12, 7, 5, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 5, 1, 3, 5, 3, 4, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 10, 1, 3, 10, 3, 4, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 5, 7, 2, 10, 7, 5, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 5, 5, 2, 5, 7, 2, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 10, 5, 2, 10, 7, 2, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 5, 5, 5, 5, 7, 5, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 10, 5, 5, 10, 7, 5, base_light, base_light, false);
        piece.place_block(level, base_light, 6, 6, 2, chunk_bb);
        piece.place_block(level, base_light, 9, 6, 2, chunk_bb);
        piece.place_block(level, base_light, 6, 6, 5, chunk_bb);
        piece.place_block(level, base_light, 9, 6, 5, chunk_bb);
        piece.generate_box(level, chunk_bb, 5, 4, 3, 6, 4, 4, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 9, 4, 3, 10, 4, 4, base_light, base_light, false);
        piece.place_block(level, lamp_block, 5, 4, 2, chunk_bb);
        piece.place_block(level, lamp_block, 5, 4, 5, chunk_bb);
        piece.place_block(level, lamp_block, 10, 4, 2, chunk_bb);
        piece.place_block(level, lamp_block, 10, 4, 5, chunk_bb);
        if west.has_opening[SOUTH] {
            monument.generate_water_box(level, chunk_bb, 3, 1, 0, 4, 2, 0);
        }
        if west.has_opening[NORTH] {
            monument.generate_water_box(level, chunk_bb, 3, 1, 7, 4, 2, 7);
        }
        if west.has_opening[WEST] {
            monument.generate_water_box(level, chunk_bb, 0, 1, 3, 0, 2, 4);
        }
        if east.has_opening[SOUTH] {
            monument.generate_water_box(level, chunk_bb, 11, 1, 0, 12, 2, 0);
        }
        if east.has_opening[NORTH] {
            monument.generate_water_box(level, chunk_bb, 11, 1, 7, 12, 2, 7);
        }
        if east.has_opening[EAST] {
            monument.generate_water_box(level, chunk_bb, 15, 1, 3, 15, 2, 4);
        }
        if west_up.has_opening[SOUTH] {
            monument.generate_water_box(level, chunk_bb, 3, 5, 0, 4, 6, 0);
        }
        if west_up.has_opening[NORTH] {
            monument.generate_water_box(level, chunk_bb, 3, 5, 7, 4, 6, 7);
        }
        if west_up.has_opening[WEST] {
            monument.generate_water_box(level, chunk_bb, 0, 5, 3, 0, 6, 4);
        }
        if east_up.has_opening[SOUTH] {
            monument.generate_water_box(level, chunk_bb, 11, 5, 0, 12, 6, 0);
        }
        if east_up.has_opening[NORTH] {
            monument.generate_water_box(level, chunk_bb, 11, 5, 7, 12, 6, 7);
        }
        if east_up.has_opening[EAST] {
            monument.generate_water_box(level, chunk_bb, 15, 5, 3, 15, 6, 4);
        }
    }
}

#[derive(Clone)]
struct OceanMonumentDoubleYRoom {
    monument: OceanMonumentPiece,
}

impl OceanMonumentDoubleYRoom {
    fn new(orientation: Direction, rooms: &[RoomDefinition], definition: u8) -> OceanMonumentDoubleYRoom {
        OceanMonumentDoubleYRoom { monument: OceanMonumentPiece::new_room(OCEAN_MONUMENT_DOUBLE_Y_ROOM, 1, orientation, rooms, definition, 1, 2, 1) }
    }

    fn post_process(&self, level: &mut impl WorldGenLevel, chunk_bb: &BoundingBox, rooms: &[RoomDefinition]) {
        let monument = &self.monument;
        let piece = &monument.piece;
        let (base_gray, base_light) = (base_gray(), base_light());
        let room_definition = monument.room_definition(rooms);
        if room_definition.index / 25 > 0 {
            monument.generate_default_floor(level, chunk_bb, 0, 0, room_definition.has_opening[DOWN]);
        }
        let above = room_definition.connection(rooms, UP);
        if above.connections[UP].is_none() {
            monument.generate_box_on_fill_only(level, chunk_bb, 1, 8, 1, 6, 8, 6, base_gray);
        }
        piece.generate_box(level, chunk_bb, 0, 4, 0, 0, 4, 7, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 7, 4, 0, 7, 4, 7, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 1, 4, 0, 6, 4, 0, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 1, 4, 7, 6, 4, 7, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 2, 4, 1, 2, 4, 2, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 1, 4, 2, 1, 4, 2, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 5, 4, 1, 5, 4, 2, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 6, 4, 2, 6, 4, 2, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 2, 4, 5, 2, 4, 6, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 1, 4, 5, 1, 4, 5, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 5, 4, 5, 5, 4, 6, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 6, 4, 5, 6, 4, 5, base_light, base_light, false);
        let mut definition = room_definition;
        for y in (1..=5).step_by(4) {
            let z = 0;
            if definition.has_opening[SOUTH] {
                piece.generate_box(level, chunk_bb, 2, y, z, 2, y + 2, z, base_light, base_light, false);
                piece.generate_box(level, chunk_bb, 5, y, z, 5, y + 2, z, base_light, base_light, false);
                piece.generate_box(level, chunk_bb, 3, y + 2, z, 4, y + 2, z, base_light, base_light, false);
            } else {
                piece.generate_box(level, chunk_bb, 0, y, z, 7, y + 2, z, base_light, base_light, false);
                piece.generate_box(level, chunk_bb, 0, y + 1, z, 7, y + 1, z, base_gray, base_gray, false);
            }
            let z = 7;
            if definition.has_opening[NORTH] {
                piece.generate_box(level, chunk_bb, 2, y, z, 2, y + 2, z, base_light, base_light, false);
                piece.generate_box(level, chunk_bb, 5, y, z, 5, y + 2, z, base_light, base_light, false);
                piece.generate_box(level, chunk_bb, 3, y + 2, z, 4, y + 2, z, base_light, base_light, false);
            } else {
                piece.generate_box(level, chunk_bb, 0, y, z, 7, y + 2, z, base_light, base_light, false);
                piece.generate_box(level, chunk_bb, 0, y + 1, z, 7, y + 1, z, base_gray, base_gray, false);
            }
            let x = 0;
            if definition.has_opening[WEST] {
                piece.generate_box(level, chunk_bb, x, y, 2, x, y + 2, 2, base_light, base_light, false);
                piece.generate_box(level, chunk_bb, x, y, 5, x, y + 2, 5, base_light, base_light, false);
                piece.generate_box(level, chunk_bb, x, y + 2, 3, x, y + 2, 4, base_light, base_light, false);
            } else {
                piece.generate_box(level, chunk_bb, x, y, 0, x, y + 2, 7, base_light, base_light, false);
                piece.generate_box(level, chunk_bb, x, y + 1, 0, x, y + 1, 7, base_gray, base_gray, false);
            }
            let x = 7;
            if definition.has_opening[EAST] {
                piece.generate_box(level, chunk_bb, x, y, 2, x, y + 2, 2, base_light, base_light, false);
                piece.generate_box(level, chunk_bb, x, y, 5, x, y + 2, 5, base_light, base_light, false);
                piece.generate_box(level, chunk_bb, x, y + 2, 3, x, y + 2, 4, base_light, base_light, false);
            } else {
                piece.generate_box(level, chunk_bb, x, y, 0, x, y + 2, 7, base_light, base_light, false);
                piece.generate_box(level, chunk_bb, x, y + 1, 0, x, y + 1, 7, base_gray, base_gray, false);
            }
            definition = above;
        }
    }
}

#[derive(Clone)]
struct OceanMonumentDoubleYZRoom {
    monument: OceanMonumentPiece,
}

impl OceanMonumentDoubleYZRoom {
    fn new(orientation: Direction, rooms: &[RoomDefinition], definition: u8) -> OceanMonumentDoubleYZRoom {
        OceanMonumentDoubleYZRoom { monument: OceanMonumentPiece::new_room(OCEAN_MONUMENT_DOUBLE_YZ_ROOM, 1, orientation, rooms, definition, 1, 2, 2) }
    }

    fn post_process(&self, level: &mut impl WorldGenLevel, chunk_bb: &BoundingBox, rooms: &[RoomDefinition]) {
        let monument = &self.monument;
        let piece = &monument.piece;
        let (base_gray, base_light, base_black, lamp_block) = (base_gray(), base_light(), base_black(), lamp_block());
        let definition = monument.room_definition(rooms);
        let north = definition.connection(rooms, NORTH);
        let south = definition;
        let north_up = north.connection(rooms, UP);
        let south_up = south.connection(rooms, UP);
        if definition.index / 25 > 0 {
            monument.generate_default_floor(level, chunk_bb, 0, 8, north.has_opening[DOWN]);
            monument.generate_default_floor(level, chunk_bb, 0, 0, south.has_opening[DOWN]);
        }
        if south_up.connections[UP].is_none() {
            monument.generate_box_on_fill_only(level, chunk_bb, 1, 8, 1, 6, 8, 7, base_gray);
        }
        if north_up.connections[UP].is_none() {
            monument.generate_box_on_fill_only(level, chunk_bb, 1, 8, 8, 6, 8, 14, base_gray);
        }
        for y in 1..=7 {
            let mut block = base_light;
            if y == 2 || y == 6 {
                block = base_gray;
            }
            piece.generate_box(level, chunk_bb, 0, y, 0, 0, y, 15, block, block, false);
            piece.generate_box(level, chunk_bb, 7, y, 0, 7, y, 15, block, block, false);
            piece.generate_box(level, chunk_bb, 1, y, 0, 6, y, 0, block, block, false);
            piece.generate_box(level, chunk_bb, 1, y, 15, 6, y, 15, block, block, false);
        }
        for y in 1..=7 {
            let mut block = base_black;
            if y == 2 || y == 6 {
                block = lamp_block;
            }
            piece.generate_box(level, chunk_bb, 3, y, 7, 4, y, 8, block, block, false);
        }
        if south.has_opening[SOUTH] {
            monument.generate_water_box(level, chunk_bb, 3, 1, 0, 4, 2, 0);
        }
        if south.has_opening[EAST] {
            monument.generate_water_box(level, chunk_bb, 7, 1, 3, 7, 2, 4);
        }
        if south.has_opening[WEST] {
            monument.generate_water_box(level, chunk_bb, 0, 1, 3, 0, 2, 4);
        }
        if north.has_opening[NORTH] {
            monument.generate_water_box(level, chunk_bb, 3, 1, 15, 4, 2, 15);
        }
        if north.has_opening[WEST] {
            monument.generate_water_box(level, chunk_bb, 0, 1, 11, 0, 2, 12);
        }
        if north.has_opening[EAST] {
            monument.generate_water_box(level, chunk_bb, 7, 1, 11, 7, 2, 12);
        }
        if south_up.has_opening[SOUTH] {
            monument.generate_water_box(level, chunk_bb, 3, 5, 0, 4, 6, 0);
        }
        if south_up.has_opening[EAST] {
            monument.generate_water_box(level, chunk_bb, 7, 5, 3, 7, 6, 4);
            piece.generate_box(level, chunk_bb, 5, 4, 2, 6, 4, 5, base_light, base_light, false);
            piece.generate_box(level, chunk_bb, 6, 1, 2, 6, 3, 2, base_light, base_light, false);
            piece.generate_box(level, chunk_bb, 6, 1, 5, 6, 3, 5, base_light, base_light, false);
        }
        if south_up.has_opening[WEST] {
            monument.generate_water_box(level, chunk_bb, 0, 5, 3, 0, 6, 4);
            piece.generate_box(level, chunk_bb, 1, 4, 2, 2, 4, 5, base_light, base_light, false);
            piece.generate_box(level, chunk_bb, 1, 1, 2, 1, 3, 2, base_light, base_light, false);
            piece.generate_box(level, chunk_bb, 1, 1, 5, 1, 3, 5, base_light, base_light, false);
        }
        if north_up.has_opening[NORTH] {
            monument.generate_water_box(level, chunk_bb, 3, 5, 15, 4, 6, 15);
        }
        if north_up.has_opening[WEST] {
            monument.generate_water_box(level, chunk_bb, 0, 5, 11, 0, 6, 12);
            piece.generate_box(level, chunk_bb, 1, 4, 10, 2, 4, 13, base_light, base_light, false);
            piece.generate_box(level, chunk_bb, 1, 1, 10, 1, 3, 10, base_light, base_light, false);
            piece.generate_box(level, chunk_bb, 1, 1, 13, 1, 3, 13, base_light, base_light, false);
        }
        if north_up.has_opening[EAST] {
            monument.generate_water_box(level, chunk_bb, 7, 5, 11, 7, 6, 12);
            piece.generate_box(level, chunk_bb, 5, 4, 10, 6, 4, 13, base_light, base_light, false);
            piece.generate_box(level, chunk_bb, 6, 1, 10, 6, 3, 10, base_light, base_light, false);
            piece.generate_box(level, chunk_bb, 6, 1, 13, 6, 3, 13, base_light, base_light, false);
        }
    }
}

#[derive(Clone)]
struct OceanMonumentDoubleZRoom {
    monument: OceanMonumentPiece,
}

impl OceanMonumentDoubleZRoom {
    fn new(orientation: Direction, rooms: &[RoomDefinition], definition: u8) -> OceanMonumentDoubleZRoom {
        OceanMonumentDoubleZRoom { monument: OceanMonumentPiece::new_room(OCEAN_MONUMENT_DOUBLE_Z_ROOM, 1, orientation, rooms, definition, 1, 1, 2) }
    }

    fn post_process(&self, level: &mut impl WorldGenLevel, chunk_bb: &BoundingBox, rooms: &[RoomDefinition]) {
        let monument = &self.monument;
        let piece = &monument.piece;
        let (base_gray, base_light, lamp_block) = (base_gray(), base_light(), lamp_block());
        let definition = monument.room_definition(rooms);
        let north = definition.connection(rooms, NORTH);
        let south = definition;
        if definition.index / 25 > 0 {
            monument.generate_default_floor(level, chunk_bb, 0, 8, north.has_opening[DOWN]);
            monument.generate_default_floor(level, chunk_bb, 0, 0, south.has_opening[DOWN]);
        }
        if south.connections[UP].is_none() {
            monument.generate_box_on_fill_only(level, chunk_bb, 1, 4, 1, 6, 4, 7, base_gray);
        }
        if north.connections[UP].is_none() {
            monument.generate_box_on_fill_only(level, chunk_bb, 1, 4, 8, 6, 4, 14, base_gray);
        }
        piece.generate_box(level, chunk_bb, 0, 3, 0, 0, 3, 15, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 7, 3, 0, 7, 3, 15, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 1, 3, 0, 7, 3, 0, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 1, 3, 15, 6, 3, 15, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 0, 2, 0, 0, 2, 15, base_gray, base_gray, false);
        piece.generate_box(level, chunk_bb, 7, 2, 0, 7, 2, 15, base_gray, base_gray, false);
        piece.generate_box(level, chunk_bb, 1, 2, 0, 7, 2, 0, base_gray, base_gray, false);
        piece.generate_box(level, chunk_bb, 1, 2, 15, 6, 2, 15, base_gray, base_gray, false);
        piece.generate_box(level, chunk_bb, 0, 1, 0, 0, 1, 15, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 7, 1, 0, 7, 1, 15, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 1, 1, 0, 7, 1, 0, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 1, 1, 15, 6, 1, 15, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 1, 1, 1, 1, 1, 2, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 6, 1, 1, 6, 1, 2, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 1, 3, 1, 1, 3, 2, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 6, 3, 1, 6, 3, 2, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 1, 1, 13, 1, 1, 14, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 6, 1, 13, 6, 1, 14, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 1, 3, 13, 1, 3, 14, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 6, 3, 13, 6, 3, 14, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 2, 1, 6, 2, 3, 6, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 5, 1, 6, 5, 3, 6, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 2, 1, 9, 2, 3, 9, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 5, 1, 9, 5, 3, 9, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 3, 2, 6, 4, 2, 6, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 3, 2, 9, 4, 2, 9, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 2, 2, 7, 2, 2, 8, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 5, 2, 7, 5, 2, 8, base_light, base_light, false);
        piece.place_block(level, lamp_block, 2, 2, 5, chunk_bb);
        piece.place_block(level, lamp_block, 5, 2, 5, chunk_bb);
        piece.place_block(level, lamp_block, 2, 2, 10, chunk_bb);
        piece.place_block(level, lamp_block, 5, 2, 10, chunk_bb);
        piece.place_block(level, base_light, 2, 3, 5, chunk_bb);
        piece.place_block(level, base_light, 5, 3, 5, chunk_bb);
        piece.place_block(level, base_light, 2, 3, 10, chunk_bb);
        piece.place_block(level, base_light, 5, 3, 10, chunk_bb);
        if south.has_opening[SOUTH] {
            monument.generate_water_box(level, chunk_bb, 3, 1, 0, 4, 2, 0);
        }
        if south.has_opening[EAST] {
            monument.generate_water_box(level, chunk_bb, 7, 1, 3, 7, 2, 4);
        }
        if south.has_opening[WEST] {
            monument.generate_water_box(level, chunk_bb, 0, 1, 3, 0, 2, 4);
        }
        if north.has_opening[NORTH] {
            monument.generate_water_box(level, chunk_bb, 3, 1, 15, 4, 2, 15);
        }
        if north.has_opening[WEST] {
            monument.generate_water_box(level, chunk_bb, 0, 1, 11, 0, 2, 12);
        }
        if north.has_opening[EAST] {
            monument.generate_water_box(level, chunk_bb, 7, 1, 11, 7, 2, 12);
        }
    }
}

#[derive(Clone)]
struct OceanMonumentEntryRoom {
    monument: OceanMonumentPiece,
}

impl OceanMonumentEntryRoom {
    fn new(orientation: Direction, rooms: &[RoomDefinition], definition: u8) -> OceanMonumentEntryRoom {
        OceanMonumentEntryRoom { monument: OceanMonumentPiece::new_room(OCEAN_MONUMENT_ENTRY_ROOM, 1, orientation, rooms, definition, 1, 1, 1) }
    }

    fn post_process(&self, level: &mut impl WorldGenLevel, chunk_bb: &BoundingBox, rooms: &[RoomDefinition]) {
        let monument = &self.monument;
        let piece = &monument.piece;
        let base_light = base_light();
        let definition = monument.room_definition(rooms);
        piece.generate_box(level, chunk_bb, 0, 3, 0, 2, 3, 7, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 5, 3, 0, 7, 3, 7, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 0, 2, 0, 1, 2, 7, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 6, 2, 0, 7, 2, 7, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 0, 1, 0, 0, 1, 7, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 7, 1, 0, 7, 1, 7, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 0, 1, 7, 7, 3, 7, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 1, 1, 0, 2, 3, 0, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 5, 1, 0, 6, 3, 0, base_light, base_light, false);
        if definition.has_opening[NORTH] {
            monument.generate_water_box(level, chunk_bb, 3, 1, 7, 4, 2, 7);
        }
        if definition.has_opening[WEST] {
            monument.generate_water_box(level, chunk_bb, 0, 1, 3, 1, 2, 4);
        }
        if definition.has_opening[EAST] {
            monument.generate_water_box(level, chunk_bb, 6, 1, 3, 7, 2, 4);
        }
    }
}

#[derive(Clone)]
struct OceanMonumentPenthouse {
    monument: OceanMonumentPiece,
}

impl OceanMonumentPenthouse {
    fn new(orientation: Direction, bounding_box: BoundingBox) -> OceanMonumentPenthouse {
        OceanMonumentPenthouse { monument: OceanMonumentPiece::new(OCEAN_MONUMENT_PENTHOUSE, orientation, 1, bounding_box) }
    }

    fn post_process(&self, level: &mut impl WorldGenLevel, chunk_bb: &BoundingBox) {
        let monument = &self.monument;
        let piece = &monument.piece;
        let (base_gray, base_light, base_black, lamp_block) = (base_gray(), base_light(), base_black(), lamp_block());
        piece.generate_box(level, chunk_bb, 2, -1, 2, 11, -1, 11, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 0, -1, 0, 1, -1, 11, base_gray, base_gray, false);
        piece.generate_box(level, chunk_bb, 12, -1, 0, 13, -1, 11, base_gray, base_gray, false);
        piece.generate_box(level, chunk_bb, 2, -1, 0, 11, -1, 1, base_gray, base_gray, false);
        piece.generate_box(level, chunk_bb, 2, -1, 12, 11, -1, 13, base_gray, base_gray, false);
        piece.generate_box(level, chunk_bb, 0, 0, 0, 0, 0, 13, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 13, 0, 0, 13, 0, 13, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 1, 0, 0, 12, 0, 0, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 1, 0, 13, 12, 0, 13, base_light, base_light, false);
        for i in (2..=11).step_by(3) {
            piece.place_block(level, lamp_block, 0, 0, i, chunk_bb);
            piece.place_block(level, lamp_block, 13, 0, i, chunk_bb);
            piece.place_block(level, lamp_block, i, 0, 0, chunk_bb);
        }
        piece.generate_box(level, chunk_bb, 2, 0, 3, 4, 0, 9, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 9, 0, 3, 11, 0, 9, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 4, 0, 9, 9, 0, 11, base_light, base_light, false);
        piece.place_block(level, base_light, 5, 0, 8, chunk_bb);
        piece.place_block(level, base_light, 8, 0, 8, chunk_bb);
        piece.place_block(level, base_light, 10, 0, 10, chunk_bb);
        piece.place_block(level, base_light, 3, 0, 10, chunk_bb);
        piece.generate_box(level, chunk_bb, 3, 0, 3, 3, 0, 7, base_black, base_black, false);
        piece.generate_box(level, chunk_bb, 10, 0, 3, 10, 0, 7, base_black, base_black, false);
        piece.generate_box(level, chunk_bb, 6, 0, 10, 7, 0, 10, base_black, base_black, false);
        let mut x = 3;
        for _ in 0..2 {
            for z in (2..=8).step_by(3) {
                piece.generate_box(level, chunk_bb, x, 0, z, x, 2, z, base_light, base_light, false);
            }
            x = 10;
        }
        piece.generate_box(level, chunk_bb, 5, 0, 10, 5, 2, 10, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 8, 0, 10, 8, 2, 10, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 6, -1, 7, 7, -1, 8, base_black, base_black, false);
        monument.generate_water_box(level, chunk_bb, 6, -1, 3, 7, -1, 4);
        monument.spawn_elder(chunk_bb, 6, 1, 6);
    }
}

#[derive(Clone)]
pub struct OceanMonumentPiece {
    pub piece: StructurePiece,
    room_definition: Option<u8>,
}

impl OceanMonumentPiece {
    fn new(piece_type: &'static str, orientation: Direction, gen_depth: i32, bounding_box: BoundingBox) -> OceanMonumentPiece {
        let mut piece = StructurePiece::new(piece_type, gen_depth, bounding_box);
        piece.set_orientation(Some(orientation));
        OceanMonumentPiece { piece, room_definition: None }
    }

    #[allow(clippy::too_many_arguments)]
    fn new_room(
        piece_type: &'static str,
        gen_depth: i32,
        orientation: Direction,
        rooms: &[RoomDefinition],
        room_definition: u8,
        room_width: i32,
        room_height: i32,
        room_depth: i32,
    ) -> OceanMonumentPiece {
        let bounding_box = OceanMonumentPiece::make_bounding_box(orientation, &rooms[room_definition as usize], room_width, room_height, room_depth);
        let mut piece = StructurePiece::new(piece_type, gen_depth, bounding_box);
        piece.set_orientation(Some(orientation));
        OceanMonumentPiece { piece, room_definition: Some(room_definition) }
    }

    fn make_bounding_box(orientation: Direction, room_definition: &RoomDefinition, room_width: i32, room_height: i32, room_depth: i32) -> BoundingBox {
        let room_index = room_definition.index;
        let room_x = room_index % 5;
        let room_z = room_index / 5 % 5;
        let room_y = room_index / 25;
        let mut bounding_box = structure_piece::make_bounding_box(0, 0, 0, orientation, room_width * 8, room_height * 4, room_depth * 8);
        match orientation {
            Direction::North => bounding_box.move_in_place(room_x * 8, room_y * 4, -(room_z + room_depth) * 8 + 1),
            Direction::South => bounding_box.move_in_place(room_x * 8, room_y * 4, room_z * 8),
            Direction::West => bounding_box.move_in_place(-(room_z + room_depth) * 8 + 1, room_y * 4, room_x * 8),
            _ => bounding_box.move_in_place(room_z * 8, room_y * 4, room_x * 8),
        }
        bounding_box
    }

    fn load(piece_type: &'static str, tag: &Nbt) -> Option<OceanMonumentPiece> {
        Some(OceanMonumentPiece { piece: StructurePiece::load(piece_type, tag)?, room_definition: None })
    }

    fn room_definition<'a>(&self, rooms: &'a [RoomDefinition]) -> &'a RoomDefinition {
        &rooms[self.room_definition.unwrap() as usize]
    }

    #[allow(clippy::too_many_arguments)]
    fn generate_water_box(&self, level: &mut impl WorldGenLevel, chunk_bb: &BoundingBox, x0: i32, y0: i32, z0: i32, x1: i32, y1: i32, z1: i32) {
        let fill_block = fill_block();
        for y in y0..=y1 {
            for x in x0..=x1 {
                for z in z0..=z1 {
                    let block = self.piece.get_block(level, x, y, z, chunk_bb);
                    if !FILL_KEEP.contains(&block.block()) {
                        if self.piece.get_world_y(y) >= level.get_sea_level() && block != fill_block {
                            self.piece.place_block(level, blocks::AIR.default_state(), x, y, z, chunk_bb);
                        } else {
                            self.piece.place_block(level, fill_block, x, y, z, chunk_bb);
                        }
                    }
                }
            }
        }
    }

    fn generate_default_floor(&self, level: &mut impl WorldGenLevel, chunk_bb: &BoundingBox, x_off: i32, z_off: i32, down_opening: bool) {
        let piece = &self.piece;
        let (base_gray, base_light) = (base_gray(), base_light());
        if down_opening {
            piece.generate_box(level, chunk_bb, x_off, 0, z_off, x_off + 2, 0, z_off + 8 - 1, base_gray, base_gray, false);
            piece.generate_box(level, chunk_bb, x_off + 5, 0, z_off, x_off + 8 - 1, 0, z_off + 8 - 1, base_gray, base_gray, false);
            piece.generate_box(level, chunk_bb, x_off + 3, 0, z_off, x_off + 4, 0, z_off + 2, base_gray, base_gray, false);
            piece.generate_box(level, chunk_bb, x_off + 3, 0, z_off + 5, x_off + 4, 0, z_off + 8 - 1, base_gray, base_gray, false);
            piece.generate_box(level, chunk_bb, x_off + 3, 0, z_off + 2, x_off + 4, 0, z_off + 2, base_light, base_light, false);
            piece.generate_box(level, chunk_bb, x_off + 3, 0, z_off + 5, x_off + 4, 0, z_off + 5, base_light, base_light, false);
            piece.generate_box(level, chunk_bb, x_off + 2, 0, z_off + 3, x_off + 2, 0, z_off + 4, base_light, base_light, false);
            piece.generate_box(level, chunk_bb, x_off + 5, 0, z_off + 3, x_off + 5, 0, z_off + 4, base_light, base_light, false);
        } else {
            piece.generate_box(level, chunk_bb, x_off, 0, z_off, x_off + 8 - 1, 0, z_off + 8 - 1, base_gray, base_gray, false);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn generate_box_on_fill_only(&self, level: &mut impl WorldGenLevel, chunk_bb: &BoundingBox, x0: i32, y0: i32, z0: i32, x1: i32, y1: i32, z1: i32, target_block: BlockState) {
        let fill_block = fill_block();
        for y in y0..=y1 {
            for x in x0..=x1 {
                for z in z0..=z1 {
                    if self.piece.get_block(level, x, y, z, chunk_bb) == fill_block {
                        self.piece.place_block(level, target_block, x, y, z, chunk_bb);
                    }
                }
            }
        }
    }

    fn chunk_intersects(&self, chunk_bb: &BoundingBox, x0: i32, z0: i32, x1: i32, z1: i32) -> bool {
        let wx0 = self.piece.get_world_x(x0, z0);
        let wz0 = self.piece.get_world_z(x0, z0);
        let wx1 = self.piece.get_world_x(x1, z1);
        let wz1 = self.piece.get_world_z(x1, z1);
        chunk_bb.intersects_xz(wx0.min(wx1), wz0.min(wz1), wx0.max(wx1), wz0.max(wz1))
    }

    fn spawn_elder(&self, chunk_bb: &BoundingBox, x: i32, y: i32, z: i32) -> bool {
        chunk_bb.is_inside(self.piece.get_world_pos(x, y, z))
    }
}

#[derive(Clone)]
struct OceanMonumentSimpleRoom {
    monument: OceanMonumentPiece,
    main_design: i32,
}

impl OceanMonumentSimpleRoom {
    fn new(orientation: Direction, rooms: &[RoomDefinition], definition: u8, random: &mut impl RandomSource) -> OceanMonumentSimpleRoom {
        let monument = OceanMonumentPiece::new_room(OCEAN_MONUMENT_SIMPLE_ROOM, 1, orientation, rooms, definition, 1, 1, 1);
        OceanMonumentSimpleRoom { monument, main_design: random.next_int_bounded(3) }
    }

    fn post_process(&self, level: &mut impl WorldGenLevel, random: &mut impl RandomSource, chunk_bb: &BoundingBox, rooms: &[RoomDefinition]) {
        let monument = &self.monument;
        let piece = &monument.piece;
        let (base_gray, base_light, base_black, lamp_block) = (base_gray(), base_light(), base_black(), lamp_block());
        let definition = monument.room_definition(rooms);
        if definition.index / 25 > 0 {
            monument.generate_default_floor(level, chunk_bb, 0, 0, definition.has_opening[DOWN]);
        }
        if definition.connections[UP].is_none() {
            monument.generate_box_on_fill_only(level, chunk_bb, 1, 4, 1, 6, 4, 6, base_gray);
        }
        let center_pillar =
            self.main_design != 0 && random.next_boolean() && !definition.has_opening[DOWN] && !definition.has_opening[UP] && definition.count_openings() > 1;
        if self.main_design == 0 {
            piece.generate_box(level, chunk_bb, 0, 1, 0, 2, 1, 2, base_light, base_light, false);
            piece.generate_box(level, chunk_bb, 0, 3, 0, 2, 3, 2, base_light, base_light, false);
            piece.generate_box(level, chunk_bb, 0, 2, 0, 0, 2, 2, base_gray, base_gray, false);
            piece.generate_box(level, chunk_bb, 1, 2, 0, 2, 2, 0, base_gray, base_gray, false);
            piece.place_block(level, lamp_block, 1, 2, 1, chunk_bb);
            piece.generate_box(level, chunk_bb, 5, 1, 0, 7, 1, 2, base_light, base_light, false);
            piece.generate_box(level, chunk_bb, 5, 3, 0, 7, 3, 2, base_light, base_light, false);
            piece.generate_box(level, chunk_bb, 7, 2, 0, 7, 2, 2, base_gray, base_gray, false);
            piece.generate_box(level, chunk_bb, 5, 2, 0, 6, 2, 0, base_gray, base_gray, false);
            piece.place_block(level, lamp_block, 6, 2, 1, chunk_bb);
            piece.generate_box(level, chunk_bb, 0, 1, 5, 2, 1, 7, base_light, base_light, false);
            piece.generate_box(level, chunk_bb, 0, 3, 5, 2, 3, 7, base_light, base_light, false);
            piece.generate_box(level, chunk_bb, 0, 2, 5, 0, 2, 7, base_gray, base_gray, false);
            piece.generate_box(level, chunk_bb, 1, 2, 7, 2, 2, 7, base_gray, base_gray, false);
            piece.place_block(level, lamp_block, 1, 2, 6, chunk_bb);
            piece.generate_box(level, chunk_bb, 5, 1, 5, 7, 1, 7, base_light, base_light, false);
            piece.generate_box(level, chunk_bb, 5, 3, 5, 7, 3, 7, base_light, base_light, false);
            piece.generate_box(level, chunk_bb, 7, 2, 5, 7, 2, 7, base_gray, base_gray, false);
            piece.generate_box(level, chunk_bb, 5, 2, 7, 6, 2, 7, base_gray, base_gray, false);
            piece.place_block(level, lamp_block, 6, 2, 6, chunk_bb);
            if definition.has_opening[SOUTH] {
                piece.generate_box(level, chunk_bb, 3, 3, 0, 4, 3, 0, base_light, base_light, false);
            } else {
                piece.generate_box(level, chunk_bb, 3, 3, 0, 4, 3, 1, base_light, base_light, false);
                piece.generate_box(level, chunk_bb, 3, 2, 0, 4, 2, 0, base_gray, base_gray, false);
                piece.generate_box(level, chunk_bb, 3, 1, 0, 4, 1, 1, base_light, base_light, false);
            }
            if definition.has_opening[NORTH] {
                piece.generate_box(level, chunk_bb, 3, 3, 7, 4, 3, 7, base_light, base_light, false);
            } else {
                piece.generate_box(level, chunk_bb, 3, 3, 6, 4, 3, 7, base_light, base_light, false);
                piece.generate_box(level, chunk_bb, 3, 2, 7, 4, 2, 7, base_gray, base_gray, false);
                piece.generate_box(level, chunk_bb, 3, 1, 6, 4, 1, 7, base_light, base_light, false);
            }
            if definition.has_opening[WEST] {
                piece.generate_box(level, chunk_bb, 0, 3, 3, 0, 3, 4, base_light, base_light, false);
            } else {
                piece.generate_box(level, chunk_bb, 0, 3, 3, 1, 3, 4, base_light, base_light, false);
                piece.generate_box(level, chunk_bb, 0, 2, 3, 0, 2, 4, base_gray, base_gray, false);
                piece.generate_box(level, chunk_bb, 0, 1, 3, 1, 1, 4, base_light, base_light, false);
            }
            if definition.has_opening[EAST] {
                piece.generate_box(level, chunk_bb, 7, 3, 3, 7, 3, 4, base_light, base_light, false);
            } else {
                piece.generate_box(level, chunk_bb, 6, 3, 3, 7, 3, 4, base_light, base_light, false);
                piece.generate_box(level, chunk_bb, 7, 2, 3, 7, 2, 4, base_gray, base_gray, false);
                piece.generate_box(level, chunk_bb, 6, 1, 3, 7, 1, 4, base_light, base_light, false);
            }
        } else if self.main_design == 1 {
            piece.generate_box(level, chunk_bb, 2, 1, 2, 2, 3, 2, base_light, base_light, false);
            piece.generate_box(level, chunk_bb, 2, 1, 5, 2, 3, 5, base_light, base_light, false);
            piece.generate_box(level, chunk_bb, 5, 1, 5, 5, 3, 5, base_light, base_light, false);
            piece.generate_box(level, chunk_bb, 5, 1, 2, 5, 3, 2, base_light, base_light, false);
            piece.place_block(level, lamp_block, 2, 2, 2, chunk_bb);
            piece.place_block(level, lamp_block, 2, 2, 5, chunk_bb);
            piece.place_block(level, lamp_block, 5, 2, 5, chunk_bb);
            piece.place_block(level, lamp_block, 5, 2, 2, chunk_bb);
            piece.generate_box(level, chunk_bb, 0, 1, 0, 1, 3, 0, base_light, base_light, false);
            piece.generate_box(level, chunk_bb, 0, 1, 1, 0, 3, 1, base_light, base_light, false);
            piece.generate_box(level, chunk_bb, 0, 1, 7, 1, 3, 7, base_light, base_light, false);
            piece.generate_box(level, chunk_bb, 0, 1, 6, 0, 3, 6, base_light, base_light, false);
            piece.generate_box(level, chunk_bb, 6, 1, 7, 7, 3, 7, base_light, base_light, false);
            piece.generate_box(level, chunk_bb, 7, 1, 6, 7, 3, 6, base_light, base_light, false);
            piece.generate_box(level, chunk_bb, 6, 1, 0, 7, 3, 0, base_light, base_light, false);
            piece.generate_box(level, chunk_bb, 7, 1, 1, 7, 3, 1, base_light, base_light, false);
            piece.place_block(level, base_gray, 1, 2, 0, chunk_bb);
            piece.place_block(level, base_gray, 0, 2, 1, chunk_bb);
            piece.place_block(level, base_gray, 1, 2, 7, chunk_bb);
            piece.place_block(level, base_gray, 0, 2, 6, chunk_bb);
            piece.place_block(level, base_gray, 6, 2, 7, chunk_bb);
            piece.place_block(level, base_gray, 7, 2, 6, chunk_bb);
            piece.place_block(level, base_gray, 6, 2, 0, chunk_bb);
            piece.place_block(level, base_gray, 7, 2, 1, chunk_bb);
            if !definition.has_opening[SOUTH] {
                piece.generate_box(level, chunk_bb, 1, 3, 0, 6, 3, 0, base_light, base_light, false);
                piece.generate_box(level, chunk_bb, 1, 2, 0, 6, 2, 0, base_gray, base_gray, false);
                piece.generate_box(level, chunk_bb, 1, 1, 0, 6, 1, 0, base_light, base_light, false);
            }
            if !definition.has_opening[NORTH] {
                piece.generate_box(level, chunk_bb, 1, 3, 7, 6, 3, 7, base_light, base_light, false);
                piece.generate_box(level, chunk_bb, 1, 2, 7, 6, 2, 7, base_gray, base_gray, false);
                piece.generate_box(level, chunk_bb, 1, 1, 7, 6, 1, 7, base_light, base_light, false);
            }
            if !definition.has_opening[WEST] {
                piece.generate_box(level, chunk_bb, 0, 3, 1, 0, 3, 6, base_light, base_light, false);
                piece.generate_box(level, chunk_bb, 0, 2, 1, 0, 2, 6, base_gray, base_gray, false);
                piece.generate_box(level, chunk_bb, 0, 1, 1, 0, 1, 6, base_light, base_light, false);
            }
            if !definition.has_opening[EAST] {
                piece.generate_box(level, chunk_bb, 7, 3, 1, 7, 3, 6, base_light, base_light, false);
                piece.generate_box(level, chunk_bb, 7, 2, 1, 7, 2, 6, base_gray, base_gray, false);
                piece.generate_box(level, chunk_bb, 7, 1, 1, 7, 1, 6, base_light, base_light, false);
            }
        } else if self.main_design == 2 {
            piece.generate_box(level, chunk_bb, 0, 1, 0, 0, 1, 7, base_light, base_light, false);
            piece.generate_box(level, chunk_bb, 7, 1, 0, 7, 1, 7, base_light, base_light, false);
            piece.generate_box(level, chunk_bb, 1, 1, 0, 6, 1, 0, base_light, base_light, false);
            piece.generate_box(level, chunk_bb, 1, 1, 7, 6, 1, 7, base_light, base_light, false);
            piece.generate_box(level, chunk_bb, 0, 2, 0, 0, 2, 7, base_black, base_black, false);
            piece.generate_box(level, chunk_bb, 7, 2, 0, 7, 2, 7, base_black, base_black, false);
            piece.generate_box(level, chunk_bb, 1, 2, 0, 6, 2, 0, base_black, base_black, false);
            piece.generate_box(level, chunk_bb, 1, 2, 7, 6, 2, 7, base_black, base_black, false);
            piece.generate_box(level, chunk_bb, 0, 3, 0, 0, 3, 7, base_light, base_light, false);
            piece.generate_box(level, chunk_bb, 7, 3, 0, 7, 3, 7, base_light, base_light, false);
            piece.generate_box(level, chunk_bb, 1, 3, 0, 6, 3, 0, base_light, base_light, false);
            piece.generate_box(level, chunk_bb, 1, 3, 7, 6, 3, 7, base_light, base_light, false);
            piece.generate_box(level, chunk_bb, 0, 1, 3, 0, 2, 4, base_black, base_black, false);
            piece.generate_box(level, chunk_bb, 7, 1, 3, 7, 2, 4, base_black, base_black, false);
            piece.generate_box(level, chunk_bb, 3, 1, 0, 4, 2, 0, base_black, base_black, false);
            piece.generate_box(level, chunk_bb, 3, 1, 7, 4, 2, 7, base_black, base_black, false);
            if definition.has_opening[SOUTH] {
                monument.generate_water_box(level, chunk_bb, 3, 1, 0, 4, 2, 0);
            }
            if definition.has_opening[NORTH] {
                monument.generate_water_box(level, chunk_bb, 3, 1, 7, 4, 2, 7);
            }
            if definition.has_opening[WEST] {
                monument.generate_water_box(level, chunk_bb, 0, 1, 3, 0, 2, 4);
            }
            if definition.has_opening[EAST] {
                monument.generate_water_box(level, chunk_bb, 7, 1, 3, 7, 2, 4);
            }
        }
        if center_pillar {
            piece.generate_box(level, chunk_bb, 3, 1, 3, 4, 1, 4, base_light, base_light, false);
            piece.generate_box(level, chunk_bb, 3, 2, 3, 4, 2, 4, base_gray, base_gray, false);
            piece.generate_box(level, chunk_bb, 3, 3, 3, 4, 3, 4, base_light, base_light, false);
        }
    }
}

#[derive(Clone)]
struct OceanMonumentSimpleTopRoom {
    monument: OceanMonumentPiece,
}

impl OceanMonumentSimpleTopRoom {
    fn new(orientation: Direction, rooms: &[RoomDefinition], definition: u8) -> OceanMonumentSimpleTopRoom {
        OceanMonumentSimpleTopRoom { monument: OceanMonumentPiece::new_room(OCEAN_MONUMENT_SIMPLE_TOP_ROOM, 1, orientation, rooms, definition, 1, 1, 1) }
    }

    fn post_process(&self, level: &mut impl WorldGenLevel, random: &mut impl RandomSource, chunk_bb: &BoundingBox, rooms: &[RoomDefinition]) {
        let monument = &self.monument;
        let piece = &monument.piece;
        let (base_gray, base_light, base_black) = (base_gray(), base_light(), base_black());
        let definition = monument.room_definition(rooms);
        if definition.index / 25 > 0 {
            monument.generate_default_floor(level, chunk_bb, 0, 0, definition.has_opening[DOWN]);
        }
        if definition.connections[UP].is_none() {
            monument.generate_box_on_fill_only(level, chunk_bb, 1, 4, 1, 6, 4, 6, base_gray);
        }
        for x in 1..=6 {
            for z in 1..=6 {
                if random.next_int_bounded(3) != 0 {
                    let y0 = 2 + if random.next_int_bounded(4) == 0 { 0 } else { 1 };
                    let wet_sponge = blocks::WET_SPONGE.default_state();
                    piece.generate_box(level, chunk_bb, x, y0, z, x, 3, z, wet_sponge, wet_sponge, false);
                }
            }
        }
        piece.generate_box(level, chunk_bb, 0, 1, 0, 0, 1, 7, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 7, 1, 0, 7, 1, 7, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 1, 1, 0, 6, 1, 0, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 1, 1, 7, 6, 1, 7, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 0, 2, 0, 0, 2, 7, base_black, base_black, false);
        piece.generate_box(level, chunk_bb, 7, 2, 0, 7, 2, 7, base_black, base_black, false);
        piece.generate_box(level, chunk_bb, 1, 2, 0, 6, 2, 0, base_black, base_black, false);
        piece.generate_box(level, chunk_bb, 1, 2, 7, 6, 2, 7, base_black, base_black, false);
        piece.generate_box(level, chunk_bb, 0, 3, 0, 0, 3, 7, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 7, 3, 0, 7, 3, 7, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 1, 3, 0, 6, 3, 0, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 1, 3, 7, 6, 3, 7, base_light, base_light, false);
        piece.generate_box(level, chunk_bb, 0, 1, 3, 0, 2, 4, base_black, base_black, false);
        piece.generate_box(level, chunk_bb, 7, 1, 3, 7, 2, 4, base_black, base_black, false);
        piece.generate_box(level, chunk_bb, 3, 1, 0, 4, 2, 0, base_black, base_black, false);
        piece.generate_box(level, chunk_bb, 3, 1, 7, 4, 2, 7, base_black, base_black, false);
        if definition.has_opening[SOUTH] {
            monument.generate_water_box(level, chunk_bb, 3, 1, 0, 4, 2, 0);
        }
    }
}

#[derive(Clone)]
struct OceanMonumentWingRoom {
    monument: OceanMonumentPiece,
    main_design: i32,
}

impl OceanMonumentWingRoom {
    fn new(orientation: Direction, bounding_box: BoundingBox, random_value: i32) -> OceanMonumentWingRoom {
        OceanMonumentWingRoom { monument: OceanMonumentPiece::new(OCEAN_MONUMENT_WING_ROOM, orientation, 1, bounding_box), main_design: random_value & 1 }
    }

    fn post_process(&self, level: &mut impl WorldGenLevel, chunk_bb: &BoundingBox) {
        let monument = &self.monument;
        let piece = &monument.piece;
        let (base_light, base_black, lamp_block) = (base_light(), base_black(), lamp_block());
        if self.main_design == 0 {
            for i in 0..4 {
                piece.generate_box(level, chunk_bb, 10 - i, 3 - i, 20 - i, 12 + i, 3 - i, 20, base_light, base_light, false);
            }
            piece.generate_box(level, chunk_bb, 7, 0, 6, 15, 0, 16, base_light, base_light, false);
            piece.generate_box(level, chunk_bb, 6, 0, 6, 6, 3, 20, base_light, base_light, false);
            piece.generate_box(level, chunk_bb, 16, 0, 6, 16, 3, 20, base_light, base_light, false);
            piece.generate_box(level, chunk_bb, 7, 1, 7, 7, 1, 20, base_light, base_light, false);
            piece.generate_box(level, chunk_bb, 15, 1, 7, 15, 1, 20, base_light, base_light, false);
            piece.generate_box(level, chunk_bb, 7, 1, 6, 9, 3, 6, base_light, base_light, false);
            piece.generate_box(level, chunk_bb, 13, 1, 6, 15, 3, 6, base_light, base_light, false);
            piece.generate_box(level, chunk_bb, 8, 1, 7, 9, 1, 7, base_light, base_light, false);
            piece.generate_box(level, chunk_bb, 13, 1, 7, 14, 1, 7, base_light, base_light, false);
            piece.generate_box(level, chunk_bb, 9, 0, 5, 13, 0, 5, base_light, base_light, false);
            piece.generate_box(level, chunk_bb, 10, 0, 7, 12, 0, 7, base_black, base_black, false);
            piece.generate_box(level, chunk_bb, 8, 0, 10, 8, 0, 12, base_black, base_black, false);
            piece.generate_box(level, chunk_bb, 14, 0, 10, 14, 0, 12, base_black, base_black, false);
            for z in (7..=18).rev().step_by(3) {
                piece.place_block(level, lamp_block, 6, 3, z, chunk_bb);
                piece.place_block(level, lamp_block, 16, 3, z, chunk_bb);
            }
            piece.place_block(level, lamp_block, 10, 0, 10, chunk_bb);
            piece.place_block(level, lamp_block, 12, 0, 10, chunk_bb);
            piece.place_block(level, lamp_block, 10, 0, 12, chunk_bb);
            piece.place_block(level, lamp_block, 12, 0, 12, chunk_bb);
            piece.place_block(level, lamp_block, 8, 3, 6, chunk_bb);
            piece.place_block(level, lamp_block, 14, 3, 6, chunk_bb);
            piece.place_block(level, base_light, 4, 2, 4, chunk_bb);
            piece.place_block(level, lamp_block, 4, 1, 4, chunk_bb);
            piece.place_block(level, base_light, 4, 0, 4, chunk_bb);
            piece.place_block(level, base_light, 18, 2, 4, chunk_bb);
            piece.place_block(level, lamp_block, 18, 1, 4, chunk_bb);
            piece.place_block(level, base_light, 18, 0, 4, chunk_bb);
            piece.place_block(level, base_light, 4, 2, 18, chunk_bb);
            piece.place_block(level, lamp_block, 4, 1, 18, chunk_bb);
            piece.place_block(level, base_light, 4, 0, 18, chunk_bb);
            piece.place_block(level, base_light, 18, 2, 18, chunk_bb);
            piece.place_block(level, lamp_block, 18, 1, 18, chunk_bb);
            piece.place_block(level, base_light, 18, 0, 18, chunk_bb);
            piece.place_block(level, base_light, 9, 7, 20, chunk_bb);
            piece.place_block(level, base_light, 13, 7, 20, chunk_bb);
            piece.generate_box(level, chunk_bb, 6, 0, 21, 7, 4, 21, base_light, base_light, false);
            piece.generate_box(level, chunk_bb, 15, 0, 21, 16, 4, 21, base_light, base_light, false);
            monument.spawn_elder(chunk_bb, 11, 2, 16);
        } else if self.main_design == 1 {
            piece.generate_box(level, chunk_bb, 9, 3, 18, 13, 3, 20, base_light, base_light, false);
            piece.generate_box(level, chunk_bb, 9, 0, 18, 9, 2, 18, base_light, base_light, false);
            piece.generate_box(level, chunk_bb, 13, 0, 18, 13, 2, 18, base_light, base_light, false);
            let mut x = 9;
            for _ in 0..2 {
                piece.place_block(level, base_light, x, 6, 20, chunk_bb);
                piece.place_block(level, lamp_block, x, 5, 20, chunk_bb);
                piece.place_block(level, base_light, x, 4, 20, chunk_bb);
                x = 13;
            }
            piece.generate_box(level, chunk_bb, 7, 3, 7, 15, 3, 14, base_light, base_light, false);
            let mut x = 10;
            for _ in 0..2 {
                piece.generate_box(level, chunk_bb, x, 0, 10, x, 6, 10, base_light, base_light, false);
                piece.generate_box(level, chunk_bb, x, 0, 12, x, 6, 12, base_light, base_light, false);
                piece.place_block(level, lamp_block, x, 0, 10, chunk_bb);
                piece.place_block(level, lamp_block, x, 0, 12, chunk_bb);
                piece.place_block(level, lamp_block, x, 4, 10, chunk_bb);
                piece.place_block(level, lamp_block, x, 4, 12, chunk_bb);
                x = 12;
            }
            x = 8;
            for _ in 0..2 {
                piece.generate_box(level, chunk_bb, x, 0, 7, x, 2, 7, base_light, base_light, false);
                piece.generate_box(level, chunk_bb, x, 0, 14, x, 2, 14, base_light, base_light, false);
                x = 14;
            }
            piece.generate_box(level, chunk_bb, 8, 3, 8, 8, 3, 13, base_black, base_black, false);
            piece.generate_box(level, chunk_bb, 14, 3, 8, 14, 3, 13, base_black, base_black, false);
            monument.spawn_elder(chunk_bb, 11, 5, 13);
        }
    }
}

#[derive(Clone)]
struct RoomDefinition {
    index: i32,
    connections: [Option<u8>; 6],
    has_opening: [bool; 6],
    claimed: bool,
    is_source: bool,
    scan_index: i32,
}

impl RoomDefinition {
    fn create(rooms: &mut Vec<RoomDefinition>, room_index: i32) -> u8 {
        rooms.push(RoomDefinition { index: room_index, connections: [None; 6], has_opening: [false; 6], claimed: false, is_source: false, scan_index: 0 });
        (rooms.len() - 1) as u8
    }

    fn connection<'a>(&self, rooms: &'a [RoomDefinition], direction: usize) -> &'a RoomDefinition {
        &rooms[self.connections[direction].unwrap() as usize]
    }

    fn set_connection(rooms: &mut [RoomDefinition], room: u8, direction: Direction, definition: u8) {
        rooms[room as usize].connections[direction.ordinal()] = Some(definition);
        rooms[definition as usize].connections[direction.opposite().ordinal()] = Some(room);
    }

    fn update_openings(&mut self) {
        for i in 0..6 {
            self.has_opening[i] = self.connections[i].is_some();
        }
    }

    fn find_source(rooms: &mut [RoomDefinition], room: u8, scan_index: i32) -> bool {
        if rooms[room as usize].is_source {
            return true;
        }
        rooms[room as usize].scan_index = scan_index;
        for i in 0..6 {
            if let Some(connection) = rooms[room as usize].connections[i]
                && rooms[room as usize].has_opening[i]
                && rooms[connection as usize].scan_index != scan_index
                && RoomDefinition::find_source(rooms, connection, scan_index)
            {
                return true;
            }
        }
        false
    }

    fn is_special(&self) -> bool {
        self.index >= 75
    }

    fn count_openings(&self) -> i32 {
        self.has_opening.iter().filter(|&&opening| opening).count() as i32
    }
}
