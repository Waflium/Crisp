use crisp_core::BlockPos;
use crisp_core::random::RandomSource;
use crisp_data::{BlockState, block_tags};

use crate::server::level::ServerLevel;
use crate::world::level::BlockGetter;
use crate::world::level::levelgen::feature::feature::{self, Feature};

pub struct TreeGrower {
    name: &'static str,
    trees: &'static [(&'static str, i32)],
    mega_trees: &'static [(&'static str, i32)],
    flower_trees: &'static [(&'static str, i32)],
    shortest_tree_type: Option<&'static str>,
}

pub const OAK: TreeGrower = TreeGrower {
    name: "oak",
    trees: &[("oak", 9), ("fancy_oak", 1)],
    mega_trees: &[],
    flower_trees: &[("oak_bees_005", 9), ("fancy_oak_bees_005", 1)],
    shortest_tree_type: Some("oak"),
};
pub const SPRUCE: TreeGrower = TreeGrower {
    name: "spruce",
    trees: &[("spruce", 1)],
    mega_trees: &[("mega_spruce", 1), ("mega_pine", 1)],
    flower_trees: &[],
    shortest_tree_type: Some("spruce"),
};
pub const MANGROVE: TreeGrower = TreeGrower {
    name: "mangrove",
    trees: &[("mangrove", 15), ("tall_mangrove", 85)],
    mega_trees: &[],
    flower_trees: &[],
    shortest_tree_type: Some("mangrove"),
};
pub const AZALEA: TreeGrower =
    TreeGrower { name: "azalea", trees: &[("azalea_tree", 1)], mega_trees: &[], flower_trees: &[], shortest_tree_type: Some("azalea_tree") };
pub const BIRCH: TreeGrower =
    TreeGrower { name: "birch", trees: &[("birch", 1)], mega_trees: &[], flower_trees: &[("birch_bees_005", 1)], shortest_tree_type: Some("birch") };
pub const JUNGLE: TreeGrower = TreeGrower {
    name: "jungle",
    trees: &[("jungle_tree_no_vine", 1)],
    mega_trees: &[("mega_jungle_tree", 1)],
    flower_trees: &[],
    shortest_tree_type: Some("jungle_tree_no_vine"),
};
pub const ACACIA: TreeGrower = TreeGrower { name: "acacia", trees: &[("acacia", 1)], mega_trees: &[], flower_trees: &[], shortest_tree_type: Some("acacia") };
pub const CHERRY: TreeGrower =
    TreeGrower { name: "cherry", trees: &[("cherry", 1)], mega_trees: &[], flower_trees: &[("cherry_bees_005", 1)], shortest_tree_type: Some("cherry") };
pub const DARK_OAK: TreeGrower = TreeGrower { name: "dark_oak", trees: &[], mega_trees: &[("dark_oak", 1)], flower_trees: &[], shortest_tree_type: None };
pub const PALE_OAK: TreeGrower = TreeGrower { name: "pale_oak", trees: &[], mega_trees: &[("pale_oak_bonemeal", 1)], flower_trees: &[], shortest_tree_type: None };
pub const POPLAR: TreeGrower = TreeGrower {
    name: "poplar",
    trees: &[("red_poplar", 1), ("orange_poplar", 1), ("yellow_poplar", 1)],
    mega_trees: &[],
    flower_trees: &[],
    shortest_tree_type: Some("red_poplar"),
};

const GROWERS: [&TreeGrower; 11] = [&OAK, &SPRUCE, &MANGROVE, &AZALEA, &BIRCH, &JUNGLE, &ACACIA, &CHERRY, &DARK_OAK, &PALE_OAK, &POPLAR];

struct TwoByTwoSaplingPos {
    offset_x: i32,
    offset_z: i32,
    ground_level_surrounding_blocks: [(BlockState, BlockPos); 4],
}

fn get_random(list: &'static [(&'static str, i32)], random: &mut impl RandomSource) -> Option<&'static str> {
    let total_weight: i32 = list.iter().map(|(_, weight)| weight).sum();
    if total_weight == 0 {
        return None;
    }
    let mut selection = random.next_int_bounded(total_weight);
    for &(item, weight) in list {
        if selection < weight {
            return Some(item);
        }
        selection -= weight;
    }
    unreachable!("selection within the total weight")
}

