use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, Direction};
use serde_json::Value;

use super::root_placer::{RootPlacer, get_potentially_waterlogged_state};
use crate::world::level::world_gen_level::WorldGenLevel;
use crate::util::valueproviders::float;
use crate::world::level::levelgen::blockpredicates::block_predicate::BlockSet;
use crate::world::level::levelgen::feature::stateproviders::block_state_provider::BlockStateProviderHolder;
use crate::world::level::levelgen::feature::tree_feature::{TreeFeature, TreeSetter};

pub struct MangroveRootPlacement {
    pub can_grow_through: BlockSet,
    muddy_roots_in: BlockSet,
    muddy_roots_provider: BlockStateProviderHolder,
    max_root_width: i32,
    max_root_length: i32,
    random_skew_chance: f32,
}

impl MangroveRootPlacement {
    pub fn parse(json: &Value) -> Result<MangroveRootPlacement, String> {
        let field = |key: &str| json.get(key).ok_or_else(|| format!("missing {key}"));
        let int = |key: &str| field(key)?.as_i64().map(|v| v as i32).ok_or_else(|| format!("bad {key}"));
        Ok(MangroveRootPlacement {
            can_grow_through: BlockSet::parse(field("can_grow_through")?)?,
            muddy_roots_in: BlockSet::parse(field("muddy_roots_in")?)?,
            muddy_roots_provider: BlockStateProviderHolder::parse(field("muddy_roots_provider")?)?,
            max_root_width: int("max_root_width")?,
            max_root_length: int("max_root_length")?,
            random_skew_chance: float(field("random_skew_chance")?)?,
        })
    }
}

#[allow(clippy::too_many_arguments)]
pub fn place_roots<R: RandomSource>(
    placement: &MangroveRootPlacement,
    placer: &RootPlacer,
    level: &mut impl WorldGenLevel,
    root_setter: &mut TreeSetter,
    random: &mut R,
    origin: BlockPos,
    trunk_origin: BlockPos,
    tree: &TreeFeature,
) -> bool {
    let mut root_positions = Vec::new();
    let mut column_pos = origin;
    while column_pos.y < trunk_origin.y {
        if !placer.can_place_root(level, column_pos) {
            return false;
        }
        column_pos = column_pos.above();
    }
    root_positions.push(trunk_origin.below());
    for dir in Direction::PLANE_HORIZONTAL {
        let pos = trunk_origin.relative(dir);
        let mut positions_in_direction = Vec::new();
        if !simulate_roots(placement, placer, level, random, pos, dir, trunk_origin, &mut positions_in_direction, 0) {
            return false;
        }
        root_positions.extend(positions_in_direction);
        root_positions.push(trunk_origin.relative(dir));
    }
    for root_pos in root_positions {
        placer.place_root(level, root_setter, random, root_pos, tree);
    }
    true
}

#[allow(clippy::too_many_arguments)]
fn simulate_roots<R: RandomSource>(
    placement: &MangroveRootPlacement,
    placer: &RootPlacer,
    level: &mut impl WorldGenLevel,
    random: &mut R,
    root_pos: BlockPos,
    dir: Direction,
    root_origin: BlockPos,
    root_positions: &mut Vec<BlockPos>,
    layer: i32,
) -> bool {
    let max_root_length = placement.max_root_length;
    if layer != max_root_length && root_positions.len() as i32 <= max_root_length {
        for pos in potential_root_positions(placement, root_pos, dir, random, root_origin) {
            if placer.can_place_root(level, pos) {
                root_positions.push(pos);
                if !simulate_roots(placement, placer, level, random, pos, dir, root_origin, root_positions, layer + 1) {
                    return false;
                }
            }
        }
        true
    } else {
        false
    }
}

fn potential_root_positions(placement: &MangroveRootPlacement, pos: BlockPos, prev_dir: Direction, random: &mut impl RandomSource, root_origin: BlockPos) -> Vec<BlockPos> {
    let below = pos.below();
    let next_to = pos.relative(prev_dir);
    let width = pos.dist_manhattan(root_origin);
    let max_root_width = placement.max_root_width;
    let random_skew_chance = placement.random_skew_chance;
    if width > max_root_width - 3 && width <= max_root_width {
        if random.next_float() < random_skew_chance { vec![below, next_to.below()] } else { vec![below] }
    } else if width > max_root_width {
        vec![below]
    } else if random.next_float() < random_skew_chance {
        vec![below]
    } else if random.next_boolean() {
        vec![next_to]
    } else {
        vec![below]
    }
}

pub fn place_root<R: RandomSource>(
    placement: &MangroveRootPlacement,
    placer: &RootPlacer,
    level: &mut impl WorldGenLevel,
    root_setter: &mut TreeSetter,
    random: &mut R,
    pos: BlockPos,
    _tree: &TreeFeature,
) {
    if placement.muddy_roots_in.contains(level.get_block_state(pos).block()) {
        let muddy_roots = placement.muddy_roots_provider.value().get_state(level, random, pos);
        let state = get_potentially_waterlogged_state(level, pos, muddy_roots);
        root_setter.accept(level, pos, state);
    } else {
        placer.root_placer_place_root(level, root_setter, random, pos);
    }
}
