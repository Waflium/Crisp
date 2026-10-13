use crisp_core::hash_map::FastSet;

use crisp_core::shapes::VoxelShape;
use crisp_core::{AABB, Axis, BlockPos, Direction, Vec3, mth};
use crisp_data::{BlockState, FluidState, block_tags, blocks, fluid_tags};

use crate::server::level::ServerLevel;
use crate::world::level::BlockGetter;
use crate::world::level::block::BlockHitResult;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ClipBlock {
    Collider,
    Outline,
    FallDamageResetting,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ClipFluid {
    None,
    SourceOnly,
    Any,
    Water,
}

impl ClipFluid {
    pub fn can_pick(self, fluid: FluidState) -> bool {
        match self {
            ClipFluid::None => false,
            ClipFluid::SourceOnly => fluid.is_source(),
            ClipFluid::Any => !fluid.is_empty(),
            ClipFluid::Water => fluid.is(&fluid_tags::WATER),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ClipContext {
    pub from: Vec3,
    pub to: Vec3,
    pub block: ClipBlock,
    pub fluid: ClipFluid,
    pub is_player: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClipResult {
    pub hit: BlockHitResult,
    pub miss: bool,
}

enum ClipShape {
    Empty,
    Block,
    Voxel(&'static VoxelShape),
    Boxes(Vec<AABB>),
}

impl ClipShape {
    fn clip(&self, from: Vec3, to: Vec3, pos: BlockPos) -> Option<BlockHitResult> {
        let owned;
        let shape = match self {
            ClipShape::Empty => return None,
            ClipShape::Block => {
                owned = VoxelShape::block();
                &owned
            }
            ClipShape::Voxel(shape) => *shape,
            ClipShape::Boxes(boxes) => {
                owned = VoxelShape::from_boxes(boxes);
                &owned
            }
        };
        voxel_clip(shape, from, to, pos)
    }
}

fn voxel_clip(shape: &VoxelShape, from: Vec3, to: Vec3, pos: BlockPos) -> Option<BlockHitResult> {
    if shape.is_empty() {
        return None;
    }
    let diff = to.subtract(from);
    if diff.length_sqr() < 1.0e-7 {
        return None;
    }
    let test_point = from.add_vec(diff.scale(0.001));
    if shape.contains_point(test_point.x - pos.x as f64, test_point.y - pos.y as f64, test_point.z - pos.z as f64) {
        let direction = Direction::approximate_nearest(diff.x as f32, diff.y as f32, diff.z as f32).opposite();
        return Some(BlockHitResult { location: test_point, direction, block_pos: pos, inside: true, world_border_hit: false });
    }
    clip_boxes(&shape.to_aabbs(), from, to, pos)
}

fn clip_boxes(boxes: &[AABB], from: Vec3, to: Vec3, pos: BlockPos) -> Option<BlockHitResult> {
    let mut scale = 1.0;
    let mut direction = None;
    let (dx, dy, dz) = (to.x - from.x, to.y - from.y, to.z - from.z);
    for b in boxes {
        direction = b.move_to_block(pos).clip_direction(from, &mut scale, direction, dx, dy, dz);
    }
    let direction = direction?;
    Some(BlockHitResult { location: from.add(scale * dx, scale * dy, scale * dz), direction, block_pos: pos, inside: false, world_border_hit: false })
}

pub fn traverse_blocks<T>(from: Vec3, to: Vec3, mut visit: impl FnMut(BlockPos) -> Option<T>, miss: impl FnOnce() -> T) -> T {
    if from == to {
        return miss();
    }
    let to_x = mth::lerp(-1.0e-7, to.x, from.x);
    let to_y = mth::lerp(-1.0e-7, to.y, from.y);
    let to_z = mth::lerp(-1.0e-7, to.z, from.z);
    let from_x = mth::lerp(-1.0e-7, from.x, to.x);
    let from_y = mth::lerp(-1.0e-7, from.y, to.y);
    let from_z = mth::lerp(-1.0e-7, from.z, to.z);
    let (mut x, mut y, mut z) = (mth::floor(from_x), mth::floor(from_y), mth::floor(from_z));
    if let Some(first) = visit(BlockPos::new(x, y, z)) {
        return first;
    }
    let (dx, dy, dz) = (to_x - from_x, to_y - from_y, to_z - from_z);
    let (sign_x, sign_y, sign_z) = (mth::sign(dx), mth::sign(dy), mth::sign(dz));
    let t_delta_x = if sign_x == 0 { f64::MAX } else { sign_x as f64 / dx };
    let t_delta_y = if sign_y == 0 { f64::MAX } else { sign_y as f64 / dy };
    let t_delta_z = if sign_z == 0 { f64::MAX } else { sign_z as f64 / dz };
    let mut t_x = t_delta_x * if sign_x > 0 { 1.0 - mth::frac(from_x) } else { mth::frac(from_x) };
    let mut t_y = t_delta_y * if sign_y > 0 { 1.0 - mth::frac(from_y) } else { mth::frac(from_y) };
    let mut t_z = t_delta_z * if sign_z > 0 { 1.0 - mth::frac(from_z) } else { mth::frac(from_z) };
    while t_x <= 1.0 || t_y <= 1.0 || t_z <= 1.0 {
        if t_x < t_y {
            if t_x < t_z {
                x += sign_x;
                t_x += t_delta_x;
            } else {
                z += sign_z;
                t_z += t_delta_z;
            }
        } else if t_y < t_z {
            y += sign_y;
            t_y += t_delta_y;
        } else {
            z += sign_z;
            t_z += t_delta_z;
        }
        if let Some(result) = visit(BlockPos::new(x, y, z)) {
            return result;
        }
    }
    miss()
}

pub fn between_corners_in_direction(first: BlockPos, second: BlockPos, direction: Vec3) -> Vec<BlockPos> {
    let (min_x, min_y, min_z) = (first.x.min(second.x), first.y.min(second.y), first.z.min(second.z));
    let (max_x, max_y, max_z) = (first.x.max(second.x), first.y.max(second.y), first.z.max(second.z));
    let diff = [max_x - min_x, max_y - min_y, max_z - min_z];
    let start = BlockPos::new(
        if direction.x >= 0.0 { min_x } else { max_x },
        if direction.y >= 0.0 { min_y } else { max_y },
        if direction.z >= 0.0 { min_z } else { max_z },
    );
    let axes = Axis::step_order(direction.x, direction.z);
    let dirs = axes.map(|axis| if direction.get(axis) >= 0.0 { axis.positive() } else { axis.negative() });
    let maxes = axes.map(|axis| diff[axis as usize]);
    let mut out = Vec::with_capacity(((maxes[0] + 1) * (maxes[1] + 1) * (maxes[2] + 1)) as usize);
    for first_index in 0..=maxes[0] {
        for second_index in 0..=maxes[1] {
            for third_index in 0..=maxes[2] {
                let step = |d: Direction| (d.step_x(), d.step_y(), d.step_z());
                let (a, b, c) = (step(dirs[0]), step(dirs[1]), step(dirs[2]));
                out.push(BlockPos::new(
                    start.x + a.0 * first_index + b.0 * second_index + c.0 * third_index,
                    start.y + a.1 * first_index + b.1 * second_index + c.1 * third_index,
                    start.z + a.2 * first_index + b.2 * second_index + c.2 * third_index,
                ));
            }
        }
    }
    out
}

fn corners_of(aabb: &AABB) -> (BlockPos, BlockPos) {
    (
        BlockPos::new(mth::floor(aabb.min_x), mth::floor(aabb.min_y), mth::floor(aabb.min_z)),
        BlockPos::new(mth::floor(aabb.max_x), mth::floor(aabb.max_y), mth::floor(aabb.max_z)),
    )
}

fn furthest_corner(direction: Vec3) -> (i32, i32, i32) {
    let (x_dot, y_dot, z_dot) = (direction.x.abs(), direction.y.abs(), direction.z.abs());
    let x_sign = if direction.x >= 0.0 { 1 } else { -1 };
    let y_sign = if direction.y >= 0.0 { 1 } else { -1 };
    let z_sign = if direction.z >= 0.0 { 1 } else { -1 };
    if x_dot <= y_dot && x_dot <= z_dot {
        (-x_sign, -z_sign, y_sign)
    } else if y_dot <= z_dot {
        (z_sign, -y_sign, -x_sign)
    } else {
        (-y_sign, x_sign, -z_sign)
    }
}

pub fn for_each_block_intersected_between(from: Vec3, to: Vec3, aabb_at_target: &AABB, mut visit: impl FnMut(BlockPos, i32) -> bool) -> bool {
    let travel = to.subtract(from);
    if travel.length_sqr() < mth::square(1.0e-5f32 as f64) {
        let start = BlockPos::containing(aabb_at_target.min_x, aabb_at_target.min_y, aabb_at_target.min_z);
        let end = BlockPos::containing(aabb_at_target.max_x, aabb_at_target.max_y, aabb_at_target.max_z);
        for z in start.z.min(end.z)..=start.z.max(end.z) {
            for y in start.y.min(end.y)..=start.y.max(end.y) {
                for x in start.x.min(end.x)..=start.x.max(end.x) {
                    if !visit(BlockPos::new(x, y, z), 0) {
                        return false;
                    }
                }
            }
        }
        return true;
    }
    let mut visited: FastSet<BlockPos> = FastSet::default();
    let (first, second) = corners_of(&aabb_at_target.move_vec(travel.scale(-1.0)));
    for pos in between_corners_in_direction(first, second, travel) {
        if !visit(pos, 0) {
            return false;
        }
        visited.insert(pos);
    }
    let Some(iterations) = add_collisions_along_travel(&mut visited, travel, aabb_at_target, &mut visit) else { return false };
    let (first, second) = corners_of(aabb_at_target);
    for pos in between_corners_in_direction(first, second, travel) {
        if visited.insert(pos) && !visit(pos, iterations + 1) {
            return false;
        }
    }
    true
}

fn add_collisions_along_travel(
    visited: &mut FastSet<BlockPos>,
    delta_move: Vec3,
    aabb_at_target: &AABB,
    visit: &mut impl FnMut(BlockPos, i32) -> bool,
) -> Option<i32> {
    let (size_x, size_y, size_z) = (aabb_at_target.x_size(), aabb_at_target.y_size(), aabb_at_target.z_size());
    let corner = furthest_corner(delta_move);
    let to_center = aabb_at_target.center();
    let to_corner = Vec3::new(
        to_center.x + size_x * 0.5 * corner.0 as f64,
        to_center.y + size_y * 0.5 * corner.1 as f64,
        to_center.z + size_z * 0.5 * corner.2 as f64,
    );
    let from_corner = to_corner.subtract(delta_move);
    let (mut x, mut y, mut z) = (mth::floor(from_corner.x), mth::floor(from_corner.y), mth::floor(from_corner.z));
    let (sign_x, sign_y, sign_z) = (mth::sign(delta_move.x), mth::sign(delta_move.y), mth::sign(delta_move.z));
    let t_delta_x = if sign_x == 0 { f64::MAX } else { sign_x as f64 / delta_move.x };
    let t_delta_y = if sign_y == 0 { f64::MAX } else { sign_y as f64 / delta_move.y };
    let t_delta_z = if sign_z == 0 { f64::MAX } else { sign_z as f64 / delta_move.z };
    let mut t_x = t_delta_x * if sign_x > 0 { 1.0 - mth::frac(from_corner.x) } else { mth::frac(from_corner.x) };
    let mut t_y = t_delta_y * if sign_y > 0 { 1.0 - mth::frac(from_corner.y) } else { mth::frac(from_corner.y) };
    let mut t_z = t_delta_z * if sign_z > 0 { 1.0 - mth::frac(from_corner.z) } else { mth::frac(from_corner.z) };
    let mut iterations = 0;
    while t_x <= 1.0 || t_y <= 1.0 || t_z <= 1.0 {
        if t_x < t_y {
            if t_x < t_z {
                x += sign_x;
                t_x += t_delta_x;
            } else {
                z += sign_z;
                t_z += t_delta_z;
            }
        } else if t_y < t_z {
            y += sign_y;
            t_y += t_delta_y;
        } else {
            z += sign_z;
            t_z += t_delta_z;
        }
        let cell = AABB::new(x as f64, y as f64, z as f64, (x + 1) as f64, (y + 1) as f64, (z + 1) as f64);
        let Some(hit) = cell.clip(from_corner, to_corner) else { continue };
        iterations += 1;
        let eps = 1.0e-5f32 as f64;
        let hit_x = mth::clamp(hit.x, x as f64 + eps, x as f64 + 1.0 - eps);
        let hit_y = mth::clamp(hit.y, y as f64 + eps, y as f64 + 1.0 - eps);
        let hit_z = mth::clamp(hit.z, z as f64 + eps, z as f64 + 1.0 - eps);
        let opposite = BlockPos::new(
            mth::floor(hit_x - size_x * corner.0 as f64),
            mth::floor(hit_y - size_y * corner.1 as f64),
            mth::floor(hit_z - size_z * corner.2 as f64),
        );
        for pos in between_corners_in_direction(BlockPos::new(x, y, z), opposite, delta_move) {
            if visited.insert(pos) && !visit(pos, iterations) {
                return None;
            }
        }
    }
    Some(iterations)
}

impl ServerLevel {
    fn clip_block_shape(&self, ctx: &ClipContext, state: BlockState) -> ClipShape {
        match ctx.block {
            ClipBlock::Collider => ClipShape::Voxel(state.collision_voxel_shape()),
            ClipBlock::Outline => ClipShape::Voxel(state.voxel_shape()),
            ClipBlock::FallDamageResetting => {
                if state.is(&block_tags::FALL_DAMAGE_RESETTING) || ctx.is_player && (state.is_block(blocks::END_GATEWAY) || state.is_block(blocks::END_PORTAL)) {
                    ClipShape::Block
                } else {
                    ClipShape::Empty
                }
            }
        }
    }

    pub fn clip(&mut self, ctx: &ClipContext) -> ClipResult {
        let ctx = *ctx;
        traverse_blocks(
            ctx.from,
            ctx.to,
            |pos| {
                let state = self.block_state(pos);
                let fluid = self.fluid_state(pos);
                let block_shape = self.clip_block_shape(&ctx, state);
                let block_result = self.clip_with_interaction_override(ctx.from, ctx.to, pos, &block_shape, state);
                let fluid_shape = if ctx.fluid.can_pick(fluid) {
                    let height = self.fluid_height(fluid, pos) as f64;
                    ClipShape::Boxes(vec![AABB::new(0.0, 0.0, 0.0, 1.0, height, 1.0)])
                } else {
                    ClipShape::Empty
                };
                let liquid_result = fluid_shape.clip(ctx.from, ctx.to, pos);
                let block_distance = block_result.map_or(f64::MAX, |r| ctx.from.distance_to_sqr(r.location));
                let liquid_distance = liquid_result.map_or(f64::MAX, |r| ctx.from.distance_to_sqr(r.location));
                let result = if block_distance <= liquid_distance { block_result } else { liquid_result };
                result.map(|hit| ClipResult { hit, miss: false })
            },
            || {
                let delta = ctx.from.subtract(ctx.to);
                let direction = Direction::approximate_nearest(delta.x as f32, delta.y as f32, delta.z as f32);
                let hit = BlockHitResult { location: ctx.to, direction, block_pos: BlockPos::containing_vec(ctx.to), inside: false, world_border_hit: false };
                ClipResult { hit, miss: true }
            },
        )
    }

    pub fn clip_including_border(&mut self, ctx: &ClipContext) -> ClipResult {
        let result = self.clip(ctx);
        let location = result.hit.location;
        if self.world_border.is_within_bounds(ctx.from.x, ctx.from.z) && !self.world_border.is_within_bounds(location.x, location.z) {
            let delta = location.subtract(ctx.from);
            let direction = Direction::approximate_nearest(delta.x as f32, delta.y as f32, delta.z as f32);
            let hit = self.world_border.clamp_vec3_to_bound(location.x, location.y, location.z);
            ClipResult { hit: BlockHitResult { location: hit, direction, block_pos: BlockPos::containing_vec(hit), inside: false, world_border_hit: true }, miss: false }
        } else {
            result
        }
    }

    fn clip_with_interaction_override(&self, from: Vec3, to: Vec3, pos: BlockPos, shape: &ClipShape, state: BlockState) -> Option<BlockHitResult> {
        let result = shape.clip(from, to, pos)?;
        let interaction = ClipShape::Boxes(state.interaction_shape().to_vec());
        if let Some(over) = interaction.clip(from, to, pos)
            && over.location.subtract(from).length_sqr() < result.location.subtract(from).length_sqr()
        {
            return Some(BlockHitResult { direction: over.direction, ..result });
        }
        Some(result)
    }
}
