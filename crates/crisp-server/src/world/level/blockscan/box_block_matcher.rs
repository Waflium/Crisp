use crisp_core::BlockPos;
use crisp_data::BlockState;

use super::block_scan_utils::find_blocks;
use crate::server::level::ServerLevel;

pub struct BoxBlockMatcher<P: Fn(BlockState) -> bool> {
    from: BlockPos,
    to: BlockPos,
    state_predicate: P,
}

impl ServerLevel {
    pub fn find_blocks_in(&self, from: BlockPos, to: BlockPos) -> BoxBlockMatcher<fn(BlockState) -> bool> {
        BoxBlockMatcher { from, to, state_predicate: |_| true }
    }
}

impl<P: Fn(BlockState) -> bool> BoxBlockMatcher<P> {
    pub fn filter_state<Q: Fn(BlockState) -> bool>(self, predicate: Q) -> BoxBlockMatcher<impl Fn(BlockState) -> bool> {
        let state_predicate = self.state_predicate;
        BoxBlockMatcher { from: self.from, to: self.to, state_predicate: move |state| state_predicate(state) && predicate(state) }
    }

    pub fn for_each(&self, level: &mut ServerLevel, mut consumer: impl FnMut(&mut ServerLevel, BlockPos, BlockState)) {
        find_blocks(level, self.from, self.to, &self.state_predicate, &mut |level, pos, state| {
            consumer(level, pos, state);
            false
        });
    }
}
