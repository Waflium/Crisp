use crisp_core::BlockPos;
use crisp_core::random::RandomSource;
use crisp_data::{BlockState, block_tags};
use serde_json::Value;

use super::bending_trunk_placer::{self, BendingTrunkPlacer};
use super::cherry_trunk_placer::{self, CherryTrunkPlacer};
use super::poplar_trunk_placer::{self, PoplarTrunkPlacer};
use super::upwards_branching_trunk_placer::{self, UpwardsBranchingTrunkPlacer};
use super::{dark_oak_trunk_placer, fancy_trunk_placer, forking_trunk_placer, giant_trunk_placer, mega_jungle_trunk_placer, straight_trunk_placer};
use crate::server::level::world_gen_region::WorldGenRegion;
use crate::world::level::levelgen::feature::foliageplacers::foliage_placer::FoliageAttachment;
use crate::world::level::levelgen::feature::tree_feature::{self, TreeFeature, TreeSetter};

pub struct TrunkPlacer {
    pub base_height: i32,
    pub height_rand_a: i32,
    pub height_rand_b: i32,
    pub kind: TrunkPlacerType,
}

pub enum TrunkPlacerType {
    Straight,
    Forking,
    Giant,
    MegaJungle,
    DarkOak,
    Fancy,
    Bending(BendingTrunkPlacer),
    UpwardsBranching(UpwardsBranchingTrunkPlacer),
    Cherry(CherryTrunkPlacer),
    Poplar(PoplarTrunkPlacer),
}

impl TrunkPlacer {
    pub fn parse(json: &Value) -> Result<TrunkPlacer, String> {
        let ty = json.get("type").and_then(Value::as_str).ok_or("trunk placer without type")?;
        let int = |key: &str| json.get(key).and_then(Value::as_i64).map(|v| v as i32).ok_or_else(|| format!("missing {key}"));
        let kind = match ty.strip_prefix("minecraft:").unwrap_or(ty) {
            "straight_trunk_placer" => TrunkPlacerType::Straight,
            "forking_trunk_placer" => TrunkPlacerType::Forking,
            "giant_trunk_placer" => TrunkPlacerType::Giant,
            "mega_jungle_trunk_placer" => TrunkPlacerType::MegaJungle,
            "dark_oak_trunk_placer" => TrunkPlacerType::DarkOak,
            "fancy_trunk_placer" => TrunkPlacerType::Fancy,
            "bending_trunk_placer" => TrunkPlacerType::Bending(BendingTrunkPlacer::parse(json)?),
            "upwards_branching_trunk_placer" => TrunkPlacerType::UpwardsBranching(UpwardsBranchingTrunkPlacer::parse(json)?),
            "cherry_trunk_placer" => TrunkPlacerType::Cherry(CherryTrunkPlacer::parse(json)?),
            "poplar_trunk_placer" => TrunkPlacerType::Poplar(PoplarTrunkPlacer::parse(json)?),
            other => return Err(format!("unknown trunk placer type {other}")),
        };
        Ok(TrunkPlacer { base_height: int("base_height")?, height_rand_a: int("height_rand_a")?, height_rand_b: int("height_rand_b")?, kind })
    }

    pub fn place_trunk<R: RandomSource>(
        &self,
        level: &mut WorldGenRegion,
        trunk_setter: &mut TreeSetter,
        random: &mut R,
        tree_height: i32,
        origin: BlockPos,
        tree: &TreeFeature,
    ) -> Vec<FoliageAttachment> {
        match &self.kind {
            TrunkPlacerType::Straight => straight_trunk_placer::place_trunk(self, level, trunk_setter, random, tree_height, origin, tree),
            TrunkPlacerType::Forking => forking_trunk_placer::place_trunk(self, level, trunk_setter, random, tree_height, origin, tree),
            TrunkPlacerType::Giant => giant_trunk_placer::place_trunk(self, level, trunk_setter, random, tree_height, origin, tree),
            TrunkPlacerType::MegaJungle => mega_jungle_trunk_placer::place_trunk(self, level, trunk_setter, random, tree_height, origin, tree),
            TrunkPlacerType::DarkOak => dark_oak_trunk_placer::place_trunk(self, level, trunk_setter, random, tree_height, origin, tree),
            TrunkPlacerType::Fancy => fancy_trunk_placer::place_trunk(self, level, trunk_setter, random, tree_height, origin, tree),
            TrunkPlacerType::Bending(placer) => bending_trunk_placer::place_trunk(placer, self, level, trunk_setter, random, tree_height, origin, tree),
            TrunkPlacerType::UpwardsBranching(placer) => upwards_branching_trunk_placer::place_trunk(placer, self, level, trunk_setter, random, tree_height, origin, tree),
            TrunkPlacerType::Cherry(placer) => cherry_trunk_placer::place_trunk(placer, self, level, trunk_setter, random, tree_height, origin, tree),
            TrunkPlacerType::Poplar(placer) => poplar_trunk_placer::place_trunk(placer, self, level, trunk_setter, random, tree_height, origin, tree),
        }
    }

    pub fn get_tree_height(&self, random: &mut impl RandomSource) -> i32 {
        self.base_height + random.next_int_bounded(self.height_rand_a + 1) + random.next_int_bounded(self.height_rand_b + 1)
    }

    pub fn place_log<R: RandomSource>(&self, level: &mut WorldGenRegion, trunk_setter: &mut TreeSetter, random: &mut R, pos: BlockPos, tree: &TreeFeature) -> bool {
        self.place_log_with(level, trunk_setter, random, pos, tree, |state| state)
    }

    pub fn place_log_with<R: RandomSource>(
        &self,
        level: &mut WorldGenRegion,
        trunk_setter: &mut TreeSetter,
        random: &mut R,
        pos: BlockPos,
        tree: &TreeFeature,
        state_modifier: impl Fn(BlockState) -> BlockState,
    ) -> bool {
        if self.valid_tree_pos(level, pos) {
            let state = tree.trunk_provider.value().get_state(level, random, pos);
            trunk_setter.accept(level, pos, state_modifier(state));
            true
        } else {
            false
        }
    }

    pub fn place_log_if_free<R: RandomSource>(&self, level: &mut WorldGenRegion, trunk_setter: &mut TreeSetter, random: &mut R, pos: BlockPos, tree: &TreeFeature) {
        if self.is_free(level, pos) {
            self.place_log(level, trunk_setter, random, pos, tree);
        }
    }

    pub fn valid_tree_pos(&self, level: &mut WorldGenRegion, pos: BlockPos) -> bool {
        match &self.kind {
            TrunkPlacerType::UpwardsBranching(placer) => placer.valid_tree_pos(level, pos),
            _ => tree_feature::valid_tree_pos(level, pos),
        }
    }

    pub fn is_free(&self, level: &mut WorldGenRegion, pos: BlockPos) -> bool {
        self.valid_tree_pos(level, pos) || level.get_block_state(pos).is(&block_tags::LOGS)
    }
}

pub fn place_below_trunk_block<R: RandomSource>(level: &mut WorldGenRegion, trunk_setter: &mut TreeSetter, random: &mut R, pos: BlockPos, tree: &TreeFeature) {
    if let Some(block_below_trunk) = tree.below_trunk_provider.value().get_optional_state(level, random, pos) {
        trunk_setter.accept(level, pos, block_below_trunk);
    }
}
