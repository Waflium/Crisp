use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, Direction};
use serde_json::Value;

use super::stateproviders::block_state_provider::BlockStateProviderHolder;
use crate::world::level::levelgen::blockpredicates::block_predicate::BlockPredicate;
use crate::world::level::levelgen::placement::placed_feature::PlacedFeatureHolder;
use crate::world::level::world_gen_level::WorldGenLevel;

pub struct SingleBlockPillarFeature {
    block: BlockStateProviderHolder,
    can_replace: BlockPredicate,
    direction: Direction,
    chance_to_continue: f32,
    cap_feature: Option<PlacedFeatureHolder>,
}

impl SingleBlockPillarFeature {
    pub fn parse(json: &Value) -> Result<SingleBlockPillarFeature, String> {
        Ok(SingleBlockPillarFeature {
            block: BlockStateProviderHolder::parse(json.get("block").ok_or("missing block")?)?,
            can_replace: json.get("can_replace").map(BlockPredicate::parse).transpose()?.unwrap_or(BlockPredicate::True),
            direction: json.get("direction").and_then(Value::as_str).and_then(Direction::by_name).ok_or("bad direction")?,
            chance_to_continue: json.get("chance_to_continue").and_then(Value::as_f64).map_or(1.0, |v| v as f32),
            cap_feature: json.get("cap_feature").map(PlacedFeatureHolder::parse).transpose()?,
        })
    }

    pub fn place<R: RandomSource>(&self, level: &mut impl WorldGenLevel, random: &mut R, origin: BlockPos) -> bool {
        let mut pos = origin;
        while self.can_replace.test(level, pos) && random.next_float() < self.chance_to_continue && !level.is_outside_build_height(pos.y) {
            let state = self.block.value().get_state(level, random, pos);
            level.set_block(pos, state, 2);
            pos = pos.relative(self.direction);
        }
        pos = pos.relative(self.direction.opposite());
        if let Some(cap_feature) = &self.cap_feature {
            cap_feature.value().place(level, random, pos);
        }
        true
    }
}
