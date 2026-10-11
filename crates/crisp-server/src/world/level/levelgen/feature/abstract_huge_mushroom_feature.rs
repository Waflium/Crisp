use crisp_core::BlockPos;
use crisp_core::random::RandomSource;
use crisp_data::{BlockState, block_tags};
use serde_json::Value;

use super::stateproviders::block_state_provider::BlockStateProviderHolder;
use super::{huge_brown_mushroom_feature, huge_red_mushroom_feature};
use crate::server::level::world_gen_region::WorldGenRegion;
use crate::world::level::levelgen::blockpredicates::block_predicate::BlockPredicate;

pub enum HugeMushroomType {
    Brown,
    Red,
}

pub struct AbstractHugeMushroomFeature {
    pub cap_provider: BlockStateProviderHolder,
    stem_provider: BlockStateProviderHolder,
    pub foliage_radius: i32,
    can_place_on: BlockPredicate,
    kind: HugeMushroomType,
}

impl AbstractHugeMushroomFeature {
    pub fn parse(json: &Value, kind: HugeMushroomType) -> Result<AbstractHugeMushroomFeature, String> {
        let field = |key: &str| json.get(key).ok_or_else(|| format!("missing {key}"));
        Ok(AbstractHugeMushroomFeature {
            cap_provider: BlockStateProviderHolder::parse(field("cap_provider")?)?,
            stem_provider: BlockStateProviderHolder::parse(field("stem_provider")?)?,
            foliage_radius: json.get("foliage_radius").and_then(Value::as_i64).map_or(2, |v| v as i32),
            can_place_on: BlockPredicate::parse(field("can_place_on")?)?,
            kind,
        })
    }

    fn place_trunk<R: RandomSource>(&self, level: &mut WorldGenRegion, random: &mut R, origin: BlockPos, tree_height: i32) {
        for dy in 0..tree_height {
            let block_pos = origin.offset(0, dy, 0);
            let state = self.stem_provider.value().get_state(level, random, origin);
            place_mushroom_block(level, block_pos, state);
        }
    }

    fn get_tree_height(random: &mut impl RandomSource) -> i32 {
        let mut tree_height = random.next_int_bounded(3) + 4;
        if random.next_int_bounded(12) == 0 {
            tree_height *= 2;
        }
        tree_height
    }

    fn is_valid_position(&self, level: &mut WorldGenRegion, origin: BlockPos, tree_height: i32) -> bool {
        let y = origin.y;
        if y < level.get_min_y() + 1 || y + tree_height + 1 > level.get_max_y() {
            return false;
        }
        if !self.can_place_on.test(level, origin.below()) {
            return false;
        }
        for dy in 0..=tree_height {
            let radius = self.get_tree_radius_for_height(-1, -1, self.foliage_radius, dy);
            for dx in -radius..=radius {
                for dz in -radius..=radius {
                    let state = level.get_block_state(origin.offset(dx, dy, dz));
                    if !state.is_air() && !state.is(&block_tags::LEAVES) {
                        return false;
                    }
                }
            }
        }
        true
    }

    pub fn place<R: RandomSource>(&self, level: &mut WorldGenRegion, random: &mut R, origin: BlockPos) -> bool {
        let tree_height = Self::get_tree_height(random);
        if !self.is_valid_position(level, origin, tree_height) {
            return false;
        }
        match self.kind {
            HugeMushroomType::Brown => huge_brown_mushroom_feature::make_cap(self, level, random, origin, tree_height),
            HugeMushroomType::Red => huge_red_mushroom_feature::make_cap(self, level, random, origin, tree_height),
        }
        self.place_trunk(level, random, origin, tree_height);
        true
    }

    fn get_tree_radius_for_height(&self, trunk_height: i32, tree_height: i32, leaf_radius: i32, yo: i32) -> i32 {
        match self.kind {
            HugeMushroomType::Brown => huge_brown_mushroom_feature::get_tree_radius_for_height(leaf_radius, yo),
            HugeMushroomType::Red => huge_red_mushroom_feature::get_tree_radius_for_height(trunk_height, tree_height, leaf_radius, yo),
        }
    }
}

pub fn place_mushroom_block(level: &mut WorldGenRegion, block_pos: BlockPos, new_state: BlockState) {
    let current_state = level.get_block_state(block_pos);
    if current_state.is_air() || current_state.is(&block_tags::REPLACEABLE_BY_MUSHROOMS) {
        level.set_block_and_update(block_pos, new_state);
    }
}
