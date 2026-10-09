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
}

fn axis_index(axis: Axis) -> usize {
    axis as usize
}

impl VoxelShape {
    pub fn empty() -> VoxelShape {
        VoxelShape { coords: [vec![0.0, 1.0], vec![0.0, 1.0], vec![0.0, 1.0]], filled: vec![false] }
    }

    pub fn block() -> VoxelShape {
        VoxelShape { coords: [vec![0.0, 1.0], vec![0.0, 1.0], vec![0.0, 1.0]], filled: vec![true] }
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
        VoxelShape { coords, filled }
    }

    pub fn from_cells(coords: [Vec<f64>; 3], filled: Vec<bool>) -> VoxelShape {
        debug_assert_eq!(filled.len(), coords.iter().map(|c| c.len() - 1).product::<usize>());
        VoxelShape { coords, filled }
    }

    fn size(&self, axis: usize) -> usize {
        self.coords[axis].len() - 1
    }

    fn is_full(&self, x: usize, y: usize, z: usize) -> bool {
        self.filled[(x * self.size(1) + y) * self.size(2) + z]
    }

    pub fn is_empty(&self) -> bool {
        !self.filled.iter().any(|&f| f)
    }

    fn first_full(&self, axis: usize) -> usize {
        (0..self.size(axis)).find(|&i| self.layer_has_any(axis, i)).unwrap_or(self.size(axis))
    }

    fn last_full(&self, axis: usize) -> usize {
        (0..self.size(axis)).rev().find(|&i| self.layer_has_any(axis, i)).map_or(0, |i| i + 1)
    }

    fn layer_has_any(&self, axis: usize, i: usize) -> bool {
        let (nx, ny, nz) = (self.size(0), self.size(1), self.size(2));
        (0..nx).any(|x| (0..ny).any(|y| (0..nz).any(|z| [x, y, z][axis] == i && self.is_full(x, y, z))))
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
        let coords = &self.coords[axis];
        (0..coords.len()).find(|&i| coord < coords[i]).unwrap_or(coords.len()) as isize - 1
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
        let slice = VoxelShape { coords, filled };
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
        VoxelShape { coords, filled: self.filled.clone() }
    }

    pub fn contains_point(&self, x: f64, y: f64, z: f64) -> bool {
        self.contains_cell_at([x, y, z])
    }

    pub fn coords(&self, axis: Axis) -> &[f64] {
        &self.coords[axis_index(axis)]
    }

    fn find_index_offset(&self, axis: usize, coord: f64, offset: f64) -> isize {
        let coords = &self.coords[axis];
        (0..coords.len()).find(|&i| coord < coords[i] + offset).unwrap_or(coords.len()) as isize - 1
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

fn merge_coords(a: &[f64], b: &[f64]) -> Vec<f64> {
    let mut all: Vec<f64> = a.iter().chain(b).copied().collect();
    all.sort_by(f64::total_cmp);
    let mut out: Vec<f64> = Vec::new();
    for v in all {
        if out.last().is_none_or(|&last| last < v - 1.0e-7) {
            out.push(v);
        }
    }
    out
}

pub fn join_unoptimized(first: &VoxelShape, second: &VoxelShape, op: BooleanOp) -> VoxelShape {
    let coords: [Vec<f64>; 3] = std::array::from_fn(|a| merge_coords(&first.coords[a], &second.coords[a]));
    let (nx, ny, nz) = (coords[0].len() - 1, coords[1].len() - 1, coords[2].len() - 1);
    let mut filled = vec![false; nx * ny * nz];
    for x in 0..nx {
        for y in 0..ny {
            for z in 0..nz {
                let p = [(coords[0][x] + coords[0][x + 1]) / 2.0, (coords[1][y] + coords[1][y + 1]) / 2.0, (coords[2][z] + coords[2][z + 1]) / 2.0];
                filled[(x * ny + y) * nz + z] = op.apply(first.contains_cell_at(p), second.contains_cell_at(p));
            }
        }
    }
    VoxelShape { coords, filled }
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
    let merged: Vec<Vec<f64>> = (0..3).map(|a| merge_coords(&first.coords[a], &second.coords[a])).collect();
    for x in 0..merged[0].len() - 1 {
        for y in 0..merged[1].len() - 1 {
            for z in 0..merged[2].len() - 1 {
                let p = [(merged[0][x] + merged[0][x + 1]) / 2.0, (merged[1][y] + merged[1][y + 1]) / 2.0, (merged[2][z] + merged[2][z + 1]) / 2.0];
                if op.apply(first.contains_cell_at(p), second.contains_cell_at(p)) {
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
