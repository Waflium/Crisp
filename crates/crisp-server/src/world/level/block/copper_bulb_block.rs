use crisp_core::{BlockPos, Direction};
use crisp_data::properties::{LIT, POWERED};
use crisp_data::{Block, BlockState, registry, registry_id};

use super::BlockBehaviour;
use super::weathering_copper::change_over_time;
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::level::BlockGetter;
use crate::world::level::update_flags::UPDATE_ALL;

pub struct CopperBulbBlock;

impl CopperBulbBlock {
    pub fn check_and_flip(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        let signal = level.has_neighbor_signal(pos);
        if signal != state.get(POWERED) {
            let mut new_state = state;
            if !state.get(POWERED) {
                new_state = new_state.cycle(LIT);
                let sound = if new_state.get(LIT) { "minecraft:block.copper_bulb.turn_on" } else { "minecraft:block.copper_bulb.turn_off" };
                level.play_sound(None, pos, registry_id(registry::SOUND_EVENT, sound), SoundSource::Blocks, 1.0, 1.0);
            }
            level.set_block(pos, new_state.set(POWERED, signal), UPDATE_ALL);
        }
    }
}

impl BlockBehaviour for CopperBulbBlock {
    fn on_place(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, old_state: BlockState, _moved_by_piston: bool) {
        if old_state.block() != state.block() {
            self.check_and_flip(state, level, pos);
        }
    }

    fn neighbor_changed(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, _block: Block, _moved_by_piston: bool) {
        self.check_and_flip(state, level, pos);
    }

    fn get_analog_output_signal(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos, _direction: Direction) -> i32 {
        if level.block_state(pos).get(LIT) { 15 } else { 0 }
    }
}

pub struct WeatheringCopperBulbBlock;

impl BlockBehaviour for WeatheringCopperBulbBlock {
    fn on_place(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, old_state: BlockState, moved_by_piston: bool) {
        CopperBulbBlock.on_place(state, level, pos, old_state, moved_by_piston);
    }

    fn neighbor_changed(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, block: Block, moved_by_piston: bool) {
        CopperBulbBlock.neighbor_changed(state, level, pos, block, moved_by_piston);
    }

    fn get_analog_output_signal(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, direction: Direction) -> i32 {
        CopperBulbBlock.get_analog_output_signal(state, level, pos, direction)
    }

    fn random_tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        change_over_time(state, level, pos);
    }
}
