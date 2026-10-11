mod block_light_engine;
pub mod level_light_engine;
mod light_engine;
mod section_storage;
pub mod sky_light_sources;
mod sky_light_engine;

use crisp_core::hash_map::FastMap;
use std::collections::{HashMap, HashSet};
use std::hash::{BuildHasherDefault, Hasher};
use std::sync::OnceLock;

use crisp_core::shapes::{self, VoxelShape};
use crisp_core::{AABB, Axis, AxisDirection, ChunkPos, Direction};
use crisp_data::BlockState;
use crisp_protocol::Writer;

pub use level_light_engine::LevelLightEngine;

use crate::world::level::chunk::LevelChunk;
use crate::world::level::light_layer::LightLayer;

pub trait LightChunkGetter {
    fn chunk_for_lighting(&self, x: i32, z: i32) -> Option<&LevelChunk>;
}

impl LightChunkGetter for FastMap<ChunkPos, LevelChunk> {
    fn chunk_for_lighting(&self, x: i32, z: i32) -> Option<&LevelChunk> {
        self.get(&ChunkPos::new(x, z))
    }
}

#[derive(Default, Clone, Copy)]
pub struct NodeHasher(u64);

impl Hasher for NodeHasher {
    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 = (self.0 ^ b as u64).wrapping_mul(0x100_0000_01b3);
        }
    }

    fn write_i64(&mut self, n: i64) {
        let h = (n as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        let h = h ^ h >> 32;
        self.0 = h ^ h >> 16;
    }

    fn finish(&self) -> u64 {
        self.0
    }
}

pub type NodeMap<V> = HashMap<i64, V, BuildHasherDefault<NodeHasher>>;
pub type NodeSet = HashSet<i64, BuildHasherDefault<NodeHasher>>;

pub mod node {
    use crisp_core::{BlockPos, Direction, SectionPos};

    pub fn of(x: i32, y: i32, z: i32) -> i64 {
        BlockPos::new(x, y, z).as_long()
    }

    pub fn x(node: i64) -> i32 {
        (node >> 38) as i32
    }

    pub fn y(node: i64) -> i32 {
        (node << 52 >> 52) as i32
    }

    pub fn z(node: i64) -> i32 {
        (node << 26 >> 38) as i32
    }

    pub fn offset(node: i64, direction: Direction) -> i64 {
        of(x(node) + direction.step_x(), y(node) + direction.step_y(), z(node) + direction.step_z())
    }

    pub fn flat_index(node: i64) -> i64 {
        node & -16
    }

    pub fn to_section(node: i64) -> i64 {
        SectionPos::as_long(x(node) >> 4, y(node) >> 4, z(node) >> 4)
    }
}

pub mod section {
    use crisp_core::SectionPos;

    pub fn of(x: i32, y: i32, z: i32) -> i64 {
        SectionPos::as_long(x, y, z)
    }

    pub fn x(section: i64) -> i32 {
        (section >> 42) as i32
    }

    pub fn y(section: i64) -> i32 {
        (section << 44 >> 44) as i32
    }

    pub fn z(section: i64) -> i32 {
        (section << 22 >> 42) as i32
    }

    pub fn offset(section: i64, dx: i32, dy: i32, dz: i32) -> i64 {
        of(x(section) + dx, y(section) + dy, z(section) + dz)
    }

    pub fn zero_node(x: i32, z: i32) -> i64 {
        of(x, 0, z)
    }

    pub fn zero_node_of(section: i64) -> i64 {
        section & !0xF_FFFF
    }
}

pub fn has_different_light_properties(old: BlockState, new: BlockState) -> bool {
    if old == new {
        return false;
    }
    old.light_dampening() != new.light_dampening()
        || old.light_emission() != new.light_emission()
        || old.use_shape_for_light_occlusion()
        || new.use_shape_for_light_occlusion()
}

pub fn is_empty_shape(state: BlockState) -> bool {
    !state.can_occlude() || !state.use_shape_for_light_occlusion()
}

struct LightShapes {
    faces: Vec<[u16; 6]>,
    masks: Vec<[u64; 4]>,
}

const EMPTY_FACE: u16 = 0;
const FULL_FACE: u16 = 1;
const FULL_MASK: [u64; 4] = [u64::MAX; 4];

fn light_shapes() -> &'static LightShapes {
    static SHAPES: OnceLock<LightShapes> = OnceLock::new();
    SHAPES.get_or_init(|| {
        let mut masks = vec![[0u64; 4], FULL_MASK];
        let mut faces = Vec::with_capacity(BlockState::count());
        let mut by_shape: HashMap<*const VoxelShape, [u16; 6]> = HashMap::new();
        for id in 0..BlockState::count() as u32 {
            let state = BlockState::by_id(id).unwrap();
            let shape = state.occlusion_voxel_shape();
            let face_ids = if is_empty_shape(state) || shape.is_empty() {
                [EMPTY_FACE; 6]
            } else if state.is_solid_render() {
                [FULL_FACE; 6]
            } else {
                *by_shape.entry(shape as *const VoxelShape).or_insert_with(|| {
                    Direction::VALUES.map(|direction| {
                        let mask = face_mask(&shape.face_shape(direction), direction.axis());
                        match masks.iter().position(|m| *m == mask) {
                            Some(i) => i as u16,
                            None => {
                                masks.push(mask);
                                (masks.len() - 1) as u16
                            }
                        }
                    })
                })
            };
            faces.push(face_ids);
        }
        LightShapes { faces, masks }
    })
}

