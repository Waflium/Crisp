use crate::world::level::world_gen_level::WorldGenLevel;
use crisp_core::random::RandomSource;
use crisp_core::{Axis, BlockPos, Direction};
use crisp_data::blocks;
use crisp_data::properties::{AGE_1, HORIZONTAL_FACING};

use super::tree_decorator::Context;

pub fn place<R: RandomSource>(placement_probability: f32, context: &mut Context<impl WorldGenLevel, R>) {
    if context.random.next_float() >= placement_probability {
        return;
    }
    if context.logs.is_empty() {
        return;
    }
    let logs = context.logs.clone();
    if is_fallen_log(&logs) {
        place_on_fallen_log(context, &logs);
    } else {
        place_on_standing_tree(context, &logs);
    }
}

fn place_on_standing_tree<R: RandomSource>(context: &mut Context<impl WorldGenLevel, R>, logs: &[BlockPos]) {
    let directions = pick_two_perpendicular_directions(context.random);
    let tree_base_y = logs[0].y;
    for &log_pos in logs {
        if is_within_decoratable_height(log_pos, tree_base_y) {
            for facing in directions {
                if context.random.next_float() <= 0.25 && try_place_mushroom_on_standing_tree(context, log_pos, facing) {
                    break;
                }
            }
        }
    }
}

fn place_on_fallen_log<R: RandomSource>(context: &mut Context<impl WorldGenLevel, R>, logs: &[BlockPos]) {
    let directions = perpendicular_to_fallen_log(logs);
    for &log_pos in logs {
        for facing in directions {
            if context.random.next_float() <= 0.25 {
                try_place_mushroom_on_fallen_tree(context, log_pos, facing);
            }
        }
    }
}

fn try_place_mushroom_on_standing_tree<R: RandomSource>(context: &mut Context<impl WorldGenLevel, R>, log_pos: BlockPos, facing: Direction) -> bool {
    let mushroom_pos = mushroom_pos_for(log_pos, facing);
    if !is_block_replaceable_with_shelf_mushroom(context, mushroom_pos) {
        return false;
    }
    if has_shelf_mushroom_at(context, mushroom_pos.below()) {
        return false;
    }
    place_mushroom(context, mushroom_pos, facing);
    true
}

fn try_place_mushroom_on_fallen_tree<R: RandomSource>(context: &mut Context<impl WorldGenLevel, R>, log_pos: BlockPos, facing: Direction) {
    let mushroom_pos = mushroom_pos_for(log_pos, facing);
    if is_block_replaceable_with_shelf_mushroom(context, mushroom_pos) && !has_horizontally_adjacent_shelf_mushroom(context, mushroom_pos) && !has_horizontally_adjacent_shelf_mushroom(context, log_pos) {
        place_mushroom(context, mushroom_pos, facing);
    }
}

fn is_fallen_log(logs: &[BlockPos]) -> bool {
    logs[0].y == logs[logs.len() - 1].y
}

fn pick_two_perpendicular_directions(random: &mut impl RandomSource) -> [Direction; 2] {
    let first = Direction::get_random_horizontal_direction(random);
    [first, first.clockwise()]
}

fn perpendicular_to_fallen_log(logs: &[BlockPos]) -> [Direction; 2] {
    let first = logs[0];
    let last = logs[logs.len() - 1];
    let log_axis = if first.x != last.x { Axis::X } else { Axis::Z };
    if log_axis == Axis::X { [Direction::North, Direction::South] } else { [Direction::East, Direction::West] }
}

fn is_within_decoratable_height(pos: BlockPos, tree_base_y: i32) -> bool {
    let dy = pos.y - tree_base_y;
    (1..=4).contains(&dy)
}

fn mushroom_pos_for(log_pos: BlockPos, facing: Direction) -> BlockPos {
    log_pos.offset(facing.step_x(), 0, facing.step_z())
}

fn place_mushroom<R: RandomSource>(context: &mut Context<impl WorldGenLevel, R>, pos: BlockPos, facing: Direction) {
    let age = context.random.next_int_bounded(2);
    context.set_block(pos, blocks::SHELF_MUSHROOM.default_state().set(AGE_1, age).set(HORIZONTAL_FACING, facing));
}

fn has_shelf_mushroom_at<R: RandomSource>(context: &mut Context<impl WorldGenLevel, R>, pos: BlockPos) -> bool {
    context.check_block(pos, |state| state.is_block(blocks::SHELF_MUSHROOM))
}

fn has_horizontally_adjacent_shelf_mushroom<R: RandomSource>(context: &mut Context<impl WorldGenLevel, R>, pos: BlockPos) -> bool {
    Direction::PLANE_HORIZONTAL.into_iter().any(|dir| has_shelf_mushroom_at(context, pos.relative(dir)))
}

pub fn is_block_replaceable_with_shelf_mushroom<R: RandomSource>(context: &mut Context<impl WorldGenLevel, R>, pos: BlockPos) -> bool {
    context.is_replaceable(pos) && !context.is_water_or_water_nearby(pos)
}
