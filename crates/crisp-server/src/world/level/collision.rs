use std::cmp::Ordering;

use crisp_core::shapes::{BooleanOp, VoxelShape, join_is_not_empty};
use crisp_core::{AABB, Axis, BlockPos, ChunkPos, Vec3, mth};
use crisp_data::blocks;

use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::level::BlockGetter;

#[derive(Clone, Copy, Debug)]
pub struct Collider<'a> {
    pub shape: &'a VoxelShape,
    pub offset: [f64; 3],
}

impl Collider<'_> {
    pub fn collide(&self, axis: Axis, moving: &AABB, distance: f64) -> f64 {
        self.shape.collide(self.offset, axis, moving, distance)
    }

    pub fn coords(&self, axis: Axis) -> impl Iterator<Item = f64> + '_ {
        let offset = self.offset[axis as usize];
        self.shape.coords(axis).iter().map(move |c| c + offset)
    }
}

static FULL_BLOCK: std::sync::LazyLock<VoxelShape> = std::sync::LazyLock::new(VoxelShape::block);

pub fn shapes_collide(axis: Axis, moving: &AABB, colliders: &[Collider], mut distance: f64) -> f64 {
    for collider in colliders {
        if distance.abs() < 1.0e-7 {
            return 0.0;
        }
        distance = collider.collide(axis, moving, distance);
    }
    distance
}

pub fn collide_with_shapes(movement: Vec3, bounding_box: &AABB, colliders: &[Collider]) -> Vec3 {
    if colliders.is_empty() {
        return movement;
    }
    let mut resolved = Vec3::ZERO;
    for axis in Axis::step_order(movement.x, movement.z) {
        let axis_movement = movement.get(axis);
        if axis_movement != 0.0 {
            let collision = shapes_collide(axis, &bounding_box.move_vec(resolved), colliders, axis_movement);
            resolved = resolved.with(axis, collision);
        }
    }
    resolved
}

pub fn compare_block_pos(a: BlockPos, b: BlockPos) -> Ordering {
    if a.y == b.y {
        if a.z == b.z { a.x.cmp(&b.x) } else { a.z.cmp(&b.z) }
    } else {
        a.y.cmp(&b.y)
    }
}

impl ServerLevel {
    pub fn block_collisions(&self, box_: &AABB, only_suffocating_blocks: bool) -> Vec<(BlockPos, Collider<'static>)> {
        let mut out = Vec::new();
        self.for_each_block_collision(box_, only_suffocating_blocks, false, |pos, collider| {
            out.push((pos, collider));
            true
        });
        out
    }

