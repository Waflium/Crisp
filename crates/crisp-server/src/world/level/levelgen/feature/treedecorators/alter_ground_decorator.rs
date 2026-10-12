use crate::world::level::world_gen_level::WorldGenLevel;
use crisp_core::BlockPos;
use crisp_core::random::RandomSource;
use serde_json::Value;

use super::tree_decorator::Context;
use crate::world::level::levelgen::feature::stateproviders::block_state_provider::BlockStateProviderHolder;
use crate::world::level::levelgen::feature::tree_feature;

pub struct AlterGroundDecorator {
    provider: BlockStateProviderHolder,
}

impl AlterGroundDecorator {
    pub fn parse(json: &Value) -> Result<AlterGroundDecorator, String> {
        Ok(AlterGroundDecorator { provider: BlockStateProviderHolder::parse(json.get("provider").ok_or("missing provider")?)? })
    }

    pub fn place<R: RandomSource>(&self, context: &mut Context<impl WorldGenLevel, R>) {
        let block_positions = tree_feature::get_lowest_trunk_or_root_of_tree(&context.roots, &context.logs);
        if block_positions.is_empty() {
            return;
        }
        let min_y = block_positions[0].y;
        for &pos in block_positions.iter().filter(|pos| pos.y == min_y) {
            self.place_circle(context, pos.west().north());
            self.place_circle(context, pos.offset(2, 0, 0).north());
            self.place_circle(context, pos.west().offset(0, 0, 2));
            self.place_circle(context, pos.offset(2, 0, 2));
            for _ in 0..5 {
                let placement = context.random.next_int_bounded(64);
                let xx = placement % 8;
                let zz = placement / 8;
                if xx == 0 || xx == 7 || zz == 0 || zz == 7 {
                    self.place_circle(context, pos.offset(-3 + xx, 0, -3 + zz));
                }
            }
        }
    }

    fn place_circle<R: RandomSource>(&self, context: &mut Context<impl WorldGenLevel, R>, pos: BlockPos) {
        for xx in -2i32..=2 {
            for zz in -2i32..=2 {
                if xx.abs() != 2 || zz.abs() != 2 {
                    self.place_block_at(context, pos.offset(xx, 0, zz));
                }
            }
        }
    }

    fn place_block_at<R: RandomSource>(&self, context: &mut Context<impl WorldGenLevel, R>, pos: BlockPos) {
        let mut dy = 2;
        while dy >= -3 {
            let cursor = pos.offset(0, dy, 0);
            let replace_with = self.provider.value().get_optional_state(context.level, context.random, cursor);
            if let Some(replace_with) = replace_with {
                context.set_block(cursor, replace_with);
                break;
            }
            if !context.is_air(cursor) && dy < 0 {
                break;
            }
            dy -= 1;
        }
    }
}
