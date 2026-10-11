use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, Vec3};

use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;

pub fn generate_random_direction(random: &mut impl RandomSource, horizontal_dist: i32, vertical_dist: i32) -> BlockPos {
    let xt = random.next_int_bounded(2 * horizontal_dist + 1) - horizontal_dist;
    let yt = random.next_int_bounded(2 * vertical_dist + 1) - vertical_dist;
    let zt = random.next_int_bounded(2 * horizontal_dist + 1) - horizontal_dist;
    BlockPos::new(xt, yt, zt)
}

pub fn move_up_out_of_solid(pos: BlockPos, max_y: i32, solidity_tester: impl Fn(BlockPos) -> bool) -> BlockPos {
    if !solidity_tester(pos) {
        return pos;
    }
    let mut on_ground_pos = pos.above();
    while on_ground_pos.y <= max_y && solidity_tester(on_ground_pos) {
        on_ground_pos = on_ground_pos.above();
    }
    on_ground_pos
}

pub fn generate_random_pos(level: &mut ServerLevel, mob: EntityId, mut pos_supplier: impl FnMut(&mut ServerLevel) -> Option<BlockPos>) -> Option<Vec3> {
    let mut best_weight = f64::NEG_INFINITY;
    let mut best_pos = None;
    for _ in 0..10 {
        if let Some(pos) = pos_supplier(level) {
            let value = level.get_walk_target_value(mob, pos) as f64;
            if value > best_weight {
                best_weight = value;
                best_pos = Some(pos);
            }
        }
    }
    best_pos.map(Vec3::at_bottom_center_of)
}

pub fn generate_random_pos_toward_direction(level: &mut ServerLevel, mob: EntityId, xz_dist: f64, direction: BlockPos) -> BlockPos {
    let mut xt = direction.x as f64;
    let mut zt = direction.z as f64;
    let m = level.mob_mut(mob).expect("a mob");
    if m.has_home() && xz_dist > 1.0 {
        let center = m.home_position;
        let e = &mut m.entity;
        if e.x() > center.x as f64 {
            xt -= e.random.next_double() * xz_dist / 2.0;
        } else {
            xt += e.random.next_double() * xz_dist / 2.0;
        }
        if e.z() > center.z as f64 {
            zt -= e.random.next_double() * xz_dist / 2.0;
        } else {
            zt += e.random.next_double() * xz_dist / 2.0;
        }
    }
    let e = &m.entity;
    BlockPos::containing(xt + e.x(), direction.y as f64 + e.y(), zt + e.z())
}
