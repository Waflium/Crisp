use std::sync::LazyLock;

use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, Direction};
use crisp_data::blocks;
use crisp_data::properties::HORIZONTAL_FACING;
use crisp_protocol::nbt::Nbt;

use crate::server::level::world_gen_region::WorldGenRegion;
use crate::util::shuffle;
use crate::world::entity::entity_types;
use crate::world::level::LevelAccessor;
use crate::world::level::block::mirror::Mirror;
use crate::world::level::block::rotation::Rotation;
use crate::world::level::levelgen::structure::BoundingBox;
use crate::world::level::levelgen::structure::structure_piece::StructurePiece;
use crate::world::level::levelgen::structure::template_structure_piece::{TemplatePlaceSettings, TemplateStructurePiece};
use crate::world::level::levelgen::structure::templatesystem::structure_processor::StructureProcessor;
use crate::world::level::levelgen::structure::templatesystem::structure_template::get_zero_position_with_transform;

pub const WOODLAND_MANSION_PIECE: &str = "minecraft:wmp";
const WOODLAND_MANSION: &str = "minecraft:chests/woodland_mansion";

static STRUCTURE_BLOCK: LazyLock<[StructureProcessor; 1]> = LazyLock::new(|| [StructureProcessor::BlockIgnore(vec![blocks::STRUCTURE_BLOCK])]);

pub fn generate_mansion(origin: BlockPos, rotation: Rotation, pieces: &mut Vec<WoodlandMansionPiece>, random: &mut impl RandomSource) {
    let grid = MansionGrid::new(random);
    let mut placer = MansionPiecePlacer::new(random);
    placer.create_mansion(origin, rotation, pieces, &grid);
}

fn rotate(pos: BlockPos, rotation: Rotation) -> BlockPos {
    match rotation {
        Rotation::Clockwise90 => BlockPos::new(-pos.z, pos.y, pos.x),
        Rotation::Clockwise180 => BlockPos::new(-pos.x, pos.y, -pos.z),
        Rotation::Counterclockwise90 => BlockPos::new(pos.z, pos.y, -pos.x),
        Rotation::None => pos,
    }
}

#[derive(Clone, Copy)]
enum FloorRoomCollection {
    FirstFloor,
    SecondFloor,
    ThirdFloor,
}

impl FloorRoomCollection {
    fn get_1x1(self, random: &mut impl RandomSource) -> String {
        match self {
            FloorRoomCollection::FirstFloor => format!("1x1_a{}", random.next_int_bounded(5) + 1),
            FloorRoomCollection::SecondFloor | FloorRoomCollection::ThirdFloor => format!("1x1_b{}", random.next_int_bounded(5) + 1),
        }
    }

    fn get_1x1_secret(self, random: &mut impl RandomSource) -> String {
        format!("1x1_as{}", random.next_int_bounded(4) + 1)
    }

    fn get_1x2_side_entrance(self, random: &mut impl RandomSource, is_stairs_room: bool) -> String {
        match self {
            FloorRoomCollection::FirstFloor => format!("1x2_a{}", random.next_int_bounded(9) + 1),
            FloorRoomCollection::SecondFloor | FloorRoomCollection::ThirdFloor => {
                if is_stairs_room {
                    "1x2_c_stairs".to_owned()
                } else {
                    format!("1x2_c{}", random.next_int_bounded(4) + 1)
                }
            }
        }
    }

    fn get_1x2_front_entrance(self, random: &mut impl RandomSource, is_stairs_room: bool) -> String {
        match self {
            FloorRoomCollection::FirstFloor => format!("1x2_b{}", random.next_int_bounded(5) + 1),
            FloorRoomCollection::SecondFloor | FloorRoomCollection::ThirdFloor => {
                if is_stairs_room {
                    "1x2_d_stairs".to_owned()
                } else {
                    format!("1x2_d{}", random.next_int_bounded(5) + 1)
                }
            }
        }
    }

    fn get_1x2_secret(self, random: &mut impl RandomSource) -> String {
        match self {
            FloorRoomCollection::FirstFloor => format!("1x2_s{}", random.next_int_bounded(2) + 1),
            FloorRoomCollection::SecondFloor | FloorRoomCollection::ThirdFloor => format!("1x2_se{}", random.next_int_bounded(1) + 1),
        }
    }

    fn get_2x2(self, random: &mut impl RandomSource) -> String {
        match self {
            FloorRoomCollection::FirstFloor => format!("2x2_a{}", random.next_int_bounded(4) + 1),
            FloorRoomCollection::SecondFloor | FloorRoomCollection::ThirdFloor => format!("2x2_b{}", random.next_int_bounded(5) + 1),
        }
    }

    fn get_2x2_secret(self) -> String {
        "2x2_s1".to_owned()
    }
}

const DEFAULT_SIZE: usize = 11;
const CLEAR: i32 = 0;
const CORRIDOR: i32 = 1;
const ROOM: i32 = 2;
const START_ROOM: i32 = 3;
const TEST_ROOM: i32 = 4;
const BLOCKED: i32 = 5;
const ROOM_1X1: i32 = 65536;
const ROOM_1X2: i32 = 131072;
const ROOM_2X2: i32 = 262144;
const ROOM_ORIGIN_FLAG: i32 = 1048576;
const ROOM_DOOR_FLAG: i32 = 2097152;
const ROOM_STAIRS_FLAG: i32 = 4194304;
const ROOM_CORRIDOR_FLAG: i32 = 8388608;
const ROOM_TYPE_MASK: i32 = 983040;
const ROOM_ID_MASK: i32 = 65535;

#[derive(Clone, Copy)]
struct GridPos {
    x: i32,
    y: i32,
}

struct MansionGrid {
    base_grid: SimpleGrid,
    third_floor_grid: SimpleGrid,
    floor_rooms: [SimpleGrid; 3],
    entrance_x: i32,
    entrance_y: i32,
}

