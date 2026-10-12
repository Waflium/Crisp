use crate::mth;
use crate::{Axis, Direction, Vec3};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default, PartialOrd, Ord)]
pub struct BlockPos {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

const PACKED_HORIZONTAL_LENGTH: u32 = 26;
const PACKED_Y_LENGTH: u32 = 64 - 2 * PACKED_HORIZONTAL_LENGTH;
const PACKED_X_MASK: i64 = (1 << PACKED_HORIZONTAL_LENGTH) - 1;
const PACKED_Y_MASK: i64 = (1 << PACKED_Y_LENGTH) - 1;
const Z_OFFSET: u32 = PACKED_Y_LENGTH;
const X_OFFSET: u32 = PACKED_Y_LENGTH + PACKED_HORIZONTAL_LENGTH;

impl BlockPos {
    pub const ZERO: BlockPos = BlockPos::new(0, 0, 0);

    pub const fn new(x: i32, y: i32, z: i32) -> BlockPos {
        BlockPos { x, y, z }
    }

    pub fn containing(x: f64, y: f64, z: f64) -> BlockPos {
        BlockPos::new(mth::floor(x), mth::floor(y), mth::floor(z))
    }

    pub fn containing_vec(v: Vec3) -> BlockPos {
        BlockPos::containing(v.x, v.y, v.z)
    }

    pub const fn offset(self, x: i32, y: i32, z: i32) -> BlockPos {
        BlockPos::new(self.x.wrapping_add(x), self.y.wrapping_add(y), self.z.wrapping_add(z))
    }

    pub const fn relative(self, d: Direction) -> BlockPos {
        self.offset(d.step_x(), d.step_y(), d.step_z())
    }

    pub const fn relative_n(self, d: Direction, n: i32) -> BlockPos {
        self.offset(d.step_x() * n, d.step_y() * n, d.step_z() * n)
    }

    pub fn relative_axis(self, axis: Axis, n: i32) -> BlockPos {
        match axis {
            Axis::X => self.offset(n, 0, 0),
            Axis::Y => self.offset(0, n, 0),
            Axis::Z => self.offset(0, 0, n),
        }
    }

    pub const fn above(self) -> BlockPos {
        self.relative(Direction::Up)
    }

    pub const fn below(self) -> BlockPos {
        self.relative(Direction::Down)
    }

    pub const fn north(self) -> BlockPos {
        self.relative(Direction::North)
    }

    pub const fn south(self) -> BlockPos {
        self.relative(Direction::South)
    }

    pub const fn west(self) -> BlockPos {
        self.relative(Direction::West)
    }

    pub const fn east(self) -> BlockPos {
        self.relative(Direction::East)
    }

    pub fn get(self, axis: Axis) -> i32 {
        axis.choose(self.x, self.y, self.z)
    }

    pub fn dist_sqr(self, o: BlockPos) -> f64 {
        let (dx, dy, dz) = ((self.x - o.x) as f64, (self.y - o.y) as f64, (self.z - o.z) as f64);
        dx * dx + dy * dy + dz * dz
    }

    pub fn dist_to_center_sqr(self, x: f64, y: f64, z: f64) -> f64 {
        let dx = self.x as f64 + 0.5 - x;
        let dy = self.y as f64 + 0.5 - y;
        let dz = self.z as f64 + 0.5 - z;
        dx * dx + dy * dy + dz * dz
    }

    pub fn dist_manhattan(self, o: BlockPos) -> i32 {
        (self.x - o.x).abs() + (self.y - o.y).abs() + (self.z - o.z).abs()
    }

    pub fn closer_than(self, o: BlockPos, distance: f64) -> bool {
        self.dist_sqr(o) < mth::square(distance)
    }

    pub const fn as_long(self) -> i64 {
        ((self.x as i64 & PACKED_X_MASK) << X_OFFSET) | (self.y as i64 & PACKED_Y_MASK) | ((self.z as i64 & PACKED_X_MASK) << Z_OFFSET)
    }

    pub const fn of_long(node: i64) -> BlockPos {
        BlockPos::new(
            (node << (64 - X_OFFSET - PACKED_HORIZONTAL_LENGTH) >> (64 - PACKED_HORIZONTAL_LENGTH)) as i32,
            (node << (64 - PACKED_Y_LENGTH) >> (64 - PACKED_Y_LENGTH)) as i32,
            (node << (64 - Z_OFFSET - PACKED_HORIZONTAL_LENGTH) >> (64 - PACKED_HORIZONTAL_LENGTH)) as i32,
        )
    }

