use std::sync::LazyLock;

use crisp_core::random::RandomSource;
use crisp_core::BlockPos;
use crisp_data::{Block, BlockState};

use crate::server::level::ServerLevel;
use crate::world::level::BlockGetter;
use crate::world::level::update_flags::UPDATE_ALL;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum WeatherState {
    Unaffected,
    Exposed,
    Weathered,
    Oxidized,
}

impl WeatherState {
    fn by_name(name: &str) -> Option<WeatherState> {
        Some(match name {
            "unaffected" => WeatherState::Unaffected,
            "exposed" => WeatherState::Exposed,
            "weathered" => WeatherState::Weathered,
            "oxidized" => WeatherState::Oxidized,
            _ => return None,
        })
    }
}

static AGES: LazyLock<Vec<Option<WeatherState>>> = LazyLock::new(|| {
    (0..Block::count())
        .map(|id| {
            let fields: serde_json::Value = serde_json::from_str(Block(id as u16).info().fields).unwrap_or_default();
            fields["weatherState"].as_str().and_then(WeatherState::by_name)
        })
        .collect()
});

pub fn age(block: Block) -> Option<WeatherState> {
    AGES[block.0 as usize]
}

fn next_block(block: Block) -> Option<Block> {
    let age = age(block)?;
    let next = Block(block.0 + 1);
    (age != WeatherState::Oxidized && self::age(next) > Some(age)).then_some(next)
}

fn previous_block(block: Block) -> Option<Block> {
    let age = age(block)?;
    let previous = Block(block.0.checked_sub(1)?);
    (age != WeatherState::Unaffected && next_block(previous) == Some(block)).then_some(previous)
}

pub fn get_previous(state: BlockState) -> Option<BlockState> {
    previous_block(state.block()).map(|previous| with_properties_of(previous, state))
}

pub fn get_first(state: BlockState) -> BlockState {
    let mut candidate = state.block();
    while let Some(previous) = previous_block(candidate) {
        candidate = previous;
    }
    with_properties_of(candidate, state)
}

pub fn with_properties_of(block: Block, state: BlockState) -> BlockState {
    state.properties().fold(block.default_state(), |result, (name, value)| result.with_property_by_name(name, value).unwrap_or(result))
}

pub fn change_over_time(state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
    if level.random.next_float() < 0.05688889
        && let Some(weathered) = next_state(state, level, pos)
    {
        level.set_block(pos, weathered, UPDATE_ALL);
    }
}

fn next_state(state: BlockState, level: &mut ServerLevel, pos: BlockPos) -> Option<BlockState> {
    let own_age = age(state.block())?;
    let mut same_age_count = 0;
    let mut older_count = 0;
    for neighbour in within_manhattan(pos, 4) {
        if neighbour == pos {
            continue;
        }
        let Some(found_age) = age(level.block_state(neighbour).block()) else { continue };
        if found_age < own_age {
            return None;
        }
        if found_age > own_age {
            older_count += 1;
        } else {
            same_age_count += 1;
        }
    }
    let chance = (older_count + 1) as f32 / (older_count + same_age_count + 1) as f32;
    let chance_modifier = if own_age == WeatherState::Unaffected { 0.75 } else { 1.0 };
    let actual_chance = chance * chance * chance_modifier;
    if level.random.next_float() < actual_chance { next_block(state.block()).map(|next| with_properties_of(next, state)) } else { None }
}

fn within_manhattan(origin: BlockPos, reach: i32) -> impl Iterator<Item = BlockPos> {
    (0..=reach).flat_map(move |depth| {
        (-depth..=depth).flat_map(move |x| {
            let max_y = depth - x.abs();
            (-max_y..=max_y).flat_map(move |y| {
                let z = depth - x.abs() - y.abs();
                let first = BlockPos::new(origin.x + x, origin.y + y, origin.z + z);
                let mirrored = (z != 0).then(|| BlockPos::new(origin.x + x, origin.y + y, origin.z - z));
                std::iter::once(first).chain(mirrored)
            })
        })
    })
}

#[cfg(test)]
mod tests {
    use crisp_core::BlockPos;
    use crisp_data::blocks;
    use crisp_data::properties::{HALF, HORIZONTAL_FACING, Half};

    use super::*;
    use crate::server::level::testing;
    use crate::world::level::update_flags::UPDATE_ALL;

    #[test]
    fn ages_follow_registration_order() {
        assert_eq!(age(blocks::COPPER_DOOR), Some(WeatherState::Unaffected));
        assert_eq!(next_block(blocks::COPPER_DOOR), Some(blocks::EXPOSED_COPPER_DOOR));
        assert_eq!(next_block(blocks::OXIDIZED_COPPER_DOOR), None);
        assert_eq!(next_block(blocks::COPPER_BLOCK), Some(blocks::EXPOSED_COPPER));
        assert_eq!(next_block(blocks::WAXED_COPPER_DOOR), None);
        assert_eq!(age(blocks::IRON_DOOR), None);
    }

    #[test]
    fn manhattan_order_and_count() {
        let all: Vec<BlockPos> = within_manhattan(BlockPos::ZERO, 4).collect();
        assert_eq!(all.len(), 129);
        assert_eq!(&all[..3], &[BlockPos::new(0, 0, 0), BlockPos::new(-1, 0, 0), BlockPos::new(0, -1, 0)]);
        assert_eq!(&all[3..5], &[BlockPos::new(0, 0, 1), BlockPos::new(0, 0, -1)]);
    }

    #[test]
    fn younger_copper_holds_back_ageing() {
        let mut level = testing::level();
        let pos = BlockPos::new(0, -60, 0);
        let stairs = blocks::EXPOSED_CUT_COPPER_STAIRS.default_state().set(HORIZONTAL_FACING, crisp_core::Direction::East).set(HALF, Half::Top);
        level.set_block(pos, stairs, UPDATE_ALL);
        level.set_block(pos.offset(2, 0, 0), blocks::COPPER_BLOCK.default_state(), UPDATE_ALL);
        assert_eq!(next_state(stairs, &mut level, pos), None);
        level.set_block(pos.offset(2, 0, 0), blocks::OXIDIZED_COPPER.default_state(), UPDATE_ALL);
        let next = next_state(stairs, &mut level, pos).expect("ages");
        assert_eq!(next, blocks::WEATHERED_CUT_COPPER_STAIRS.default_state().set(HORIZONTAL_FACING, crisp_core::Direction::East).set(HALF, Half::Top));
    }
}