fn face_mask(face: &VoxelShape, axis: Axis) -> [u64; 4] {
    let mut mask = [0u64; 4];
    for i in 0..16 {
        for j in 0..16 {
            let (u, v) = ((i as f64 + 0.5) / 16.0, (j as f64 + 0.5) / 16.0);
            let covered = match axis {
                Axis::X => face.contains_point(0.5, u, v),
                Axis::Y => face.contains_point(u, 0.5, v),
                Axis::Z => face.contains_point(u, v, 0.5),
            };
            if covered {
                let bit = i * 16 + j;
                mask[bit / 64] |= 1 << (bit % 64);
            }
        }
    }
    mask
}

pub fn occlusion_face(state: BlockState, direction: Direction) -> u16 {
    light_shapes().faces[state.id() as usize][direction.ordinal()]
}

pub fn face_shape_occludes(shape: u16, occluder: u16) -> bool {
    if shape == FULL_FACE || occluder == FULL_FACE {
        return true;
    }
    if shape == EMPTY_FACE && occluder == EMPTY_FACE {
        return false;
    }
    let masks = &light_shapes().masks;
    let (a, b) = (masks[shape as usize], masks[occluder as usize]);
    (0..4).all(|i| a[i] | b[i] == u64::MAX)
}

pub fn shape_occludes(from: BlockState, to: BlockState, direction: Direction) -> bool {
    face_shape_occludes(occlusion_face(from, direction), occlusion_face(to, direction.opposite()))
}

pub fn light_dampening_into(from: BlockState, to: BlockState, direction: Direction, simple_opacity: i32) -> i32 {
    let from_empty = is_empty_shape(from);
    let to_empty = is_empty_shape(to);
    if from_empty && to_empty {
        return simple_opacity;
    }
    let from_shape: &[AABB] = if from_empty { &[] } else { from.occlusion_shape() };
    let to_shape: &[AABB] = if to_empty { &[] } else { to.occlusion_shape() };
    if merged_face_occludes(from_shape, to_shape, direction) { 16 } else { simple_opacity }
}

fn merged_face_occludes(shape: &[AABB], occluder: &[AABB], direction: Direction) -> bool {
    let axis = direction.axis();
    let (first, second) = if direction.axis_direction() == AxisDirection::Positive { (shape, occluder) } else { (occluder, shape) };
    let stretch = |b: &AABB| match axis {
        Axis::X => AABB::new(0.0, b.min_y, b.min_z, 1.0, b.max_y, b.max_z),
        Axis::Y => AABB::new(b.min_x, 0.0, b.min_z, b.max_x, 1.0, b.max_z),
        Axis::Z => AABB::new(b.min_x, b.min_y, 0.0, b.max_x, b.max_y, 1.0),
    };
    let first_slice = first.iter().filter(|b| (axis.choose(b.max_x, b.max_y, b.max_z) - 1.0).abs() <= 1.0e-7).map(stretch);
    let second_slice = second.iter().filter(|b| axis.choose(b.min_x, b.min_y, b.min_z).abs() <= 1.0e-7).map(stretch);
    let joined: Vec<AABB> = first_slice.chain(second_slice).collect();
    shapes::is_shape_full_block(&VoxelShape::from_boxes(&joined))
}

pub fn write_light_data(w: &mut Writer, pos: ChunkPos, engine: &LevelLightEngine, sky_filter: Option<&[u64]>, block_filter: Option<&[u64]>) {
    let count = engine.light_section_count() as usize;
    let words = count.div_ceil(64);
    let (mut sky_mask, mut empty_sky_mask) = (vec![0u64; words], vec![0u64; words]);
    let (mut block_mask, mut empty_block_mask) = (vec![0u64; words], vec![0u64; words]);
    let (mut sky_updates, mut block_updates) = (Vec::new(), Vec::new());
    let is_set = |filter: Option<&[u64]>, i: usize| filter.is_none_or(|f| f.get(i / 64).is_some_and(|w| w >> (i % 64) & 1 != 0));
    for i in 0..count {
        let section_y = engine.min_light_section() + i as i32;
        let prepare = |layer, mask: &mut [u64], empty_mask: &mut [u64], updates: &mut Vec<Box<[u8; 2048]>>| {
            if let Some(data) = engine.data_layer_data(layer, section::of(pos.x, section_y, pos.z)) {
                if data.is_empty() {
                    empty_mask[i / 64] |= 1 << (i % 64);
                } else {
                    mask[i / 64] |= 1 << (i % 64);
                    updates.push(data.to_bytes());
                }
            }
        };
        if is_set(sky_filter, i) {
            prepare(LightLayer::Sky, &mut sky_mask, &mut empty_sky_mask, &mut sky_updates);
        }
        if is_set(block_filter, i) {
            prepare(LightLayer::Block, &mut block_mask, &mut empty_block_mask, &mut block_updates);
        }
    }
    w.bitset(&sky_mask);
    w.bitset(&block_mask);
    w.bitset(&empty_sky_mask);
    w.bitset(&empty_block_mask);
    for updates in [&sky_updates, &block_updates] {
        w.varint(updates.len() as i32);
        for data in updates {
            w.varint(data.len() as i32);
            w.bytes(&data[..]);
        }
    }
}

#[cfg(test)]
mod tests;
