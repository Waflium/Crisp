use crisp_core::BlockPos;
use crisp_core::random::RandomSource;
use crisp_data::{BlockState, block_tags, blocks};

use super::BlockBehaviour;
use crate::server::level::ServerLevel;
use crate::world::level::BlockGetter;
use crate::world::level::update_flags::UPDATE_ALL;

pub struct NetherrackBlock;

impl BlockBehaviour for NetherrackBlock {
    fn is_valid_bonemeal_target(&self, level: &mut ServerLevel, pos: BlockPos, _state: BlockState) -> bool {
        if !level.block_state(pos.above()).propagates_skylight_down() {
            return false;
        }
        BlockPos::between_closed(pos.offset(-1, -1, -1), pos.offset(1, 1, 1)).any(|p| level.block_state(p).is(&block_tags::NYLIUM))
    }

    fn is_bonemeal_success(&self, _level: &mut ServerLevel, _pos: BlockPos, _state: BlockState) -> bool {
        true
    }

    fn perform_bonemeal(&self, level: &mut ServerLevel, pos: BlockPos, _state: BlockState) {
        let (mut found_red, mut found_blue) = (false, false);
        for p in BlockPos::between_closed(pos.offset(-1, -1, -1), pos.offset(1, 1, 1)) {
            let block_state = level.block_state(p);
            if !block_state.is(&block_tags::NYLIUM) {
                continue;
            }
            if block_state.is_block(blocks::WARPED_NYLIUM) {
                found_blue = true;
            } else if block_state.is_block(blocks::CRIMSON_NYLIUM) {
                found_red = true;
            }
            if found_blue && found_red {
                break;
            }
        }
        if found_blue && found_red {
            let state = if level.random.next_boolean() { blocks::WARPED_NYLIUM.default_state() } else { blocks::CRIMSON_NYLIUM.default_state() };
            level.set_block(pos, state, UPDATE_ALL);
        } else if found_blue {
            level.set_block(pos, blocks::WARPED_NYLIUM.default_state(), UPDATE_ALL);
        } else if found_red {
            level.set_block(pos, blocks::CRIMSON_NYLIUM.default_state(), UPDATE_ALL);
        }
    }
}