    fn for_each_block_collision(&self, box_: &AABB, only_suffocating_blocks: bool, always_collide_with_fluid: bool, mut visit: impl FnMut(BlockPos, Collider<'static>) -> bool) -> bool {
        let x0 = mth::floor(box_.min_x - 1.0e-7) - 1;
        let x1 = mth::floor(box_.max_x + 1.0e-7) + 1;
        let y0 = mth::floor(box_.min_y - 1.0e-7) - 1;
        let y1 = mth::floor(box_.max_y + 1.0e-7) + 1;
        let z0 = mth::floor(box_.min_z - 1.0e-7) - 1;
        let z1 = mth::floor(box_.max_z + 1.0e-7) + 1;
        let (width, height, depth) = (x1 - x0 + 1, y1 - y0 + 1, z1 - z0 + 1);
        let mut entity_shape: Option<VoxelShape> = None;
        let mut cached_chunk: Option<(ChunkPos, Option<&crate::world::level::chunk::LevelChunk>)> = None;
        for index in 0..width * height * depth {
            let (cx, slice) = (index % width, index / width);
            let (cy, cz) = (slice % height, slice / height);
            let mut face_type = 0;
            if cx == 0 || cx == width - 1 {
                face_type += 1;
            }
            if cy == 0 || cy == height - 1 {
                face_type += 1;
            }
            if cz == 0 || cz == depth - 1 {
                face_type += 1;
            }
            if face_type == 3 {
                continue;
            }
            let (x, y, z) = (x0 + cx, y0 + cy, z0 + cz);
            let chunk_pos = ChunkPos::new(x >> 4, z >> 4);
            let chunk = match cached_chunk {
                Some((p, c)) if p == chunk_pos => c,
                _ => {
                    let c = self.chunks.loaded_chunk(chunk_pos);
                    cached_chunk = Some((chunk_pos, c));
                    c
                }
            };
            let Some(chunk) = chunk else { continue };
            let pos = BlockPos::new(x, y, z);
            let state = chunk.block_state(pos);
            if only_suffocating_blocks && !state.is_suffocating() {
                continue;
            }
            if face_type == 1 && !state.has_large_collision_shape() || face_type == 2 && !state.is_block(blocks::MOVING_PISTON) {
                continue;
            }
            let liquid_block = always_collide_with_fluid && (state.is_block(blocks::WATER) || state.is_block(blocks::LAVA));
            let shape = if liquid_block { &*FULL_BLOCK } else { state.collision_voxel_shape() };
            let collider = Collider { shape, offset: [x as f64, y as f64, z as f64] };
            let collides = if liquid_block || state.is_collision_shape_full_block() {
                box_.intersects_bounds(x as f64, y as f64, z as f64, x as f64 + 1.0, y as f64 + 1.0, z as f64 + 1.0)
            } else if !shape.is_empty() {
                let moved = shape.moved(x as f64, y as f64, z as f64);
                let entity_shape = entity_shape.get_or_insert_with(|| VoxelShape::from_boxes(&[*box_]));
                join_is_not_empty(&moved, entity_shape, BooleanOp::And)
            } else {
                false
            };
            if collides && !visit(pos, collider) {
                return false;
            }
        }
        true
    }

    pub fn no_collision(&self, box_: &AABB) -> bool {
        self.for_each_block_collision(box_, false, false, |_, c| c.shape.is_empty())
    }

    pub fn no_block_and_liquid_collision(&self, box_: &AABB) -> bool {
        self.for_each_block_collision(box_, false, true, |_, c| c.shape.is_empty())
    }

    pub fn no_collision_for(&self, entity: EntityId, box_: &AABB) -> bool {
        self.no_collision(box_) && self.no_border_collision(entity, box_)
    }

    pub fn no_border_collision(&self, entity: EntityId, box_: &AABB) -> bool {
        self.border_collision(entity, box_).is_none_or(|shape| !join_is_not_empty(shape, &VoxelShape::from_boxes(&[*box_]), BooleanOp::And))
    }

    pub fn border_collision(&self, entity: EntityId, box_: &AABB) -> Option<&VoxelShape> {
        let border = &self.world_border;
        border.is_inside_close_to_border(self.entity(entity), box_).then(|| border.get_collision_shape())
    }

    pub fn find_supporting_block(&self, position: Vec3, box_: &AABB) -> Option<BlockPos> {
        let mut main_support: Option<BlockPos> = None;
        let mut main_support_distance = f64::MAX;
        for (pos, _) in self.block_collisions(box_, false) {
            let distance = pos.dist_to_center_sqr(position.x, position.y, position.z);
            if distance < main_support_distance
                || distance == main_support_distance && main_support.is_none_or(|m| compare_block_pos(m, pos) == Ordering::Less)
            {
                main_support = Some(pos);
                main_support_distance = distance;
            }
        }
        main_support
    }

    pub fn has_chunks_at(&self, x0: i32, z0: i32, x1: i32, z1: i32) -> bool {
        for chunk_x in x0 >> 4..=x1 >> 4 {
            for chunk_z in z0 >> 4..=z1 >> 4 {
                if !self.chunks.has_chunk(ChunkPos::new(chunk_x, chunk_z)) {
                    return false;
                }
            }
        }
        true
    }

    pub fn has_chunk_at(&self, pos: BlockPos) -> bool {
        self.chunks.has_chunk(ChunkPos::new(pos.x >> 4, pos.z >> 4))
    }

    pub fn contains_any_liquid(&mut self, box_: &AABB) -> bool {
        let from = BlockPos::containing(box_.min_x, box_.min_y, box_.min_z);
        let to = BlockPos::containing(box_.max_x, box_.max_y, box_.max_z);
        for z in from.z..=to.z {
            for y in from.y..=to.y {
                for x in from.x..=to.x {
                    if !self.block_state(BlockPos::new(x, y, z)).fluid_state().is_empty() {
                        return true;
                    }
                }
            }
        }
        false
    }

    pub fn all_air_in(&mut self, box_: &AABB) -> bool {
        let from = BlockPos::containing(box_.min_x, box_.min_y, box_.min_z);
        let to = BlockPos::containing(box_.max_x, box_.max_y, box_.max_z);
        for z in from.z..=to.z {
            for y in from.y..=to.y {
                for x in from.x..=to.x {
                    if !self.block_state(BlockPos::new(x, y, z)).is_air() {
                        return false;
                    }
                }
            }
        }
        true
    }
}
