use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, Direction};
use serde_json::Value;

use super::stateproviders::block_state_provider::BlockStateProviderHolder;
use crate::server::level::world_gen_region::WorldGenRegion;
use crate::util::valueproviders::IntProvider;
use crate::world::level::levelgen::blockpredicates::block_predicate::BlockPredicate;

pub struct Layer {
    height: IntProvider,
    state: BlockStateProviderHolder,
}

pub struct BlockColumnFeature {
    layers: Vec<Layer>,
    direction: Direction,
    allowed_placement: BlockPredicate,
    prioritize_tip: bool,
}

impl BlockColumnFeature {
    pub fn parse(json: &Value) -> Result<BlockColumnFeature, String> {
        let field = |key: &str| json.get(key).ok_or_else(|| format!("missing {key}"));
        Ok(BlockColumnFeature {
            layers: field("layers")?
                .as_array()
                .ok_or("layers is a list")?
                .iter()
                .map(|layer| {
                    Ok(Layer {
                        height: IntProvider::parse(layer.get("height").ok_or("missing height")?)?,
                        state: BlockStateProviderHolder::parse(layer.get("provider").ok_or("missing provider")?)?,
                    })
                })
                .collect::<Result<_, String>>()?,
            direction: field("direction")?.as_str().and_then(Direction::by_name).ok_or("bad direction")?,
            allowed_placement: BlockPredicate::parse(field("allowed_placement")?)?,
            prioritize_tip: field("prioritize_tip")?.as_bool().ok_or("bad prioritize_tip")?,
        })
    }

    pub fn place<R: RandomSource>(&self, level: &mut WorldGenRegion, random: &mut R, origin: BlockPos) -> bool {
        let mut layer_heights: Vec<i32> = Vec::with_capacity(self.layers.len());
        let mut total_height = 0;
        for layer in &self.layers {
            let height = layer.height.sample(random);
            layer_heights.push(height);
            total_height += height;
        }
        if total_height == 0 {
            return false;
        }
        let mut place_pos = origin;
        let mut next_pos = place_pos.relative(self.direction);
        for y in 0..total_height {
            if !self.allowed_placement.test(level, next_pos) {
                truncate(&mut layer_heights, total_height, y, self.prioritize_tip);
                break;
            }
            next_pos = next_pos.relative(self.direction);
        }
        for (layer, &count) in self.layers.iter().zip(&layer_heights) {
            for _ in 0..count {
                let state = layer.state.value().get_state(level, random, place_pos);
                level.set_block(place_pos, state, 2);
                place_pos = place_pos.relative(self.direction);
            }
        }
        true
    }
}

fn truncate(layer_heights: &mut [i32], total_height: i32, new_height: i32, prioritize_tip: bool) {
    let mut amount_to_remove = total_height - new_height;
    let direction: isize = if prioritize_tip { 1 } else { -1 };
    let mut i: isize = if prioritize_tip { 0 } else { layer_heights.len() as isize - 1 };
    let end: isize = if prioritize_tip { layer_heights.len() as isize } else { -1 };
    while i != end && amount_to_remove > 0 {
        let this_layer = layer_heights[i as usize];
        let to_remove_from_layer = this_layer.min(amount_to_remove);
        amount_to_remove -= to_remove_from_layer;
        layer_heights[i as usize] -= to_remove_from_layer;
        i += direction;
    }
}
