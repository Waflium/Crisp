use crisp_core::BlockPos;
use crisp_core::random::RandomSource;
use crisp_data::properties::LEVEL_CAULDRON;
use crisp_data::{Block, BlockState, blocks};

use super::{BlockBehaviour, vegetation_block::fields};
use crate::server::level::ServerLevel;
use crate::world::level::biome::Precipitation;
use crate::world::level::update_flags::UPDATE_ALL;

fn should_handle_precipitation(level: &mut ServerLevel, precipitation: Precipitation) -> bool {
    match precipitation {
        Precipitation::Rain => level.random.next_float() < 0.05,
        Precipitation::Snow => level.random.next_float() < 0.1,
        Precipitation::None => false,
    }
}

pub struct CauldronBlock;

impl BlockBehaviour for CauldronBlock {
    fn handle_precipitation(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos, precipitation: Precipitation) {
        if should_handle_precipitation(level, precipitation) {
            match precipitation {
                Precipitation::Rain => drop(level.set_block(pos, blocks::WATER_CAULDRON.default_state(), UPDATE_ALL)),
                Precipitation::Snow => drop(level.set_block(pos, blocks::POWDER_SNOW_CAULDRON.default_state(), UPDATE_ALL)),
                Precipitation::None => {}
            }
        }
    }
}

pub struct LayeredCauldronBlock {
    precipitation_type: Precipitation,
}

impl LayeredCauldronBlock {
    pub fn new(block: Block) -> LayeredCauldronBlock {
        let precipitation_type = match fields(block)["precipitationType"].as_str() {
            Some("rain") => Precipitation::Rain,
            Some("snow") => Precipitation::Snow,
            _ => Precipitation::None,
        };
        LayeredCauldronBlock { precipitation_type }
    }
}

impl BlockBehaviour for LayeredCauldronBlock {
    fn handle_precipitation(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, precipitation: Precipitation) {
        if should_handle_precipitation(level, precipitation) && state.get(LEVEL_CAULDRON) != 3 && precipitation == self.precipitation_type {
            level.set_block(pos, state.cycle(LEVEL_CAULDRON), UPDATE_ALL);
        }
    }
}
