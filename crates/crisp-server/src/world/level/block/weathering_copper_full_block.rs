use crisp_core::BlockPos;
use crisp_data::BlockState;

use super::BlockBehaviour;
use super::weathering_copper::change_over_time;
use crate::server::level::ServerLevel;

pub struct WeatheringCopperFullBlock;

impl BlockBehaviour for WeatheringCopperFullBlock {
    fn random_tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        change_over_time(state, level, pos);
    }
}