impl TreeGrower {
    pub fn by_name(name: &str) -> Option<&'static TreeGrower> {
        GROWERS.into_iter().find(|grower| grower.name == name)
    }

    fn get_configured_feature(&self, random: &mut impl RandomSource, has_flowers: bool) -> Option<&'static str> {
        if has_flowers && !self.flower_trees.is_empty() { get_random(self.flower_trees, random) } else { get_random(self.trees, random) }
    }

    fn get_configured_mega_feature(&self, random: &mut impl RandomSource) -> Option<&'static str> {
        get_random(self.mega_trees, random)
    }

    pub fn grow_tree(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState) -> bool {
        if let Some(mega_feature_key) = self.get_configured_mega_feature(&mut level.random)
            && let Some(feature) = feature::feature(mega_feature_key)
            && let Some(two_by_two_sapling_pos) = find_two_by_two_sapling_pos(level, state, pos)
        {
            let (dx, dz) = (two_by_two_sapling_pos.offset_x, two_by_two_sapling_pos.offset_z);
            let ground_level_surrounding_blocks = two_by_two_sapling_pos.ground_level_surrounding_blocks;
            remove_saplings(level, &ground_level_surrounding_blocks);
            if level.with_level_random(|level, random| feature.place(level, random, pos.offset(dx, 0, dz))) {
                return true;
            }
            reset_saplings(level, &ground_level_surrounding_blocks);
            return false;
        }
        let has_flowers = has_flowers(level, pos);
        let Some(feature_key) = self.get_configured_feature(&mut level.random, has_flowers) else { return false };
        let Some(feature) = feature::feature(feature_key) else { return false };
        remove_sapling(level, pos);
        if level.with_level_random(|level, random| feature.place(level, random, pos)) {
            return true;
        }
        reset_saplings(level, &[(state, pos)]);
        false
    }

    pub fn get_minimum_height(&self) -> Option<i32> {
        match feature::feature(self.shortest_tree_type?)? {
            Feature::Tree(tree_feature) => Some(tree_feature.trunk_placer.base_height),
            _ => None,
        }
    }

    pub fn can_grow(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState) -> bool {
        let has_flowers = has_flowers(level, pos);
        let feature_key = self.get_configured_feature(&mut level.random, has_flowers);
        let mega_feature_key = self.get_configured_mega_feature(&mut level.random);
        if feature_key.is_none() && mega_feature_key.is_some() { find_two_by_two_sapling_pos(level, state, pos).is_some() } else { true }
    }
}

fn get_surrounding_block_states(level: &mut ServerLevel, pos: BlockPos, dx: i32, dz: i32) -> [(BlockState, BlockPos); 4] {
    [pos.offset(dx, 0, dz), pos.offset(dx + 1, 0, dz), pos.offset(dx, 0, dz + 1), pos.offset(dx + 1, 0, dz + 1)].map(|p| (level.block_state(p), p))
}

fn remove_saplings(level: &mut ServerLevel, sapling_blocks: &[(BlockState, BlockPos)]) {
    for &(_, sapling_position) in sapling_blocks {
        remove_sapling(level, sapling_position);
    }
}

fn remove_sapling(level: &mut ServerLevel, sapling_position: BlockPos) {
    let empty_block = crate::server::level::legacy_block(level.fluid_state(sapling_position));
    level.set_block(sapling_position, empty_block, 818);
}

fn reset_saplings(level: &mut ServerLevel, sapling_blocks: &[(BlockState, BlockPos)]) {
    for &(state, pos) in sapling_blocks {
        level.set_block(pos, state, 260);
    }
}

fn is_two_by_two_sapling(state: BlockState, surrounding_blocks: &[(BlockState, BlockPos); 4]) -> bool {
    let block = state.block();
    surrounding_blocks.iter().all(|(surrounding_block_state, _)| surrounding_block_state.is_block(block))
}

fn has_flowers(level: &mut ServerLevel, pos: BlockPos) -> bool {
    BlockPos::between_closed(pos.offset(-2, -1, -2), pos.offset(2, 1, 2)).any(|p| level.block_state(p).is(&block_tags::FLOWERS))
}

fn find_two_by_two_sapling_pos(level: &mut ServerLevel, state: BlockState, pos: BlockPos) -> Option<TwoByTwoSaplingPos> {
    for dx in (-1..=0).rev() {
        for dz in (-1..=0).rev() {
            let ground_level_surrounding_blocks = get_surrounding_block_states(level, pos, dx, dz);
            if is_two_by_two_sapling(state, &ground_level_surrounding_blocks) {
                return Some(TwoByTwoSaplingPos { offset_x: dx, offset_z: dz, ground_level_surrounding_blocks });
            }
        }
    }
    None
}
