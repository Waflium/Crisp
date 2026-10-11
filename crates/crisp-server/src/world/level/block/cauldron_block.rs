use crisp_core::BlockPos;
use crisp_core::random::RandomSource;
use crisp_data::properties::LEVEL_CAULDRON;
use crisp_data::{Block, BlockState, Fluid, blocks};

use super::{BlockBehaviour, abstract_cauldron_block, vegetation_block::fields};
use crate::server::level::ServerLevel;
use crate::server::level::events::{SOUND_DRIP_LAVA_INTO_CAULDRON, SOUND_DRIP_WATER_INTO_CAULDRON};
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

    fn tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        abstract_cauldron_block::tick(self, state, level, pos);
    }

    fn can_receive_stalactite_drip(&self, _fluid: Fluid) -> bool {
        true
    }

    fn receive_stalactite_drip(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos, fluid: Fluid) {
        if fluid == Fluid::Water {
            let new_state = blocks::WATER_CAULDRON.default_state();
            level.set_block(pos, new_state, UPDATE_ALL);
            level.level_event(None, SOUND_DRIP_WATER_INTO_CAULDRON, pos, 0);
        } else if fluid == Fluid::Lava {
            let new_state = blocks::LAVA_CAULDRON.default_state();
            level.set_block(pos, new_state, UPDATE_ALL);
            level.level_event(None, SOUND_DRIP_LAVA_INTO_CAULDRON, pos, 0);
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

    fn tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        abstract_cauldron_block::tick(self, state, level, pos);
    }

    fn can_receive_stalactite_drip(&self, fluid: Fluid) -> bool {
        fluid == Fluid::Water && self.precipitation_type == Precipitation::Rain
    }

    fn receive_stalactite_drip(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, _fluid: Fluid) {
        if state.get(LEVEL_CAULDRON) != 3 {
            let new_state = state.set(LEVEL_CAULDRON, state.get(LEVEL_CAULDRON) + 1);
            level.set_block(pos, new_state, UPDATE_ALL);
            level.level_event(None, SOUND_DRIP_WATER_INTO_CAULDRON, pos, 0);
        }
    }
}
