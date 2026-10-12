use crisp_core::BlockPos;
use crisp_data::{Block, BlockState};

use super::BlockBehaviour;
use super::vegetation_block::fields;
use crate::server::level::ServerLevel;
use crate::world::level::BlockGetter;
use crate::world::level::levelgen::feature::feature::{self, Feature};

pub struct BonemealableFeaturePlacerBlock {
    feature: Option<&'static Feature>,
}

impl BonemealableFeaturePlacerBlock {
    pub fn new(block: Block) -> BonemealableFeaturePlacerBlock {
        let fields = fields(block);
        BonemealableFeaturePlacerBlock { feature: feature::feature(fields["feature"].as_str().expect("a feature to place")) }
    }
}

impl BlockBehaviour for BonemealableFeaturePlacerBlock {
    fn is_valid_bonemeal_target(&self, level: &mut ServerLevel, pos: BlockPos, _state: BlockState) -> bool {
        level.block_state(pos.above()).is_air()
    }

    fn is_bonemeal_success(&self, _level: &mut ServerLevel, _pos: BlockPos, _state: BlockState) -> bool {
        true
    }

    fn perform_bonemeal(&self, level: &mut ServerLevel, pos: BlockPos, _state: BlockState) {
        if let Some(moss_patch) = self.feature {
            level.with_level_random(|level, random| moss_patch.place(level, random, pos.above()));
        }
    }
}