    pub const fn java_hash(self) -> i32 {
        (self.y.wrapping_add(self.z.wrapping_mul(31))).wrapping_mul(31).wrapping_add(self.x)
    }

    pub const fn chunk(self) -> ChunkPos {
        ChunkPos::new(self.x >> 4, self.z >> 4)
    }

    pub fn center(self) -> Vec3 {
        Vec3::at_center_of(self)
    }

    pub fn within_manhattan(origin: BlockPos, reach: i32) -> impl Iterator<Item = BlockPos> {
        BlockPos::manhattan_ordered(origin, reach, reach, reach, reach)
    }

    pub fn within_box_by_manhattan_distance(origin: BlockPos, reach_x: i32, reach_y: i32, reach_z: i32) -> impl Iterator<Item = BlockPos> {
        BlockPos::manhattan_ordered(origin, reach_x, reach_y, reach_z, reach_x + reach_y + reach_z)
    }

    pub fn spiral_around(center: BlockPos, radius: i32, first_direction: Direction, second_direction: Direction) -> impl Iterator<Item = BlockPos> {
        let directions = [first_direction, second_direction, first_direction.opposite(), second_direction.opposite()];
        let legs = 4 * radius;
        let mut last = center.relative(second_direction);
        let (mut leg, mut leg_size, mut leg_index) = (-1, 0, 0);
        std::iter::from_fn(move || {
            let cursor = last.relative(directions[((leg + 4) % 4) as usize]);
            last = cursor;
            if leg_index >= leg_size {
                if leg >= legs {
                    return None;
                }
                leg += 1;
                leg_index = 0;
                leg_size = leg / 2 + 1;
            }
            leg_index += 1;
            Some(cursor)
        })
    }

    fn manhattan_ordered(origin: BlockPos, reach_x: i32, reach_y: i32, reach_z: i32, max_depth: i32) -> impl Iterator<Item = BlockPos> {
        (0..=max_depth).flat_map(move |depth| {
            let max_x = reach_x.min(depth);
            (-max_x..=max_x).flat_map(move |x| {
                let max_y = reach_y.min(depth - x.abs());
                (-max_y..=max_y).flat_map(move |y| {
                    let z = depth - x.abs() - y.abs();
                    let found = (z <= reach_z).then(|| BlockPos::new(origin.x + x, origin.y + y, origin.z + z));
                    let mirrored = (z <= reach_z && z != 0).then(|| BlockPos::new(origin.x + x, origin.y + y, origin.z - z));
                    found.into_iter().chain(mirrored)
                })
            })
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default, PartialOrd, Ord)]
pub struct ChunkPos {
    pub x: i32,
    pub z: i32,
}

impl ChunkPos {
    pub const fn new(x: i32, z: i32) -> ChunkPos {
        ChunkPos { x, z }
    }

    pub const fn pack(self) -> i64 {
        (self.x as i64 & 0xFFFF_FFFF) | ((self.z as i64 & 0xFFFF_FFFF) << 32)
    }

    pub const fn unpack(packed: i64) -> ChunkPos {
        ChunkPos::new(packed as i32, (packed >> 32) as i32)
    }

    pub const fn min_block_x(self) -> i32 {
        self.x << 4
    }

    pub const fn min_block_z(self) -> i32 {
        self.z << 4
    }

    pub fn chessboard_distance(self, o: ChunkPos) -> i32 {
        (self.x - o.x).abs().max((self.z - o.z).abs())
    }
}

pub struct SectionPos;

impl SectionPos {
    pub const fn block_to_section_coord(c: i32) -> i32 {
        c >> 4
    }

    pub const fn section_relative(c: i32) -> i32 {
        c & 15
    }

    pub const fn section_to_block_coord(c: i32) -> i32 {
        c << 4
    }

    pub const fn as_long(x: i32, y: i32, z: i32) -> i64 {
        ((x as i64 & 0x3F_FFFF) << 42) | (y as i64 & 0xF_FFFF) | ((z as i64 & 0x3F_FFFF) << 20)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packing_round_trips() {
        for pos in [BlockPos::new(0, 0, 0), BlockPos::new(-1, -64, 1), BlockPos::new(29_999_999, 2047, -30_000_000)] {
            assert_eq!(BlockPos::of_long(pos.as_long()), pos);
        }
        assert_eq!(ChunkPos::unpack(ChunkPos::new(-5, 7).pack()), ChunkPos::new(-5, 7));
    }
}
