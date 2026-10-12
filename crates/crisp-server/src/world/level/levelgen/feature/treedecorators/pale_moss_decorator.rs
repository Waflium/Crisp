use crate::world::level::world_gen_level::WorldGenLevel;
use crisp_core::BlockPos;
use crisp_core::random::RandomSource;
use crisp_data::blocks;
use crisp_data::properties::TIP;
use serde_json::Value;

use super::tree_decorator::Context;
use crate::util;
use crate::util::valueproviders::float;
use crate::world::level::levelgen::feature::feature::{FeatureHolder, feature_index};

pub struct PaleMossDecorator {
    leaves_probability: f32,
    trunk_probability: f32,
    ground_probability: f32,
}

impl PaleMossDecorator {
    pub fn parse(json: &Value) -> Result<PaleMossDecorator, String> {
        let field = |key: &str| json.get(key).ok_or_else(|| format!("missing {key}"));
        Ok(PaleMossDecorator {
            leaves_probability: float(field("leaves_probability")?)?,
            trunk_probability: float(field("trunk_probability")?)?,
            ground_probability: float(field("ground_probability")?)?,
        })
    }

    pub fn place<R: RandomSource>(&self, context: &mut Context<impl WorldGenLevel, R>) {
        let mut logs = context.logs.clone();
        util::shuffle(&mut logs, context.random);
        let Some(&origin) = logs.iter().min_by_key(|pos| pos.y) else { return };
        if context.random.next_float() < self.ground_probability
            && let Some(index) = feature_index("minecraft:pale_moss_patch")
        {
            FeatureHolder::Reference(index).value().place(context.level, context.random, origin.above());
        }
        for i in 0..context.logs.len() {
            let pos = context.logs[i];
            if context.random.next_float() < self.trunk_probability {
                let down = pos.below();
                if context.is_air(down) {
                    add_moss_hanger(down, context);
                }
            }
        }
        for i in 0..context.leaves.len() {
            let pos = context.leaves[i];
            if context.random.next_float() < self.leaves_probability {
                let down = pos.below();
                if context.is_air(down) {
                    add_moss_hanger(down, context);
                }
            }
        }
    }
}

fn add_moss_hanger<R: RandomSource>(mut pos: BlockPos, context: &mut Context<impl WorldGenLevel, R>) {
    while context.is_air(pos.below()) && context.random.next_float() as f64 >= 0.5 {
        context.set_block(pos, blocks::PALE_HANGING_MOSS.default_state().set(TIP, false));
        pos = pos.below();
    }
    context.set_block(pos, blocks::PALE_HANGING_MOSS.default_state().set(TIP, true));
}
