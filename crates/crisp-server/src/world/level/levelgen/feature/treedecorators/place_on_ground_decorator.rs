use crate::world::level::world_gen_level::WorldGenLevel;
use crisp_core::BlockPos;
use crisp_core::random::RandomSource;
use crisp_data::blocks;
use serde_json::Value;

use super::tree_decorator::Context;
use crate::world::level::chunk::HeightmapType;
use crate::world::level::levelgen::feature::stateproviders::block_state_provider::BlockStateProviderHolder;
use crate::world::level::levelgen::feature::tree_feature;

pub struct PlaceOnGroundDecorator {
    tries: i32,
    radius: i32,
    height: i32,
    block_state_provider: BlockStateProviderHolder,
}

impl PlaceOnGroundDecorator {
    pub fn parse(json: &Value) -> Result<PlaceOnGroundDecorator, String> {
        let int = |key: &str, default: i32| json.get(key).and_then(Value::as_i64).map_or(default, |v| v as i32);
        Ok(PlaceOnGroundDecorator {
            tries: int("tries", 128),
            radius: int("radius", 2),
            height: int("height", 1),
            block_state_provider: BlockStateProviderHolder::parse(json.get("block_state_provider").ok_or("missing block_state_provider")?)?,
        })
    }

    pub fn place<R: RandomSource>(&self, context: &mut Context<impl WorldGenLevel, R>) {
        let block_positions = tree_feature::get_lowest_trunk_or_root_of_tree(&context.roots, &context.logs);
        if block_positions.is_empty() {
            return;
        }
        let origin = block_positions[0];
        let min_y = origin.y;
        let (mut min_x, mut max_x, mut min_z, mut max_z) = (origin.x, origin.x, origin.z, origin.z);
        for position in &block_positions {
            if position.y == min_y {
                min_x = min_x.min(position.x);
                max_x = max_x.max(position.x);
                min_z = min_z.min(position.z);
                max_z = max_z.max(position.z);
            }
        }
        let (bb_min_x, bb_max_x) = (min_x - self.radius, max_x + self.radius);
        let (bb_min_y, bb_max_y) = (min_y - self.height, min_y + self.height);
        let (bb_min_z, bb_max_z) = (min_z - self.radius, max_z + self.radius);
        for _ in 0..self.tries {
            let x = context.random.next_int_between_inclusive(bb_min_x, bb_max_x);
            let y = context.random.next_int_between_inclusive(bb_min_y, bb_max_y);
            let z = context.random.next_int_between_inclusive(bb_min_z, bb_max_z);
            self.attempt_to_place_block_above(context, BlockPos::new(x, y, z));
        }
    }

    fn attempt_to_place_block_above<R: RandomSource>(&self, context: &mut Context<impl WorldGenLevel, R>, pos: BlockPos) {
        let above_pos = pos.above();
        let above = context.level.get_block_state(above_pos);
        if (above.is_air() || above.is_block(blocks::VINE))
            && context.check_block(pos, |state| state.is_solid_render())
            && context.level.get_height(HeightmapType::MotionBlockingNoLeaves, pos.x, pos.z) <= above_pos.y
        {
            let state = self.block_state_provider.value().get_state(context.level, context.random, above_pos);
            context.set_block(above_pos, state);
        }
    }
}
