use crisp_core::BlockPos;
use crisp_data::BlockState;
use crisp_data::properties::RailShape;

use super::base_rail_block::{is_rail, is_rail_at, is_straight, shape_property};
use crate::server::level::ServerLevel;
use crate::world::level::BlockGetter;

pub struct RailState {
    pos: BlockPos,
    state: BlockState,
    is_straight: bool,
    connections: Vec<BlockPos>,
}

impl RailState {
    pub fn new(pos: BlockPos, state: BlockState) -> RailState {
        let block = state.block();
        let direction = state.get(shape_property(block));
        let mut rail = RailState { pos, state, is_straight: is_straight(block), connections: Vec::with_capacity(4) };
        rail.update_connections(direction);
        rail
    }

    pub fn connections(&self) -> &[BlockPos] {
        &self.connections
    }

    pub fn state(&self) -> BlockState {
        self.state
    }

    fn update_connections(&mut self, direction: RailShape) {
        let pos = self.pos;
        let ends = match direction {
            RailShape::NorthSouth => [pos.north(), pos.south()],
            RailShape::EastWest => [pos.west(), pos.east()],
            RailShape::AscendingEast => [pos.west(), pos.east().above()],
            RailShape::AscendingWest => [pos.west().above(), pos.east()],
            RailShape::AscendingNorth => [pos.north().above(), pos.south()],
            RailShape::AscendingSouth => [pos.north(), pos.south().above()],
            RailShape::SouthEast => [pos.east(), pos.south()],
            RailShape::SouthWest => [pos.west(), pos.south()],
            RailShape::NorthWest => [pos.west(), pos.north()],
            RailShape::NorthEast => [pos.east(), pos.north()],
        };
        self.connections.clear();
        self.connections.extend(ends);
    }

    fn remove_soft_connections(&mut self, level: &mut ServerLevel) {
        let mut i = 0;
        while i < self.connections.len() {
            match get_rail(level, self.connections[i]) {
                Some(rail) if rail.has_connection(self.pos) => {
                    self.connections[i] = rail.pos;
                    i += 1;
                }
                _ => {
                    self.connections.remove(i);
                }
            }
        }
    }

    fn has_connection(&self, rail_pos: BlockPos) -> bool {
        self.connections.iter().any(|pos| pos.x == rail_pos.x && pos.z == rail_pos.z)
    }

    pub fn count_potential_connections(&self, level: &mut ServerLevel) -> i32 {
        let mut count = 0;
        for direction in crisp_core::Direction::PLANE_HORIZONTAL {
            if has_rail(level, self.pos.relative(direction)) {
                count += 1;
            }
        }
        count
    }

    fn can_connect_to(&self, rail_pos: BlockPos) -> bool {
        self.has_connection(rail_pos) || self.connections.len() != 2
    }

    fn connect_to(&mut self, level: &mut ServerLevel, rail_pos: BlockPos) {
        self.connections.push(rail_pos);
        let north = self.pos.north();
        let south = self.pos.south();
        let west = self.pos.west();
        let east = self.pos.east();
        let n = self.has_connection(north);
        let s = self.has_connection(south);
        let w = self.has_connection(west);
        let e = self.has_connection(east);
        let mut shape = None;
        if n || s {
            shape = Some(RailShape::NorthSouth);
        }
        if w || e {
            shape = Some(RailShape::EastWest);
        }
        if !self.is_straight {
            if s && e && !n && !w {
                shape = Some(RailShape::SouthEast);
            }
            if s && w && !n && !e {
                shape = Some(RailShape::SouthWest);
            }
            if n && w && !s && !e {
                shape = Some(RailShape::NorthWest);
            }
            if n && e && !s && !w {
                shape = Some(RailShape::NorthEast);
            }
        }
        if shape == Some(RailShape::NorthSouth) {
            if is_rail_at(level, north.above()) {
                shape = Some(RailShape::AscendingNorth);
            }
            if is_rail_at(level, south.above()) {
                shape = Some(RailShape::AscendingSouth);
            }
        }
        if shape == Some(RailShape::EastWest) {
            if is_rail_at(level, east.above()) {
                shape = Some(RailShape::AscendingEast);
            }
            if is_rail_at(level, west.above()) {
                shape = Some(RailShape::AscendingWest);
            }
        }
        let shape = shape.unwrap_or(RailShape::NorthSouth);
        self.state = self.state.set(shape_property(self.state.block()), shape);
        level.set_block_simple(self.pos, self.state);
    }