impl MansionGrid {
    fn new(random: &mut impl RandomSource) -> MansionGrid {
        let entrance_x = 7;
        let entrance_y = 4;
        let mut base_grid = SimpleGrid::new(BLOCKED);
        base_grid.set_area(entrance_x, entrance_y, entrance_x + 1, entrance_y + 1, START_ROOM);
        base_grid.set_area(entrance_x - 1, entrance_y, entrance_x - 1, entrance_y + 1, ROOM);
        base_grid.set_area(entrance_x + 2, entrance_y - 2, entrance_x + 3, entrance_y + 3, BLOCKED);
        base_grid.set_area(entrance_x + 1, entrance_y - 2, entrance_x + 1, entrance_y - 1, CORRIDOR);
        base_grid.set_area(entrance_x + 1, entrance_y + 2, entrance_x + 1, entrance_y + 3, CORRIDOR);
        base_grid.set(entrance_x - 1, entrance_y - 1, CORRIDOR);
        base_grid.set(entrance_x - 1, entrance_y + 2, CORRIDOR);
        base_grid.set_area(0, 0, 11, 1, BLOCKED);
        base_grid.set_area(0, 9, 11, 11, BLOCKED);
        MansionGrid::recursive_corridor(&mut base_grid, random, entrance_x, entrance_y - 2, Direction::West, 6);
        MansionGrid::recursive_corridor(&mut base_grid, random, entrance_x, entrance_y + 3, Direction::West, 6);
        MansionGrid::recursive_corridor(&mut base_grid, random, entrance_x - 2, entrance_y - 1, Direction::West, 3);
        MansionGrid::recursive_corridor(&mut base_grid, random, entrance_x - 2, entrance_y + 2, Direction::West, 3);
        while MansionGrid::clean_edges(&mut base_grid) {}
        let mut floor_rooms = [SimpleGrid::new(BLOCKED), SimpleGrid::new(BLOCKED), SimpleGrid::new(BLOCKED)];
        MansionGrid::identify_rooms(&base_grid, &mut floor_rooms[0], random);
        MansionGrid::identify_rooms(&base_grid, &mut floor_rooms[1], random);
        floor_rooms[0].set_area(entrance_x + 1, entrance_y, entrance_x + 1, entrance_y + 1, ROOM_CORRIDOR_FLAG);
        floor_rooms[1].set_area(entrance_x + 1, entrance_y, entrance_x + 1, entrance_y + 1, ROOM_CORRIDOR_FLAG);
        let mut mansion = MansionGrid { base_grid, third_floor_grid: SimpleGrid::new(BLOCKED), floor_rooms, entrance_x, entrance_y };
        mansion.setup_third_floor(random);
        MansionGrid::identify_rooms(&mansion.third_floor_grid, &mut mansion.floor_rooms[2], random);
        mansion
    }

    fn is_house(grid: &SimpleGrid, x: i32, y: i32) -> bool {
        let value = grid.get(x, y);
        value == CORRIDOR || value == ROOM || value == START_ROOM || value == TEST_ROOM
    }

    fn is_room_id(&self, x: i32, y: i32, floor: usize, room_id: i32) -> bool {
        (self.floor_rooms[floor].get(x, y) & ROOM_ID_MASK) == room_id
    }

    fn get_1x2_room_direction(&self, x: i32, y: i32, floor_num: usize, room_id: i32) -> Option<Direction> {
        Direction::PLANE_HORIZONTAL.into_iter().find(|direction| self.is_room_id(x + direction.step_x(), y + direction.step_z(), floor_num, room_id))
    }

    fn recursive_corridor(grid: &mut SimpleGrid, random: &mut impl RandomSource, x: i32, y: i32, heading: Direction, depth: i32) {
        if depth > 0 {
            grid.set(x, y, CORRIDOR);
            grid.setif(x + heading.step_x(), y + heading.step_z(), CLEAR, CORRIDOR);
            for _ in 0..8 {
                let next_dir = Direction::from_2d_data_value(random.next_int_bounded(4));
                if next_dir != heading.opposite() && (next_dir != Direction::East || !random.next_boolean()) {
                    let nx = x + heading.step_x();
                    let ny = y + heading.step_z();
                    if grid.get(nx + next_dir.step_x(), ny + next_dir.step_z()) == CLEAR && grid.get(nx + next_dir.step_x() * 2, ny + next_dir.step_z() * 2) == CLEAR {
                        MansionGrid::recursive_corridor(
                            grid,
                            random,
                            x + heading.step_x() + next_dir.step_x(),
                            y + heading.step_z() + next_dir.step_z(),
                            next_dir,
                            depth - 1,
                        );
                        break;
                    }
                }
            }
            let cw = heading.clockwise();
            let ccw = heading.counter_clockwise();
            grid.setif(x + cw.step_x(), y + cw.step_z(), CLEAR, ROOM);
            grid.setif(x + ccw.step_x(), y + ccw.step_z(), CLEAR, ROOM);
            grid.setif(x + heading.step_x() + cw.step_x(), y + heading.step_z() + cw.step_z(), CLEAR, ROOM);
            grid.setif(x + heading.step_x() + ccw.step_x(), y + heading.step_z() + ccw.step_z(), CLEAR, ROOM);
            grid.setif(x + heading.step_x() * 2, y + heading.step_z() * 2, CLEAR, ROOM);
            grid.setif(x + cw.step_x() * 2, y + cw.step_z() * 2, CLEAR, ROOM);
            grid.setif(x + ccw.step_x() * 2, y + ccw.step_z() * 2, CLEAR, ROOM);
        }
    }

    fn clean_edges(grid: &mut SimpleGrid) -> bool {
        let mut touched = false;
        for y in 0..grid.height {
            for x in 0..grid.width {
                if grid.get(x, y) == CLEAR {
                    let mut direct_neighbors = 0;
                    direct_neighbors += MansionGrid::is_house(grid, x + 1, y) as i32;
                    direct_neighbors += MansionGrid::is_house(grid, x - 1, y) as i32;
                    direct_neighbors += MansionGrid::is_house(grid, x, y + 1) as i32;
                    direct_neighbors += MansionGrid::is_house(grid, x, y - 1) as i32;
                    if direct_neighbors >= 3 {
                        grid.set(x, y, ROOM);
                        touched = true;
                    } else if direct_neighbors == 2 {
                        let mut diagonal_neighbors = 0;
                        diagonal_neighbors += MansionGrid::is_house(grid, x + 1, y + 1) as i32;
                        diagonal_neighbors += MansionGrid::is_house(grid, x - 1, y + 1) as i32;
                        diagonal_neighbors += MansionGrid::is_house(grid, x + 1, y - 1) as i32;
                        diagonal_neighbors += MansionGrid::is_house(grid, x - 1, y - 1) as i32;
                        if diagonal_neighbors <= 1 {
                            grid.set(x, y, ROOM);
                            touched = true;
                        }
                    }
                }
            }
        }
        touched
    }

    fn setup_third_floor(&mut self, random: &mut impl RandomSource) {
        let mut potential_rooms = Vec::new();
        for y in 0..self.third_floor_grid.height {
            for x in 0..self.third_floor_grid.width {
                let room_data = self.floor_rooms[1].get(x, y);
                let room_type = room_data & ROOM_TYPE_MASK;
                if room_type == ROOM_1X2 && (room_data & ROOM_DOOR_FLAG) == ROOM_DOOR_FLAG {
                    potential_rooms.push(GridPos { x, y });
                }
            }
        }
        if potential_rooms.is_empty() {
            let (width, height) = (self.third_floor_grid.width, self.third_floor_grid.height);
            self.third_floor_grid.set_area(0, 0, width, height, BLOCKED);
        } else {
            let room_pos = potential_rooms[random.next_int_bounded(potential_rooms.len() as i32) as usize];
            let room_data = self.floor_rooms[1].get(room_pos.x, room_pos.y);
            self.floor_rooms[1].set(room_pos.x, room_pos.y, room_data | ROOM_STAIRS_FLAG);
            let room_dir = self.get_1x2_room_direction(room_pos.x, room_pos.y, 1, room_data & ROOM_ID_MASK).unwrap();
            let room_end_x = room_pos.x + room_dir.step_x();
            let room_end_y = room_pos.y + room_dir.step_z();
            for y in 0..self.third_floor_grid.height {
                for x in 0..self.third_floor_grid.width {
                    if !MansionGrid::is_house(&self.base_grid, x, y) {
                        self.third_floor_grid.set(x, y, BLOCKED);
                    } else if x == room_pos.x && y == room_pos.y {
                        self.third_floor_grid.set(x, y, START_ROOM);
                    } else if x == room_end_x && y == room_end_y {
                        self.third_floor_grid.set(x, y, START_ROOM);
                        self.floor_rooms[2].set(x, y, ROOM_CORRIDOR_FLAG);
                    }
                }
            }
            let mut potential_corridors = Vec::new();
            for direction in Direction::PLANE_HORIZONTAL {
                if self.third_floor_grid.get(room_end_x + direction.step_x(), room_end_y + direction.step_z()) == CLEAR {
                    potential_corridors.push(direction);
                }
            }
            if potential_corridors.is_empty() {
                let (width, height) = (self.third_floor_grid.width, self.third_floor_grid.height);
                self.third_floor_grid.set_area(0, 0, width, height, BLOCKED);
                self.floor_rooms[1].set(room_pos.x, room_pos.y, room_data);
            } else {
                let corridor_dir = potential_corridors[random.next_int_bounded(potential_corridors.len() as i32) as usize];
                MansionGrid::recursive_corridor(
                    &mut self.third_floor_grid,
                    random,
                    room_end_x + corridor_dir.step_x(),
                    room_end_y + corridor_dir.step_z(),
                    corridor_dir,
                    4,
                );
                while MansionGrid::clean_edges(&mut self.third_floor_grid) {}
            }
        }
    }

