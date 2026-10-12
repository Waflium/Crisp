use crate::world::level::world_gen_level::WorldGenLevel;
use crisp_core::hash_map::FastSet;
use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, Direction};
use serde_json::Value;

use super::attached_to_logs_decorator::parse_directions;
use super::tree_decorator::Context;
use crate::util;
use crate::util::valueproviders::float;
use crate::world::level::levelgen::feature::stateproviders::block_state_provider::BlockStateProviderHolder;

pub struct AttachedToLeavesDecorator {
    probability: f32,
    exclusion_radius_xz: i32,
    exclusion_radius_y: i32,
    block_provider: BlockStateProviderHolder,
    required_empty_blocks: i32,
    directions: Vec<Direction>,
}

impl AttachedToLeavesDecorator {
    pub fn parse(json: &Value) -> Result<AttachedToLeavesDecorator, String> {
        let field = |key: &str| json.get(key).ok_or_else(|| format!("missing {key}"));
        let int = |key: &str| field(key)?.as_i64().map(|v| v as i32).ok_or_else(|| format!("bad {key}"));
        Ok(AttachedToLeavesDecorator {
            probability: float(field("probability")?)?,
            exclusion_radius_xz: int("exclusion_radius_xz")?,
            exclusion_radius_y: int("exclusion_radius_y")?,
            block_provider: BlockStateProviderHolder::parse(field("block_provider")?)?,
            required_empty_blocks: int("required_empty_blocks")?,
            directions: parse_directions(field("directions")?)?,
        })
    }

    pub fn place<R: RandomSource>(&self, context: &mut Context<impl WorldGenLevel, R>) {
        let mut propagule_blacklist = FastSet::default();
        let mut leaves = context.leaves.clone();
        util::shuffle(&mut leaves, context.random);
        for leaf_pos in leaves {
            let direction = self.directions[context.random.next_int_bounded(self.directions.len() as i32) as usize];
            let placement_pos = leaf_pos.relative(direction);
            if !propagule_blacklist.contains(&placement_pos) && context.random.next_float() < self.probability && self.has_required_empty_blocks(context, leaf_pos, direction) {
                let corner1 = placement_pos.offset(-self.exclusion_radius_xz, -self.exclusion_radius_y, -self.exclusion_radius_xz);
                let corner2 = placement_pos.offset(self.exclusion_radius_xz, self.exclusion_radius_y, self.exclusion_radius_xz);
                for x in corner1.x..=corner2.x {
                    for y in corner1.y..=corner2.y {
                        for z in corner1.z..=corner2.z {
                            propagule_blacklist.insert(BlockPos::new(x, y, z));
                        }
                    }
                }
                let state = self.block_provider.value().get_state(context.level, context.random, placement_pos);
                context.set_block(placement_pos, state);
            }
        }
    }

    fn has_required_empty_blocks<R: RandomSource>(&self, context: &mut Context<impl WorldGenLevel, R>, leaf_pos: BlockPos, direction: Direction) -> bool {
        (1..=self.required_empty_blocks).all(|i| context.is_air(leaf_pos.relative_n(direction, i)))
    }
}