    fn has_neighbor_rail(&self, level: &mut ServerLevel, rail_pos: BlockPos) -> bool {
        let Some(mut neighbor) = get_rail(level, rail_pos) else { return false };
        neighbor.remove_soft_connections(level);
        neighbor.can_connect_to(self.pos)
    }

    pub fn place(&mut self, level: &mut ServerLevel, has_signal: bool, first: bool, default_shape: RailShape) -> &mut RailState {
        let north = self.pos.north();
        let south = self.pos.south();
        let west = self.pos.west();
        let east = self.pos.east();
        let n = self.has_neighbor_rail(level, north);
        let s = self.has_neighbor_rail(level, south);
        let w = self.has_neighbor_rail(level, west);
        let e = self.has_neighbor_rail(level, east);
        let mut shape = None;
        let north_or_south = n || s;
        let west_or_east = w || e;
        if north_or_south && !west_or_east {
            shape = Some(RailShape::NorthSouth);
        }
        if west_or_east && !north_or_south {
            shape = Some(RailShape::EastWest);
        }
        let south_and_east = s && e;
        let south_and_west = s && w;
        let north_and_east = n && e;
        let north_and_west = n && w;
        if !self.is_straight {
            if south_and_east && !n && !w {
                shape = Some(RailShape::SouthEast);
            }
            if south_and_west && !n && !e {
                shape = Some(RailShape::SouthWest);
            }
            if north_and_west && !s && !e {
                shape = Some(RailShape::NorthWest);
            }
            if north_and_east && !s && !w {
                shape = Some(RailShape::NorthEast);
            }
        }
        if shape.is_none() {
            if north_or_south && west_or_east {
                shape = Some(default_shape);
            } else if north_or_south {
                shape = Some(RailShape::NorthSouth);
            } else if west_or_east {
                shape = Some(RailShape::EastWest);
            }
            if !self.is_straight {
                if has_signal {
                    if south_and_east {
                        shape = Some(RailShape::SouthEast);
                    }
                    if south_and_west {
                        shape = Some(RailShape::SouthWest);
                    }
                    if north_and_east {
                        shape = Some(RailShape::NorthEast);
                    }
                    if north_and_west {
                        shape = Some(RailShape::NorthWest);
                    }
                } else {
                    if north_and_west {
                        shape = Some(RailShape::NorthWest);
                    }
                    if north_and_east {
                        shape = Some(RailShape::NorthEast);
                    }
                    if south_and_west {
                        shape = Some(RailShape::SouthWest);
                    }
                    if south_and_east {
                        shape = Some(RailShape::SouthEast);
                    }
                }
            }
        }
        if shape == Some(RailShape::NorthSouth) {
            if is_rail_at(level, north.above()) {
                shape = Some(RailShape::AscendingNorth);
            }
            if is_rail_at(level, south.above()) {
                shape = Some(RailShape::AscendingSouth);
            }
        }
        if shape == Some(RailShape::EastWest) {
            if is_rail_at(level, east.above()) {
                shape = Some(RailShape::AscendingEast);
            }
            if is_rail_at(level, west.above()) {
                shape = Some(RailShape::AscendingWest);
            }
        }
        let shape = shape.unwrap_or(default_shape);
        self.update_connections(shape);
        self.state = self.state.set(shape_property(self.state.block()), shape);
        if first || level.block_state(self.pos) != self.state {
            level.set_block_simple(self.pos, self.state);
            for i in 0..self.connections.len() {
                if let Some(mut neighbor) = get_rail(level, self.connections[i]) {
                    neighbor.remove_soft_connections(level);
                    if neighbor.can_connect_to(self.pos) {
                        neighbor.connect_to(level, self.pos);
                    }
                }
            }
        }
        self
    }
}

fn has_rail(level: &mut ServerLevel, pos: BlockPos) -> bool {
    is_rail_at(level, pos) || is_rail_at(level, pos.above()) || is_rail_at(level, pos.below())
}

fn get_rail(level: &mut ServerLevel, pos: BlockPos) -> Option<RailState> {
    for test_pos in [pos, pos.above(), pos.below()] {
        let test_state = level.block_state(test_pos);
        if is_rail(test_state) {
            return Some(RailState::new(test_pos, test_state));
        }
    }
    None
}