    fn identify_rooms(from_grid: &SimpleGrid, room_grid: &mut SimpleGrid, random: &mut impl RandomSource) {
        let mut room_pos = Vec::new();
        for y in 0..from_grid.height {
            for x in 0..from_grid.width {
                if from_grid.get(x, y) == ROOM {
                    room_pos.push(GridPos { x, y });
                }
            }
        }
        shuffle(&mut room_pos, random);
        let mut room_id = 10;
        for pos in room_pos {
            let x = pos.x;
            let y = pos.y;
            if room_grid.get(x, y) == CLEAR {
                let mut x0 = x;
                let mut x1 = x;
                let mut y0 = y;
                let mut y1 = y;
                let mut room_type = ROOM_1X1;
                if room_grid.get(x + 1, y) == CLEAR
                    && room_grid.get(x, y + 1) == CLEAR
                    && room_grid.get(x + 1, y + 1) == CLEAR
                    && from_grid.get(x + 1, y) == ROOM
                    && from_grid.get(x, y + 1) == ROOM
                    && from_grid.get(x + 1, y + 1) == ROOM
                {
                    x1 += 1;
                    y1 += 1;
                    room_type = ROOM_2X2;
                } else if room_grid.get(x - 1, y) == CLEAR
                    && room_grid.get(x, y + 1) == CLEAR
                    && room_grid.get(x - 1, y + 1) == CLEAR
                    && from_grid.get(x - 1, y) == ROOM
                    && from_grid.get(x, y + 1) == ROOM
                    && from_grid.get(x - 1, y + 1) == ROOM
                {
                    x0 -= 1;
                    y1 += 1;
                    room_type = ROOM_2X2;
                } else if room_grid.get(x - 1, y) == CLEAR
                    && room_grid.get(x, y - 1) == CLEAR
                    && room_grid.get(x - 1, y - 1) == CLEAR
                    && from_grid.get(x - 1, y) == ROOM
                    && from_grid.get(x, y - 1) == ROOM
                    && from_grid.get(x - 1, y - 1) == ROOM
                {
                    x0 -= 1;
                    y0 -= 1;
                    room_type = ROOM_2X2;
                } else if room_grid.get(x + 1, y) == CLEAR && from_grid.get(x + 1, y) == ROOM {
                    x1 += 1;
                    room_type = ROOM_1X2;
                } else if room_grid.get(x, y + 1) == CLEAR && from_grid.get(x, y + 1) == ROOM {
                    y1 += 1;
                    room_type = ROOM_1X2;
                } else if room_grid.get(x - 1, y) == CLEAR && from_grid.get(x - 1, y) == ROOM {
                    x0 -= 1;
                    room_type = ROOM_1X2;
                } else if room_grid.get(x, y - 1) == CLEAR && from_grid.get(x, y - 1) == ROOM {
                    y0 -= 1;
                    room_type = ROOM_1X2;
                }
                let mut door_x = if random.next_boolean() { x0 } else { x1 };
                let mut door_y = if random.next_boolean() { y0 } else { y1 };
                let mut door_flag = ROOM_DOOR_FLAG;
                if !from_grid.edges_to(door_x, door_y, CORRIDOR) {
                    door_x = if door_x == x0 { x1 } else { x0 };
                    door_y = if door_y == y0 { y1 } else { y0 };
                    if !from_grid.edges_to(door_x, door_y, CORRIDOR) {
                        door_y = if door_y == y0 { y1 } else { y0 };
                        if !from_grid.edges_to(door_x, door_y, CORRIDOR) {
                            door_x = if door_x == x0 { x1 } else { x0 };
                            door_y = if door_y == y0 { y1 } else { y0 };
                            if !from_grid.edges_to(door_x, door_y, CORRIDOR) {
                                door_flag = 0;
                                door_x = x0;
                                door_y = y0;
                            }
                        }
                    }
                }
                for ry in y0..=y1 {
                    for rx in x0..=x1 {
                        if rx == door_x && ry == door_y {
                            room_grid.set(rx, ry, ROOM_ORIGIN_FLAG | door_flag | room_type | room_id);
                        } else {
                            room_grid.set(rx, ry, room_type | room_id);
                        }
                    }
                }
                room_id += 1;
            }
        }
    }
}

struct MansionPiecePlacer<'r, R> {
    random: &'r mut R,
    start_x: i32,
    start_y: i32,
}

struct PlacementData {
    rotation: Rotation,
    position: BlockPos,
    wall_type: &'static str,
}

