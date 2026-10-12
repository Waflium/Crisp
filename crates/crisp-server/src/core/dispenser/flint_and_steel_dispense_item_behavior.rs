use crisp_data::properties::LIT;

use super::block_source::BlockSource;
use crate::server::level::ServerLevel;
use crate::world::item::ItemStack;
use crate::world::level::BlockGetter;
use crate::world::level::block::{base_fire_block, campfire_block, candle_block, candle_cake_block, tnt_block};
use crate::world::level::update_flags::UPDATE_ALL;

pub fn execute(level: &mut ServerLevel, source: &BlockSource, mut dispensed: ItemStack, success: &mut bool) -> ItemStack {
    *success = true;
    let facing = source.facing();
    let target_pos = source.pos.relative(facing);
    let target = level.block_state(target_pos);
    if base_fire_block::can_be_placed_at(level, target_pos, facing) {
        let fire = base_fire_block::get_state(level, target_pos);
        level.set_block(target_pos, fire, UPDATE_ALL);
    } else if campfire_block::can_light(target) || candle_block::can_light(target) || candle_cake_block::can_light(target) {
        level.set_block(target_pos, target.set(LIT, true), UPDATE_ALL);
    } else if target.block().is_instance_of("TntBlock") {
        if tnt_block::prime(level, target_pos, None, Some(&dispensed)) {
            level.remove_block(target_pos, false);
        } else {
            *success = false;
        }
    }
    if *success {
        dispensed.hurt_and_break(1, level);
    }
    dispensed
}
