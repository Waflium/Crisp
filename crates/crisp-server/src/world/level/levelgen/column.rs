use crisp_core::{BlockPos, Direction};
use crisp_data::BlockState;

use crate::server::level::world_gen_region::WorldGenRegion;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Column {
    Line,
    Range { floor: i32, ceiling: i32 },
    Ray { edge: i32, pointing_up: bool },
}

impl Column {
    pub fn inside(floor: i32, ceiling: i32) -> Column {
        let column = Column::Range { floor, ceiling };
        assert!(ceiling - floor - 1 >= 0, "Column of negative height: C({ceiling}-{floor})");
        column
    }

    pub fn below(ceiling: i32) -> Column {
        Column::Ray { edge: ceiling, pointing_up: false }
    }

    pub fn above(floor: i32) -> Column {
        Column::Ray { edge: floor, pointing_up: true }
    }

    pub fn create(floor: Option<i32>, ceiling: Option<i32>) -> Column {
        match (floor, ceiling) {
            (Some(floor), Some(ceiling)) => Column::inside(floor, ceiling),
            (Some(floor), None) => Column::above(floor),
            (None, Some(ceiling)) => Column::below(ceiling),
            (None, None) => Column::Line,
        }
    }

    pub fn get_ceiling(&self) -> Option<i32> {
        match *self {
            Column::Line => None,
            Column::Range { ceiling, .. } => Some(ceiling),
            Column::Ray { edge, pointing_up } => (!pointing_up).then_some(edge),
        }
    }

    pub fn get_floor(&self) -> Option<i32> {
        match *self {
            Column::Line => None,
            Column::Range { floor, .. } => Some(floor),
            Column::Ray { edge, pointing_up } => pointing_up.then_some(edge),
        }
    }

    pub fn get_height(&self) -> Option<i32> {
        match *self {
            Column::Range { floor, ceiling } => Some(ceiling - floor - 1),
            _ => None,
        }
    }

    pub fn with_floor(&self, floor: Option<i32>) -> Column {
        Column::create(floor, self.get_ceiling())
    }

    pub fn scan(level: &mut WorldGenRegion, pos: BlockPos, search_range: i32, inside_column: impl Fn(BlockState) -> bool, valid_edge: impl Fn(BlockState) -> bool) -> Option<Column> {
        if !inside_column(level.get_block_state(pos)) {
            return None;
        }
        let nearest_empty_y = pos.y;
        let ceiling = scan_direction(level, search_range, &inside_column, &valid_edge, pos, nearest_empty_y, Direction::Up);
        let floor = scan_direction(level, search_range, &inside_column, &valid_edge, pos, nearest_empty_y, Direction::Down);
        Some(Column::create(floor, ceiling))
    }
}

fn scan_direction(
    level: &mut WorldGenRegion,
    search_range: i32,
    inside_column: &impl Fn(BlockState) -> bool,
    valid_edge: &impl Fn(BlockState) -> bool,
    pos: BlockPos,
    nearest_empty_y: i32,
    direction: Direction,
) -> Option<i32> {
    let mut mutable_pos = BlockPos::new(pos.x, nearest_empty_y, pos.z);
    let mut i = 1;
    while i < search_range && inside_column(level.get_block_state(mutable_pos)) {
        mutable_pos = mutable_pos.relative(direction);
        i += 1;
    }
    valid_edge(level.get_block_state(mutable_pos)).then_some(mutable_pos.y)
}