impl<'r, R: RandomSource> MansionPiecePlacer<'r, R> {
    fn new(random: &'r mut R) -> MansionPiecePlacer<'r, R> {
        MansionPiecePlacer { random, start_x: 0, start_y: 0 }
    }

    fn create_mansion(&mut self, origin: BlockPos, rotation: Rotation, pieces: &mut Vec<WoodlandMansionPiece>, mansion: &MansionGrid) {
        let mut data = PlacementData { position: origin, rotation, wall_type: "wall_flat" };
        self.entrance(pieces, &mut data);
        let mut second_data = PlacementData { position: data.position.offset(0, 8, 0), rotation: data.rotation, wall_type: "wall_window" };
        let base_grid = &mansion.base_grid;
        let third_grid = &mansion.third_floor_grid;
        self.start_x = mansion.entrance_x + 1;
        self.start_y = mansion.entrance_y + 1;
        let end_x = mansion.entrance_x + 1;
        let end_y = mansion.entrance_y;
        self.traverse_outer_walls(pieces, &mut data, base_grid, Direction::South, self.start_x, self.start_y, end_x, end_y);
        self.traverse_outer_walls(pieces, &mut second_data, base_grid, Direction::South, self.start_x, self.start_y, end_x, end_y);
        let mut third_data = PlacementData { position: data.position.offset(0, 19, 0), rotation: data.rotation, wall_type: "wall_window" };
        let mut done = false;
        let mut y = 0;
        while y < third_grid.height && !done {
            let mut x = third_grid.width - 1;
            while x >= 0 && !done {
                if MansionGrid::is_house(third_grid, x, y) {
                    third_data.position = third_data.position.relative_n(rotation.rotate(Direction::South), 8 + (y - self.start_y) * 8);
                    third_data.position = third_data.position.relative_n(rotation.rotate(Direction::East), (x - self.start_x) * 8);
                    self.traverse_wall_piece(pieces, &mut third_data);
                    self.traverse_outer_walls(pieces, &mut third_data, third_grid, Direction::South, x, y, x, y);
                    done = true;
                }
                x -= 1;
            }
            y += 1;
        }
        self.create_roof(pieces, origin.offset(0, 16, 0), rotation, base_grid, Some(third_grid));
        self.create_roof(pieces, origin.offset(0, 27, 0), rotation, third_grid, None);
        let room_collections = [FloorRoomCollection::FirstFloor, FloorRoomCollection::SecondFloor, FloorRoomCollection::ThirdFloor];
        let mut door_dirs = Vec::new();
        for floor_num in 0..3 {
            let floor_origin = origin.offset(0, 8 * floor_num as i32 + if floor_num == 2 { 3 } else { 0 }, 0);
            let rooms = &mansion.floor_rooms[floor_num];
            let grid = if floor_num == 2 { third_grid } else { base_grid };
            let south_piece = if floor_num == 0 { "carpet_south_1" } else { "carpet_south_2" };
            let west_piece = if floor_num == 0 { "carpet_west_1" } else { "carpet_west_2" };
            for y in 0..grid.height {
                for x in 0..grid.width {
                    if grid.get(x, y) == CORRIDOR {
                        let mut pos = floor_origin.relative_n(rotation.rotate(Direction::South), 8 + (y - self.start_y) * 8);
                        pos = pos.relative_n(rotation.rotate(Direction::East), (x - self.start_x) * 8);
                        pieces.push(WoodlandMansionPiece::new("corridor_floor", pos, rotation));
                        if grid.get(x, y - 1) == CORRIDOR || (rooms.get(x, y - 1) & ROOM_CORRIDOR_FLAG) == ROOM_CORRIDOR_FLAG {
                            pieces.push(WoodlandMansionPiece::new("carpet_north", pos.relative_n(rotation.rotate(Direction::East), 1).above(), rotation));
                        }
                        if grid.get(x + 1, y) == CORRIDOR || (rooms.get(x + 1, y) & ROOM_CORRIDOR_FLAG) == ROOM_CORRIDOR_FLAG {
                            pieces.push(WoodlandMansionPiece::new(
                                "carpet_east",
                                pos.relative_n(rotation.rotate(Direction::South), 1).relative_n(rotation.rotate(Direction::East), 5).above(),
                                rotation,
                            ));
                        }
                        if grid.get(x, y + 1) == CORRIDOR || (rooms.get(x, y + 1) & ROOM_CORRIDOR_FLAG) == ROOM_CORRIDOR_FLAG {
                            pieces.push(WoodlandMansionPiece::new(
                                south_piece,
                                pos.relative_n(rotation.rotate(Direction::South), 5).relative_n(rotation.rotate(Direction::West), 1),
                                rotation,
                            ));
                        }
                        if grid.get(x - 1, y) == CORRIDOR || (rooms.get(x - 1, y) & ROOM_CORRIDOR_FLAG) == ROOM_CORRIDOR_FLAG {
                            pieces.push(WoodlandMansionPiece::new(
                                west_piece,
                                pos.relative_n(rotation.rotate(Direction::West), 1).relative_n(rotation.rotate(Direction::North), 1),
                                rotation,
                            ));
                        }
                    }
                }
            }
            let wall_piece = if floor_num == 0 { "indoors_wall_1" } else { "indoors_wall_2" };
            let door_piece = if floor_num == 0 { "indoors_door_1" } else { "indoors_door_2" };
            for y in 0..grid.height {
                for x in 0..grid.width {
                    let mut third_floor_start_room = floor_num == 2 && grid.get(x, y) == START_ROOM;
                    if grid.get(x, y) == ROOM || third_floor_start_room {
                        let room_data = rooms.get(x, y);
                        let room_type = room_data & ROOM_TYPE_MASK;
                        let room_id = room_data & ROOM_ID_MASK;
                        third_floor_start_room = third_floor_start_room && (room_data & ROOM_CORRIDOR_FLAG) == ROOM_CORRIDOR_FLAG;
                        door_dirs.clear();
                        if (room_data & ROOM_DOOR_FLAG) == ROOM_DOOR_FLAG {
                            for direction in Direction::PLANE_HORIZONTAL {
                                if grid.get(x + direction.step_x(), y + direction.step_z()) == CORRIDOR {
                                    door_dirs.push(direction);
                                }
                            }
                        }
                        let mut door_dir = None;
                        if !door_dirs.is_empty() {
                            door_dir = Some(door_dirs[self.random.next_int_bounded(door_dirs.len() as i32) as usize]);
                        } else if (room_data & ROOM_ORIGIN_FLAG) == ROOM_ORIGIN_FLAG {
                            door_dir = Some(Direction::Up);
                        }
                        let mut room_pos = floor_origin.relative_n(rotation.rotate(Direction::South), 8 + (y - self.start_y) * 8);
                        room_pos = room_pos.relative_n(rotation.rotate(Direction::East), -1 + (x - self.start_x) * 8);
                        if MansionGrid::is_house(grid, x - 1, y) && !mansion.is_room_id(x - 1, y, floor_num, room_id) {
                            pieces.push(WoodlandMansionPiece::new(if door_dir == Some(Direction::West) { door_piece } else { wall_piece }, room_pos, rotation));
                        }
                        if grid.get(x + 1, y) == CORRIDOR && !third_floor_start_room {
                            let pos = room_pos.relative_n(rotation.rotate(Direction::East), 8);
                            pieces.push(WoodlandMansionPiece::new(if door_dir == Some(Direction::East) { door_piece } else { wall_piece }, pos, rotation));
                        }
                        if MansionGrid::is_house(grid, x, y + 1) && !mansion.is_room_id(x, y + 1, floor_num, room_id) {
                            let mut pos = room_pos.relative_n(rotation.rotate(Direction::South), 7);
                            pos = pos.relative_n(rotation.rotate(Direction::East), 7);
                            pieces.push(WoodlandMansionPiece::new(
                                if door_dir == Some(Direction::South) { door_piece } else { wall_piece },
                                pos,
                                rotation.get_rotated(Rotation::Clockwise90),
                            ));
                        }
                        if grid.get(x, y - 1) == CORRIDOR && !third_floor_start_room {
                            let mut pos = room_pos.relative_n(rotation.rotate(Direction::North), 1);
                            pos = pos.relative_n(rotation.rotate(Direction::East), 7);
                            pieces.push(WoodlandMansionPiece::new(
                                if door_dir == Some(Direction::North) { door_piece } else { wall_piece },
                                pos,
                                rotation.get_rotated(Rotation::Clockwise90),
                            ));
                        }
                        if room_type == ROOM_1X1 {
                            self.add_room_1x1(pieces, room_pos, rotation, door_dir, room_collections[floor_num]);
                        } else if room_type == ROOM_1X2
                            && let Some(door_dir) = door_dir
                        {
                            let room_dir = mansion.get_1x2_room_direction(x, y, floor_num, room_id);
                            let is_stairs_room = (room_data & ROOM_STAIRS_FLAG) == ROOM_STAIRS_FLAG;
                            self.add_room_1x2(pieces, room_pos, rotation, room_dir, door_dir, room_collections[floor_num], is_stairs_room);
                        } else if room_type == ROOM_2X2
                            && let Some(door_dir) = door_dir
                            && door_dir != Direction::Up
                        {
                            let mut room_dir = door_dir.clockwise();
                            if !mansion.is_room_id(x + room_dir.step_x(), y + room_dir.step_z(), floor_num, room_id) {
                                room_dir = room_dir.opposite();
                            }
                            self.add_room_2x2(pieces, room_pos, rotation, room_dir, door_dir, room_collections[floor_num]);
                        } else if room_type == ROOM_2X2 && door_dir == Some(Direction::Up) {
                            self.add_room_2x2_secret(pieces, room_pos, rotation, room_collections[floor_num]);
                        }
                    }
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn traverse_outer_walls(
        &mut self,
        pieces: &mut Vec<WoodlandMansionPiece>,
        data: &mut PlacementData,
        grid: &SimpleGrid,
        mut grid_direction: Direction,
        start_x: i32,
        start_y: i32,
        end_x: i32,
        end_y: i32,
    ) {
        let mut grid_x = start_x;
        let mut grid_y = start_y;
        let start_direction = grid_direction;
        loop {
            if !MansionGrid::is_house(grid, grid_x + grid_direction.step_x(), grid_y + grid_direction.step_z()) {
                self.traverse_turn(pieces, data);
                grid_direction = grid_direction.clockwise();
                if grid_x != end_x || grid_y != end_y || start_direction != grid_direction {
                    self.traverse_wall_piece(pieces, data);
                }
            } else if MansionGrid::is_house(grid, grid_x + grid_direction.step_x(), grid_y + grid_direction.step_z())
                && MansionGrid::is_house(
                    grid,
                    grid_x + grid_direction.step_x() + grid_direction.counter_clockwise().step_x(),
                    grid_y + grid_direction.step_z() + grid_direction.counter_clockwise().step_z(),
                )
            {
                self.traverse_inner_turn(data);
                grid_x += grid_direction.step_x();
                grid_y += grid_direction.step_z();
                grid_direction = grid_direction.counter_clockwise();
            } else {
                grid_x += grid_direction.step_x();
                grid_y += grid_direction.step_z();
                if grid_x != end_x || grid_y != end_y || start_direction != grid_direction {
                    self.traverse_wall_piece(pieces, data);
                }
            }
            if !(grid_x != end_x || grid_y != end_y || start_direction != grid_direction) {
                break;
            }
        }
    }

    fn create_roof(&mut self, pieces: &mut Vec<WoodlandMansionPiece>, roof_origin: BlockPos, rotation: Rotation, grid: &SimpleGrid, above_grid: Option<&SimpleGrid>) {
        for y in 0..grid.height {
            for x in 0..grid.width {
                let mut position = roof_origin;
                position = position.relative_n(rotation.rotate(Direction::South), 8 + (y - self.start_y) * 8);
                position = position.relative_n(rotation.rotate(Direction::East), (x - self.start_x) * 8);
                let is_above = above_grid.is_some_and(|above_grid| MansionGrid::is_house(above_grid, x, y));
                if MansionGrid::is_house(grid, x, y) && !is_above {
                    pieces.push(WoodlandMansionPiece::new("roof", position.offset(0, 3, 0), rotation));
                    if !MansionGrid::is_house(grid, x + 1, y) {
                        let p2 = position.relative_n(rotation.rotate(Direction::East), 6);
                        pieces.push(WoodlandMansionPiece::new("roof_front", p2, rotation));
                    }
                    if !MansionGrid::is_house(grid, x - 1, y) {
                        let mut p2 = position.relative_n(rotation.rotate(Direction::East), 0);
                        p2 = p2.relative_n(rotation.rotate(Direction::South), 7);
                        pieces.push(WoodlandMansionPiece::new("roof_front", p2, rotation.get_rotated(Rotation::Clockwise180)));
                    }
                    if !MansionGrid::is_house(grid, x, y - 1) {
                        let p2 = position.relative_n(rotation.rotate(Direction::West), 1);
                        pieces.push(WoodlandMansionPiece::new("roof_front", p2, rotation.get_rotated(Rotation::Counterclockwise90)));
                    }
                    if !MansionGrid::is_house(grid, x, y + 1) {
                        let mut p2 = position.relative_n(rotation.rotate(Direction::East), 6);
                        p2 = p2.relative_n(rotation.rotate(Direction::South), 6);
                        pieces.push(WoodlandMansionPiece::new("roof_front", p2, rotation.get_rotated(Rotation::Clockwise90)));
                    }
                }
            }
        }
        if let Some(above_grid) = above_grid {
            for y in 0..grid.height {
                for x in 0..grid.width {
                    let mut position = roof_origin;
                    position = position.relative_n(rotation.rotate(Direction::South), 8 + (y - self.start_y) * 8);
                    position = position.relative_n(rotation.rotate(Direction::East), (x - self.start_x) * 8);
                    let is_above = MansionGrid::is_house(above_grid, x, y);
                    if MansionGrid::is_house(grid, x, y) && is_above {
                        if !MansionGrid::is_house(grid, x + 1, y) {
                            let p2 = position.relative_n(rotation.rotate(Direction::East), 7);
                            pieces.push(WoodlandMansionPiece::new("small_wall", p2, rotation));
                        }
                        if !MansionGrid::is_house(grid, x - 1, y) {
                            let mut p2 = position.relative_n(rotation.rotate(Direction::West), 1);
                            p2 = p2.relative_n(rotation.rotate(Direction::South), 6);
                            pieces.push(WoodlandMansionPiece::new("small_wall", p2, rotation.get_rotated(Rotation::Clockwise180)));
                        }
                        if !MansionGrid::is_house(grid, x, y - 1) {
                            let mut p2 = position.relative_n(rotation.rotate(Direction::West), 0);
                            p2 = p2.relative_n(rotation.rotate(Direction::North), 1);
                            pieces.push(WoodlandMansionPiece::new("small_wall", p2, rotation.get_rotated(Rotation::Counterclockwise90)));
                        }
                        if !MansionGrid::is_house(grid, x, y + 1) {
                            let mut p2 = position.relative_n(rotation.rotate(Direction::East), 6);
                            p2 = p2.relative_n(rotation.rotate(Direction::South), 7);
                            pieces.push(WoodlandMansionPiece::new("small_wall", p2, rotation.get_rotated(Rotation::Clockwise90)));
                        }
                        if !MansionGrid::is_house(grid, x + 1, y) {
                            if !MansionGrid::is_house(grid, x, y - 1) {
                                let mut p2 = position.relative_n(rotation.rotate(Direction::East), 7);
                                p2 = p2.relative_n(rotation.rotate(Direction::North), 2);
                                pieces.push(WoodlandMansionPiece::new("small_wall_corner", p2, rotation));
                            }
                            if !MansionGrid::is_house(grid, x, y + 1) {
                                let mut p2 = position.relative_n(rotation.rotate(Direction::East), 8);
                                p2 = p2.relative_n(rotation.rotate(Direction::South), 7);
                                pieces.push(WoodlandMansionPiece::new("small_wall_corner", p2, rotation.get_rotated(Rotation::Clockwise90)));
                            }
                        }
                        if !MansionGrid::is_house(grid, x - 1, y) {
                            if !MansionGrid::is_house(grid, x, y - 1) {
                                let mut p2 = position.relative_n(rotation.rotate(Direction::West), 2);
                                p2 = p2.relative_n(rotation.rotate(Direction::North), 1);
                                pieces.push(WoodlandMansionPiece::new("small_wall_corner", p2, rotation.get_rotated(Rotation::Counterclockwise90)));
                            }
                            if !MansionGrid::is_house(grid, x, y + 1) {
                                let mut p2 = position.relative_n(rotation.rotate(Direction::West), 1);
                                p2 = p2.relative_n(rotation.rotate(Direction::South), 8);
                                pieces.push(WoodlandMansionPiece::new("small_wall_corner", p2, rotation.get_rotated(Rotation::Clockwise180)));
                            }
                        }
                    }
                }
            }
        }
        for y in 0..grid.height {
            for x in 0..grid.width {
                let mut position = roof_origin;
                position = position.relative_n(rotation.rotate(Direction::South), 8 + (y - self.start_y) * 8);
                position = position.relative_n(rotation.rotate(Direction::East), (x - self.start_x) * 8);
                let is_above = above_grid.is_some_and(|above_grid| MansionGrid::is_house(above_grid, x, y));
                if MansionGrid::is_house(grid, x, y) && !is_above {
                    if !MansionGrid::is_house(grid, x + 1, y) {
                        let p2 = position.relative_n(rotation.rotate(Direction::East), 6);
                        if !MansionGrid::is_house(grid, x, y + 1) {
                            let p3 = p2.relative_n(rotation.rotate(Direction::South), 6);
                            pieces.push(WoodlandMansionPiece::new("roof_corner", p3, rotation));
                        } else if MansionGrid::is_house(grid, x + 1, y + 1) {
                            let p3 = p2.relative_n(rotation.rotate(Direction::South), 5);
                            pieces.push(WoodlandMansionPiece::new("roof_inner_corner", p3, rotation));
                        }
                        if !MansionGrid::is_house(grid, x, y - 1) {
                            pieces.push(WoodlandMansionPiece::new("roof_corner", p2, rotation.get_rotated(Rotation::Counterclockwise90)));
                        } else if MansionGrid::is_house(grid, x + 1, y - 1) {
                            let mut p3 = position.relative_n(rotation.rotate(Direction::East), 9);
                            p3 = p3.relative_n(rotation.rotate(Direction::North), 2);
                            pieces.push(WoodlandMansionPiece::new("roof_inner_corner", p3, rotation.get_rotated(Rotation::Clockwise90)));
                        }
                    }
                    if !MansionGrid::is_house(grid, x - 1, y) {
                        let mut p2 = position.relative_n(rotation.rotate(Direction::East), 0);
                        p2 = p2.relative_n(rotation.rotate(Direction::South), 0);
                        if !MansionGrid::is_house(grid, x, y + 1) {
                            let p3 = p2.relative_n(rotation.rotate(Direction::South), 6);
                            pieces.push(WoodlandMansionPiece::new("roof_corner", p3, rotation.get_rotated(Rotation::Clockwise90)));
                        } else if MansionGrid::is_house(grid, x - 1, y + 1) {
                            let mut p3 = p2.relative_n(rotation.rotate(Direction::South), 8);
                            p3 = p3.relative_n(rotation.rotate(Direction::West), 3);
                            pieces.push(WoodlandMansionPiece::new("roof_inner_corner", p3, rotation.get_rotated(Rotation::Counterclockwise90)));
                        }
                        if !MansionGrid::is_house(grid, x, y - 1) {
                            pieces.push(WoodlandMansionPiece::new("roof_corner", p2, rotation.get_rotated(Rotation::Clockwise180)));
                        } else if MansionGrid::is_house(grid, x - 1, y - 1) {
                            let p3 = p2.relative_n(rotation.rotate(Direction::South), 1);
                            pieces.push(WoodlandMansionPiece::new("roof_inner_corner", p3, rotation.get_rotated(Rotation::Clockwise180)));
                        }
                    }
                }
            }
        }
    }

    fn entrance(&mut self, pieces: &mut Vec<WoodlandMansionPiece>, data: &mut PlacementData) {
        let west = data.rotation.rotate(Direction::West);
        pieces.push(WoodlandMansionPiece::new("entrance", data.position.relative_n(west, 9), data.rotation));
        data.position = data.position.relative_n(data.rotation.rotate(Direction::South), 16);
    }

    fn traverse_wall_piece(&mut self, pieces: &mut Vec<WoodlandMansionPiece>, data: &mut PlacementData) {
        pieces.push(WoodlandMansionPiece::new(data.wall_type, data.position.relative_n(data.rotation.rotate(Direction::East), 7), data.rotation));
        data.position = data.position.relative_n(data.rotation.rotate(Direction::South), 8);
    }

    fn traverse_turn(&mut self, pieces: &mut Vec<WoodlandMansionPiece>, data: &mut PlacementData) {
        data.position = data.position.relative_n(data.rotation.rotate(Direction::South), -1);
        pieces.push(WoodlandMansionPiece::new("wall_corner", data.position, data.rotation));
        data.position = data.position.relative_n(data.rotation.rotate(Direction::South), -7);
        data.position = data.position.relative_n(data.rotation.rotate(Direction::West), -6);
        data.rotation = data.rotation.get_rotated(Rotation::Clockwise90);
    }

    fn traverse_inner_turn(&mut self, data: &mut PlacementData) {
        data.position = data.position.relative_n(data.rotation.rotate(Direction::South), 6);
        data.position = data.position.relative_n(data.rotation.rotate(Direction::East), 8);
        data.rotation = data.rotation.get_rotated(Rotation::Counterclockwise90);
    }

    fn add_room_1x1(&mut self, pieces: &mut Vec<WoodlandMansionPiece>, room_pos: BlockPos, rotation: Rotation, door_dir: Option<Direction>, rooms: FloorRoomCollection) {
        let mut piece_rot = Rotation::None;
        let mut room_type = rooms.get_1x1(self.random);
        if door_dir != Some(Direction::East) {
            if door_dir == Some(Direction::North) {
                piece_rot = piece_rot.get_rotated(Rotation::Counterclockwise90);
            } else if door_dir == Some(Direction::West) {
                piece_rot = piece_rot.get_rotated(Rotation::Clockwise180);
            } else if door_dir == Some(Direction::South) {
                piece_rot = piece_rot.get_rotated(Rotation::Clockwise90);
            } else {
                room_type = rooms.get_1x1_secret(self.random);
            }
        }
        let mut orientation = get_zero_position_with_transform(BlockPos::new(1, 0, 0), Mirror::None, piece_rot, 7, 7);
        piece_rot = piece_rot.get_rotated(rotation);
        orientation = rotate(orientation, rotation);
        let pos = room_pos.offset(orientation.x, 0, orientation.z);
        pieces.push(WoodlandMansionPiece::with_mirror(room_type, pos, piece_rot, Mirror::None));
    }

    #[allow(clippy::too_many_arguments)]
    fn add_room_1x2(
        &mut self,
        pieces: &mut Vec<WoodlandMansionPiece>,
        room_pos: BlockPos,
        rotation: Rotation,
        room_dir: Option<Direction>,
        door_dir: Direction,
        rooms: FloorRoomCollection,
        is_stairs_room: bool,
    ) {
        let random = &mut *self.random;
        match (door_dir, room_dir) {
            (Direction::East, Some(Direction::South)) => {
                let pos = room_pos.relative_n(rotation.rotate(Direction::East), 1);
                pieces.push(WoodlandMansionPiece::with_mirror(rooms.get_1x2_side_entrance(random, is_stairs_room), pos, rotation, Mirror::None));
            }
            (Direction::East, Some(Direction::North)) => {
                let mut pos = room_pos.relative_n(rotation.rotate(Direction::East), 1);
                pos = pos.relative_n(rotation.rotate(Direction::South), 6);
                pieces.push(WoodlandMansionPiece::with_mirror(rooms.get_1x2_side_entrance(random, is_stairs_room), pos, rotation, Mirror::LeftRight));
            }
            (Direction::West, Some(Direction::North)) => {
                let mut pos = room_pos.relative_n(rotation.rotate(Direction::East), 7);
                pos = pos.relative_n(rotation.rotate(Direction::South), 6);
                pieces.push(WoodlandMansionPiece::with_mirror(
                    rooms.get_1x2_side_entrance(random, is_stairs_room),
                    pos,
                    rotation.get_rotated(Rotation::Clockwise180),
                    Mirror::None,
                ));
            }
            (Direction::West, Some(Direction::South)) => {
                let pos = room_pos.relative_n(rotation.rotate(Direction::East), 7);
                pieces.push(WoodlandMansionPiece::with_mirror(rooms.get_1x2_side_entrance(random, is_stairs_room), pos, rotation, Mirror::FrontBack));
            }
            (Direction::South, Some(Direction::East)) => {
                let pos = room_pos.relative_n(rotation.rotate(Direction::East), 1);
                pieces.push(WoodlandMansionPiece::with_mirror(
                    rooms.get_1x2_side_entrance(random, is_stairs_room),
                    pos,
                    rotation.get_rotated(Rotation::Clockwise90),
                    Mirror::LeftRight,
                ));
            }
            (Direction::South, Some(Direction::West)) => {
                let pos = room_pos.relative_n(rotation.rotate(Direction::East), 7);
                pieces.push(WoodlandMansionPiece::with_mirror(
                    rooms.get_1x2_side_entrance(random, is_stairs_room),
                    pos,
                    rotation.get_rotated(Rotation::Clockwise90),
                    Mirror::None,
                ));
            }
            (Direction::North, Some(Direction::West)) => {
                let mut pos = room_pos.relative_n(rotation.rotate(Direction::East), 7);
                pos = pos.relative_n(rotation.rotate(Direction::South), 6);
                pieces.push(WoodlandMansionPiece::with_mirror(
                    rooms.get_1x2_side_entrance(random, is_stairs_room),
                    pos,
                    rotation.get_rotated(Rotation::Clockwise90),
                    Mirror::FrontBack,
                ));
            }
            (Direction::North, Some(Direction::East)) => {
                let mut pos = room_pos.relative_n(rotation.rotate(Direction::East), 1);
                pos = pos.relative_n(rotation.rotate(Direction::South), 6);
                pieces.push(WoodlandMansionPiece::with_mirror(
                    rooms.get_1x2_side_entrance(random, is_stairs_room),
                    pos,
                    rotation.get_rotated(Rotation::Counterclockwise90),
                    Mirror::None,
                ));
            }
            (Direction::South, Some(Direction::North)) => {
                let mut pos = room_pos.relative_n(rotation.rotate(Direction::East), 1);
                pos = pos.relative_n(rotation.rotate(Direction::North), 8);
                pieces.push(WoodlandMansionPiece::with_mirror(rooms.get_1x2_front_entrance(random, is_stairs_room), pos, rotation, Mirror::None));
            }
            (Direction::North, Some(Direction::South)) => {
                let mut pos = room_pos.relative_n(rotation.rotate(Direction::East), 7);
                pos = pos.relative_n(rotation.rotate(Direction::South), 14);
                pieces.push(WoodlandMansionPiece::with_mirror(
                    rooms.get_1x2_front_entrance(random, is_stairs_room),
                    pos,
                    rotation.get_rotated(Rotation::Clockwise180),
                    Mirror::None,
                ));
            }
            (Direction::West, Some(Direction::East)) => {
                let pos = room_pos.relative_n(rotation.rotate(Direction::East), 15);
                pieces.push(WoodlandMansionPiece::with_mirror(
                    rooms.get_1x2_front_entrance(random, is_stairs_room),
                    pos,
                    rotation.get_rotated(Rotation::Clockwise90),
                    Mirror::None,
                ));
            }
            (Direction::East, Some(Direction::West)) => {
                let mut pos = room_pos.relative_n(rotation.rotate(Direction::West), 7);
                pos = pos.relative_n(rotation.rotate(Direction::South), 6);
                pieces.push(WoodlandMansionPiece::with_mirror(
                    rooms.get_1x2_front_entrance(random, is_stairs_room),
                    pos,
                    rotation.get_rotated(Rotation::Counterclockwise90),
                    Mirror::None,
                ));
            }
            (Direction::Up, Some(Direction::East)) => {
                let pos = room_pos.relative_n(rotation.rotate(Direction::East), 15);
                pieces.push(WoodlandMansionPiece::with_mirror(rooms.get_1x2_secret(random), pos, rotation.get_rotated(Rotation::Clockwise90), Mirror::None));
            }
            (Direction::Up, Some(Direction::South)) => {
                let mut pos = room_pos.relative_n(rotation.rotate(Direction::East), 1);
                pos = pos.relative_n(rotation.rotate(Direction::North), 0);
                pieces.push(WoodlandMansionPiece::with_mirror(rooms.get_1x2_secret(random), pos, rotation, Mirror::None));
            }
            _ => {}
        }
    }

    fn add_room_2x2(
        &mut self,
        pieces: &mut Vec<WoodlandMansionPiece>,
        room_pos: BlockPos,
        rotation: Rotation,
        room_dir: Direction,
        door_dir: Direction,
        rooms: FloorRoomCollection,
    ) {
        let mut east = 0;
        let mut south = 0;
        let mut rot = rotation;
        let mut mirror = Mirror::None;
        match (door_dir, room_dir) {
            (Direction::East, Direction::South) => east = -7,
            (Direction::East, Direction::North) => {
                east = -7;
                south = 6;
                mirror = Mirror::LeftRight;
            }
            (Direction::North, Direction::East) => {
                east = 1;
                south = 14;
                rot = rotation.get_rotated(Rotation::Counterclockwise90);
            }
            (Direction::North, Direction::West) => {
                east = 7;
                south = 14;
                rot = rotation.get_rotated(Rotation::Counterclockwise90);
                mirror = Mirror::LeftRight;
            }
            (Direction::South, Direction::West) => {
                east = 7;
                south = -8;
                rot = rotation.get_rotated(Rotation::Clockwise90);
            }
            (Direction::South, Direction::East) => {
                east = 1;
                south = -8;
                rot = rotation.get_rotated(Rotation::Clockwise90);
                mirror = Mirror::LeftRight;
            }
            (Direction::West, Direction::North) => {
                east = 15;
                south = 6;
                rot = rotation.get_rotated(Rotation::Clockwise180);
            }
            (Direction::West, Direction::South) => {
                east = 15;
                mirror = Mirror::FrontBack;
            }
            _ => {}
        }
        let mut pos = room_pos.relative_n(rotation.rotate(Direction::East), east);
        pos = pos.relative_n(rotation.rotate(Direction::South), south);
        pieces.push(WoodlandMansionPiece::with_mirror(rooms.get_2x2(self.random), pos, rot, mirror));
    }

    fn add_room_2x2_secret(&mut self, pieces: &mut Vec<WoodlandMansionPiece>, room_pos: BlockPos, rotation: Rotation, rooms: FloorRoomCollection) {
        let pos = room_pos.relative_n(rotation.rotate(Direction::East), 1);
        pieces.push(WoodlandMansionPiece::with_mirror(rooms.get_2x2_secret(), pos, rotation, Mirror::None));
    }
}

struct SimpleGrid {
    grid: [[i32; DEFAULT_SIZE]; DEFAULT_SIZE],
    width: i32,
    height: i32,
    value_if_outside: i32,
}

impl SimpleGrid {
    fn new(value_if_outside: i32) -> SimpleGrid {
        SimpleGrid { grid: [[0; DEFAULT_SIZE]; DEFAULT_SIZE], width: DEFAULT_SIZE as i32, height: DEFAULT_SIZE as i32, value_if_outside }
    }

    fn set(&mut self, x: i32, y: i32, value: i32) {
        if x >= 0 && x < self.width && y >= 0 && y < self.height {
            self.grid[x as usize][y as usize] = value;
        }
    }

    fn set_area(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, value: i32) {
        for y in y0..=y1 {
            for x in x0..=x1 {
                self.set(x, y, value);
            }
        }
    }

    fn get(&self, x: i32, y: i32) -> i32 {
        if x >= 0 && x < self.width && y >= 0 && y < self.height { self.grid[x as usize][y as usize] } else { self.value_if_outside }
    }

    fn setif(&mut self, x: i32, y: i32, if_value: i32, value: i32) {
        if self.get(x, y) == if_value {
            self.set(x, y, value);
        }
    }

    fn edges_to(&self, x: i32, y: i32, if_value: i32) -> bool {
        self.get(x - 1, y) == if_value || self.get(x + 1, y) == if_value || self.get(x, y + 1) == if_value || self.get(x, y - 1) == if_value
    }
}

#[derive(Clone)]
pub struct WoodlandMansionPiece {
    pub template: TemplateStructurePiece,
}

fn make_location(template_name: &str) -> String {
    format!("minecraft:woodland_mansion/{template_name}")
}

fn make_settings(mirror: Mirror, rotation: Rotation) -> TemplatePlaceSettings {
    TemplatePlaceSettings { mirror, rotation, rotation_pivot: BlockPos::ZERO, apply_waterlogging: true, processors: &*STRUCTURE_BLOCK }
}

impl WoodlandMansionPiece {
    fn new(template_name: &str, position: BlockPos, rotation: Rotation) -> WoodlandMansionPiece {
        WoodlandMansionPiece::with_mirror(template_name.to_owned(), position, rotation, Mirror::None)
    }

    fn with_mirror(template_name: String, position: BlockPos, rotation: Rotation, mirror: Mirror) -> WoodlandMansionPiece {
        let template_location = make_location(&template_name);
        WoodlandMansionPiece {
            template: TemplateStructurePiece::new(WOODLAND_MANSION_PIECE, 0, &template_location, template_name, make_settings(mirror, rotation), position),
        }
    }

    pub fn load(tag: &Nbt) -> Option<WoodlandMansionPiece> {
        let mirror = tag.get("Mi").and_then(Nbt::as_str).and_then(Mirror::by_legacy_name)?;
        let rotation = tag.get("Rot").and_then(Nbt::as_str).and_then(Rotation::by_legacy_name)?;
        Some(WoodlandMansionPiece {
            template: TemplateStructurePiece::load_with_template_location(WOODLAND_MANSION_PIECE, tag, make_location, |_| make_settings(mirror, rotation))?,
        })
    }

    pub fn add_additional_save_data(&self, tag: &mut Vec<(String, Nbt)>) {
        self.template.add_additional_save_data(tag);
        tag.push(("Rot".into(), Nbt::String(self.template.place_settings.rotation.legacy_name().to_owned())));
        tag.push(("Mi".into(), Nbt::String(self.template.place_settings.mirror.legacy_name().to_owned())));
    }

    pub fn post_process(&mut self, level: &mut WorldGenRegion, random: &mut impl RandomSource, chunk_bb: &BoundingBox, reference_pos: BlockPos) {
        let piece = self.template.piece.clone();
        let rot = self.template.place_settings.rotation;
        self.template.post_process(level, random, chunk_bb, reference_pos, |marker_id, position, level, random, chunk_bb| {
            WoodlandMansionPiece::handle_data_marker(&piece, rot, marker_id, position, level, random, chunk_bb);
        });
    }

    fn handle_data_marker(
        piece: &StructurePiece,
        rot: Rotation,
        marker_id: &str,
        position: BlockPos,
        level: &mut WorldGenRegion,
        random: &mut impl RandomSource,
        chunk_bb: &BoundingBox,
    ) {
        if marker_id.starts_with("Chest") {
            let mut chest_state = blocks::CHEST.default_state();
            match marker_id {
                "ChestWest" => chest_state = chest_state.set(HORIZONTAL_FACING, rot.rotate(Direction::West)),
                "ChestEast" => chest_state = chest_state.set(HORIZONTAL_FACING, rot.rotate(Direction::East)),
                "ChestSouth" => chest_state = chest_state.set(HORIZONTAL_FACING, rot.rotate(Direction::South)),
                "ChestNorth" => chest_state = chest_state.set(HORIZONTAL_FACING, rot.rotate(Direction::North)),
                _ => {}
            }
            piece.create_chest(level, chunk_bb, random, position, WOODLAND_MANSION, Some(chest_state));
        } else {
            let entity_type = match marker_id {
                "Mage" => &entity_types::EVOKER,
                "Warrior" => &entity_types::VINDICATOR,
                "Group of Allays" => {
                    LevelAccessor::get_random(level).next_int_bounded(3);
                    &entity_types::ALLAY
                }
                _ => return,
            };
            if entity_type.can_spawn(level.get_difficulty()) {
                level.set_block(position, blocks::AIR.default_state(), 2);
            }
        }
    }
}
