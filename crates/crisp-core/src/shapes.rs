use crate::{AABB, Axis, Direction};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BooleanOp {
    False,
    NotOr,
    OnlySecond,
    NotFirst,
    OnlyFirst,
    NotSecond,
    NotSame,
    NotAnd,
    And,
    Same,
    Second,
    Causes,
    First,
    CausedBy,
    Or,
    True,
}

impl BooleanOp {
    pub fn apply(self, first: bool, second: bool) -> bool {
        match self {
            BooleanOp::False => false,
            BooleanOp::NotOr => !first && !second,
            BooleanOp::OnlySecond => second && !first,
            BooleanOp::NotFirst => !first,
            BooleanOp::OnlyFirst => first && !second,
            BooleanOp::NotSecond => !second,
            BooleanOp::NotSame => first != second,
            BooleanOp::NotAnd => !first || !second,
            BooleanOp::And => first && second,
            BooleanOp::Same => first == second,
            BooleanOp::Second => second,
            BooleanOp::Causes => !first || second,
            BooleanOp::First => first,
            BooleanOp::CausedBy => first || !second,
            BooleanOp::Or => first || second,
            BooleanOp::True => true,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct VoxelShape {
    coords: [Vec<f64>; 3],
    filled: Vec<bool>,
    first_full: [usize; 3],
    last_full: [usize; 3],
}

fn axis_index(axis: Axis) -> usize {
    axis as usize
}

impl VoxelShape {
    fn new(coords: [Vec<f64>; 3], filled: Vec<bool>) -> VoxelShape {
        let sizes = [coords[0].len() - 1, coords[1].len() - 1, coords[2].len() - 1];
        let mut first_full = sizes;
        let mut last_full = [0; 3];
        for x in 0..sizes[0] {
            for y in 0..sizes[1] {
                let row = (x * sizes[1] + y) * sizes[2];
                let Some(min_z) = filled[row..row + sizes[2]].iter().position(|&f| f) else { continue };
                let max_z = filled[row..row + sizes[2]].iter().rposition(|&f| f).unwrap();
                first_full = [first_full[0].min(x), first_full[1].min(y), first_full[2].min(min_z)];
                last_full = [last_full[0].max(x + 1), last_full[1].max(y + 1), last_full[2].max(max_z + 1)];
            }
        }
        VoxelShape { coords, filled, first_full, last_full }
    }

    pub fn empty() -> VoxelShape {
        VoxelShape::new([vec![0.0, 1.0], vec![0.0, 1.0], vec![0.0, 1.0]], vec![false])
    }

    pub fn block() -> VoxelShape {
        VoxelShape::new([vec![0.0, 1.0], vec![0.0, 1.0], vec![0.0, 1.0]], vec![true])
    }

    pub fn from_boxes(boxes: &[AABB]) -> VoxelShape {
        if boxes.is_empty() {
            return VoxelShape::empty();
        }
        let mut coords: [Vec<f64>; 3] = Default::default();
        for b in boxes {
            coords[0].extend([b.min_x, b.max_x]);
            coords[1].extend([b.min_y, b.max_y]);
            coords[2].extend([b.min_z, b.max_z]);
        }
        for c in &mut coords {
            c.sort_by(f64::total_cmp);
            c.dedup();
        }
        let (nx, ny, nz) = (coords[0].len() - 1, coords[1].len() - 1, coords[2].len() - 1);
        let mut filled = vec![false; nx * ny * nz];
        for x in 0..nx {
            for y in 0..ny {
                for z in 0..nz {
                    let c = [(coords[0][x] + coords[0][x + 1]) / 2.0, (coords[1][y] + coords[1][y + 1]) / 2.0, (coords[2][z] + coords[2][z + 1]) / 2.0];
                    filled[(x * ny + y) * nz + z] = boxes.iter().any(|b| b.contains(c[0], c[1], c[2]));
                }
            }
        }
        VoxelShape::new(coords, filled)
    }

    pub fn from_cells(coords: [Vec<f64>; 3], filled: Vec<bool>) -> VoxelShape {
        debug_assert_eq!(filled.len(), coords.iter().map(|c| c.len() - 1).product::<usize>());
        VoxelShape::new(coords, filled)
    }

    fn size(&self, axis: usize) -> usize {
        self.coords[axis].len() - 1
    }

    fn is_full(&self, x: usize, y: usize, z: usize) -> bool {
        self.filled[(x * self.size(1) + y) * self.size(2) + z]
    }

    pub fn is_empty(&self) -> bool {
        self.first_full[0] >= self.last_full[0]
    }

    fn first_full(&self, axis: usize) -> usize {
        self.first_full[axis]
    }

    fn last_full(&self, axis: usize) -> usize {
        self.last_full[axis]
    }

    pub fn min(&self, axis: Axis) -> f64 {
        let a = axis_index(axis);
        let i = self.first_full(a);
        if i >= self.size(a) { f64::INFINITY } else { self.coords[a][i] }
    }

    pub fn max(&self, axis: Axis) -> f64 {
        let a = axis_index(axis);
        let i = self.last_full(a);
        if i == 0 { f64::NEG_INFINITY } else { self.coords[a][i] }
    }

    fn find_index(&self, axis: usize, coord: f64) -> isize {
        self.coords[axis].partition_point(|&c| !(coord < c)) as isize - 1
    }

    fn is_cube_like_along(&self, axis: usize) -> bool {
        let c = &self.coords[axis];
        c.len() == 2 && (c[0] - 0.0).abs() <= 1.0e-7 && (c[1] - 1.0).abs() <= 1.0e-7
    }

    fn is_cube_like(&self) -> bool {
        (0..3).all(|a| self.is_cube_like_along(a))
    }

    pub fn face_shape(&self, direction: Direction) -> VoxelShape {
        let a = axis_index(direction.axis());
        if self.is_cube_like_along(a) {
            return self.clone();
        }
        let index = self.find_index(a, if direction.step_x() + direction.step_y() + direction.step_z() > 0 { 0.9999999 } else { 1.0e-7 });
        let mut coords = self.coords.clone();
        coords[a] = vec![0.0, 1.0];
        let (nx, ny, nz) = (coords[0].len() - 1, coords[1].len() - 1, coords[2].len() - 1);
        let mut filled = vec![false; nx * ny * nz];
        if index >= 0 && (index as usize) < self.size(a) {
            for x in 0..nx {
                for y in 0..ny {
                    for z in 0..nz {
                        let mut src = [x, y, z];
                        src[a] = index as usize;
                        filled[(x * ny + y) * nz + z] = self.is_full(src[0], src[1], src[2]);
                    }
                }
            }
        }
        let slice = VoxelShape::new(coords, filled);
        if slice.is_empty() {
            VoxelShape::empty()
        } else if slice.is_cube_like() && slice.filled.iter().all(|&f| f) {
            VoxelShape::block()
        } else {
            slice
        }
    }

    fn contains_cell_at(&self, point: [f64; 3]) -> bool {
        let mut idx = [0usize; 3];
        for a in 0..3 {
            let i = self.find_index(a, point[a]);
            if i < 0 || i as usize >= self.size(a) {
                return false;
            }
            idx[a] = i as usize;
        }
        self.is_full(idx[0], idx[1], idx[2])
    }

    pub fn moved(&self, dx: f64, dy: f64, dz: f64) -> VoxelShape {
        let offset = [dx, dy, dz];
        let mut coords = self.coords.clone();
        for (axis, list) in coords.iter_mut().enumerate() {
            list.iter_mut().for_each(|c| *c += offset[axis]);
        }
        VoxelShape { coords, filled: self.filled.clone(), first_full: self.first_full, last_full: self.last_full }
    }

    pub fn contains_point(&self, x: f64, y: f64, z: f64) -> bool {
        self.contains_cell_at([x, y, z])
    }

    pub fn coords(&self, axis: Axis) -> &[f64] {
        &self.coords[axis_index(axis)]
    }

    fn find_index_offset(&self, axis: usize, coord: f64, offset: f64) -> isize {
        self.coords[axis].partition_point(|&c| !(coord < c + offset)) as isize - 1
    }

    fn is_full_wide(&self, x: isize, y: isize, z: isize) -> bool {
        x >= 0
            && y >= 0
            && z >= 0
            && (x as usize) < self.size(0)
            && (y as usize) < self.size(1)
            && (z as usize) < self.size(2)
            && self.is_full(x as usize, y as usize, z as usize)
    }

    pub fn collide(&self, offset: [f64; 3], axis: Axis, moving: &AABB, mut distance: f64) -> f64 {
        if self.is_empty() {
            return distance;
        }
        if distance.abs() < 1.0e-7 {
            return 0.0;
        }
        let a = axis_index(axis);
        let (b, c) = ((a + 1) % 3, (a + 2) % 3);
        let min = |axis: usize| [moving.min_x, moving.min_y, moving.min_z][axis];
        let max = |axis: usize| [moving.max_x, moving.max_y, moving.max_z][axis];
        let (max_a, min_a) = (max(a), min(a));
        let a_min = self.find_index_offset(a, min_a + 1.0e-7, offset[a]);
        let a_max = self.find_index_offset(a, max_a - 1.0e-7, offset[a]);
        let b_min = 0.max(self.find_index_offset(b, min(b) + 1.0e-7, offset[b]));
        let b_max = (self.size(b) as isize).min(self.find_index_offset(b, max(b) - 1.0e-7, offset[b]) + 1);
        let c_min = 0.max(self.find_index_offset(c, min(c) + 1.0e-7, offset[c]));
        let c_max = (self.size(c) as isize).min(self.find_index_offset(c, max(c) - 1.0e-7, offset[c]) + 1);
        let full = |ia: isize, ib: isize, ic: isize| {
            let mut idx = [0isize; 3];
            idx[a] = ia;
            idx[b] = ib;
            idx[c] = ic;
            self.is_full_wide(idx[0], idx[1], idx[2])
        };
        let a_size = self.size(a) as isize;
        if distance > 0.0 {
            for ia in a_max + 1..a_size {
                for ib in b_min..b_max {
                    for ic in c_min..c_max {
                        if full(ia, ib, ic) {
                            let new_distance = self.coords[a][ia as usize] + offset[a] - max_a;
                            if new_distance >= -1.0e-7 {
                                distance = distance.min(new_distance);
                            }
                            return distance;
                        }
                    }
                }
            }
        } else if distance < 0.0 {
            for ia in (0..a_min).rev() {
                for ib in b_min..b_max {
                    for ic in c_min..c_max {
                        if full(ia, ib, ic) {
                            let new_distance = self.coords[a][ia as usize + 1] + offset[a] - min_a;
                            if new_distance <= 1.0e-7 {
                                distance = distance.max(new_distance);
                            }
                            return distance;
                        }
                    }
                }
            }
        }
        distance
    }

    pub fn to_aabbs(&self) -> Vec<AABB> {
        let (nx, ny, nz) = (self.size(0), self.size(1), self.size(2));
        let mut out = Vec::new();
        for x in 0..nx {
            for y in 0..ny {
                for z in 0..nz {
                    if self.is_full(x, y, z) {
                        let c = &self.coords;
                        out.push(AABB::new(c[0][x], c[1][y], c[2][z], c[0][x + 1], c[1][y + 1], c[2][z + 1]));
                    }
                }
            }
        }
        out
    }
}

struct IndexMerger {
    result: Vec<f64>,
    first_indices: Vec<isize>,
    second_indices: Vec<isize>,
}

impl IndexMerger {
    fn new(first: &[f64], second: &[f64], first_only_matters: bool, second_only_matters: bool) -> IndexMerger {
        let mut last_value = f64::NAN;
        let (first_size, second_size) = (first.len(), second.len());
        let capacity = first_size + second_size;
        let mut merger = IndexMerger { result: Vec::with_capacity(capacity), first_indices: Vec::with_capacity(capacity), second_indices: Vec::with_capacity(capacity) };
        let can_skip_first = !first_only_matters;
        let can_skip_second = !second_only_matters;
        let (mut first_index, mut second_index) = (0, 0);
        loop {
            let ran_out_of_first = first_index >= first_size;
            let ran_out_of_second = second_index >= second_size;
            if ran_out_of_first && ran_out_of_second {
                break;
            }
            let chose_first = !ran_out_of_first && (ran_out_of_second || first[first_index] < second[second_index] + 1.0e-7);
            if chose_first {
                first_index += 1;
                if can_skip_first && (second_index == 0 || ran_out_of_second) {
                    continue;
                }
            } else {
                second_index += 1;
                if can_skip_second && (first_index == 0 || ran_out_of_first) {
                    continue;
                }
            }
            let current_first_index = first_index as isize - 1;
            let current_second_index = second_index as isize - 1;
            let next_value = if chose_first { first[first_index - 1] } else { second[second_index - 1] };
            if !(last_value >= next_value - 1.0e-7) {
                merger.first_indices.push(current_first_index);
                merger.second_indices.push(current_second_index);
                merger.result.push(next_value);
                last_value = next_value;
            } else {
                *merger.first_indices.last_mut().unwrap() = current_first_index;
                *merger.second_indices.last_mut().unwrap() = current_second_index;
            }
        }
        if merger.result.is_empty() {
            merger.result.push(0.0);
            merger.first_indices.push(0);
            merger.second_indices.push(0);
        }
        merger
    }

    fn cells(&self) -> usize {
        self.result.len() - 1
    }
}

pub fn join_unoptimized(first: &VoxelShape, second: &VoxelShape, op: BooleanOp) -> VoxelShape {
    assert!(!op.apply(false, false), "joinUnoptimized needs an op that is false for empty cells");
    let first_only_matters = op.apply(true, false);
    let second_only_matters = op.apply(false, true);
    if first.is_empty() {
        return if second_only_matters { second.clone() } else { VoxelShape::empty() };
    }
    if second.is_empty() {
        return if first_only_matters { first.clone() } else { VoxelShape::empty() };
    }
    let [x_merger, y_merger, z_merger] = std::array::from_fn(|a| IndexMerger::new(&first.coords[a], &second.coords[a], first_only_matters, second_only_matters));
    let (nx, ny, nz) = (x_merger.cells(), y_merger.cells(), z_merger.cells());
    let mut filled = vec![false; nx * ny * nz];
    for xr in 0..nx {
        let (x1, x2) = (x_merger.first_indices[xr], x_merger.second_indices[xr]);
        for yr in 0..ny {
            let (y1, y2) = (y_merger.first_indices[yr], y_merger.second_indices[yr]);
            let row = (xr * ny + yr) * nz;
            for zr in 0..nz {
                filled[row + zr] = op.apply(first.is_full_wide(x1, y1, z_merger.first_indices[zr]), second.is_full_wide(x2, y2, z_merger.second_indices[zr]));
            }
        }
    }
    VoxelShape::new([x_merger.result, y_merger.result, z_merger.result], filled)
}

pub fn join_is_not_empty(first: &VoxelShape, second: &VoxelShape, op: BooleanOp) -> bool {
    assert!(!op.apply(false, false), "joinIsNotEmpty needs an op that is false for empty cells");
    let (first_empty, second_empty) = (first.is_empty(), second.is_empty());
    if first_empty || second_empty {
        return op.apply(!first_empty, !second_empty);
    }
    let first_only = op.apply(true, false);
    let second_only = op.apply(false, true);
    for axis in Axis::VALUES {
        if first.max(axis) < second.min(axis) - 1.0e-7 || second.max(axis) < first.min(axis) - 1.0e-7 {
            return first_only || second_only;
        }
    }
    let [x_merger, y_merger, z_merger] = std::array::from_fn(|a| IndexMerger::new(&first.coords[a], &second.coords[a], first_only, second_only));
    for xr in 0..x_merger.cells() {
        let (x1, x2) = (x_merger.first_indices[xr], x_merger.second_indices[xr]);
        for yr in 0..y_merger.cells() {
            let (y1, y2) = (y_merger.first_indices[yr], y_merger.second_indices[yr]);
            for zr in 0..z_merger.cells() {
                if op.apply(first.is_full_wide(x1, y1, z_merger.first_indices[zr]), second.is_full_wide(x2, y2, z_merger.second_indices[zr])) {
                    return true;
                }
            }
        }
    }
    false
}

pub fn is_shape_full_block(shape: &VoxelShape) -> bool {
    !join_is_not_empty(&VoxelShape::block(), shape, BooleanOp::NotSame)
}

pub fn is_face_full(shape: &VoxelShape, direction: Direction) -> bool {
    is_shape_full_block(&shape.face_shape(direction))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum AxisCycle {
    None,
    Forward,
    Backward,
}

impl AxisCycle {
    fn inverse(self) -> AxisCycle {
        match self {
            AxisCycle::None => AxisCycle::None,
            AxisCycle::Forward => AxisCycle::Backward,
            AxisCycle::Backward => AxisCycle::Forward,
        }
    }

    fn cycle(self, x: i32, y: i32, z: i32, axis: Axis) -> i32 {
        match self {
            AxisCycle::None => axis.choose(x, y, z),
            AxisCycle::Forward => axis.choose(z, x, y),
            AxisCycle::Backward => axis.choose(y, z, x),
        }
    }

    fn cycle_axis(self, axis: Axis) -> Axis {
        let ordinal = Axis::VALUES.iter().position(|&a| a == axis).unwrap_or(0);
        match self {
            AxisCycle::None => axis,
            AxisCycle::Forward => Axis::VALUES[(ordinal + 1) % 3],
            AxisCycle::Backward => Axis::VALUES[(ordinal + 2) % 3],
        }
    }
}

pub struct BitSetDiscreteVoxelShape {
    x_size: i32,
    y_size: i32,
    z_size: i32,
    storage: Vec<u64>,
}

impl BitSetDiscreteVoxelShape {
    pub fn new(x_size: i32, y_size: i32, z_size: i32) -> BitSetDiscreteVoxelShape {
        let bits = (x_size * y_size * z_size) as usize;
        BitSetDiscreteVoxelShape { x_size, y_size, z_size, storage: vec![0; bits.div_ceil(64)] }
    }

    fn get_index(&self, x: i32, y: i32, z: i32) -> usize {
        ((x * self.y_size + y) * self.z_size + z) as usize
    }

    pub fn get_size(&self, axis: Axis) -> i32 {
        axis.choose(self.x_size, self.y_size, self.z_size)
    }

    pub fn is_full(&self, x: i32, y: i32, z: i32) -> bool {
        let index = self.get_index(x, y, z);
        self.storage[index / 64] & (1 << (index % 64)) != 0
    }

    pub fn fill(&mut self, x: i32, y: i32, z: i32) {
        let index = self.get_index(x, y, z);
        self.storage[index / 64] |= 1 << (index % 64);
    }

    pub fn for_all_faces(&self, mut consumer: impl FnMut(Direction, i32, i32, i32)) {
        for transform in [AxisCycle::None, AxisCycle::Forward, AxisCycle::Backward] {
            self.for_all_axis_faces(&mut consumer, transform);
        }
    }

    fn for_all_axis_faces(&self, consumer: &mut impl FnMut(Direction, i32, i32, i32), transform: AxisCycle) {
        let inverse = transform.inverse();
        let c_axis = inverse.cycle_axis(Axis::Z);
        let a_size = self.get_size(inverse.cycle_axis(Axis::X));
        let b_size = self.get_size(inverse.cycle_axis(Axis::Y));
        let c_size = self.get_size(c_axis);
        let negative = Direction::from_axis_and_direction(c_axis, crate::AxisDirection::Negative);
        let positive = Direction::from_axis_and_direction(c_axis, crate::AxisDirection::Positive);
        for a in 0..a_size {
            for b in 0..b_size {
                let mut last_full = false;
                for c in 0..=c_size {
                    let full = c != c_size && self.is_full(inverse.cycle(a, b, c, Axis::X), inverse.cycle(a, b, c, Axis::Y), inverse.cycle(a, b, c, Axis::Z));
                    if !last_full && full {
                        consumer(negative, inverse.cycle(a, b, c, Axis::X), inverse.cycle(a, b, c, Axis::Y), inverse.cycle(a, b, c, Axis::Z));
                    }
                    if last_full && !full {
                        consumer(positive, inverse.cycle(a, b, c - 1, Axis::X), inverse.cycle(a, b, c - 1, Axis::Y), inverse.cycle(a, b, c - 1, Axis::Z));
                    }
                    last_full = full;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bx(x0: f64, y0: f64, z0: f64, x1: f64, y1: f64, z1: f64) -> AABB {
        AABB::new(x0, y0, z0, x1, y1, z1)
    }

    #[test]
    fn slab_faces() {
        let bottom = VoxelShape::from_boxes(&[bx(0.0, 0.0, 0.0, 1.0, 0.5, 1.0)]);
        assert!(is_face_full(&bottom, Direction::Down));
        assert!(!is_face_full(&bottom, Direction::Up));
        assert!(!is_face_full(&bottom, Direction::North));
        assert!(!is_shape_full_block(&bottom));
        assert!(is_shape_full_block(&VoxelShape::from_boxes(&[bx(0.0, 0.0, 0.0, 1.0, 0.5, 1.0), bx(0.0, 0.5, 0.0, 1.0, 1.0, 1.0)])));
    }

    #[test]
    fn post_does_not_cover_face() {
        let post = VoxelShape::from_boxes(&[bx(0.375, 0.0, 0.375, 0.625, 1.0, 0.625)]);
        assert!(!is_face_full(&post, Direction::Up));
        let face = post.face_shape(Direction::Up);
        assert!(join_is_not_empty(&face, &VoxelShape::from_boxes(&[bx(0.4, 0.0, 0.4, 0.6, 1.0, 0.6)]), BooleanOp::And));
    }
}
