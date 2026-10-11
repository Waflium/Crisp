use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, Direction};
use crisp_data::blocks;

use crate::server::level::world_gen_region::WorldGenRegion;

pub fn place<R: RandomSource>(level: &mut WorldGenRegion, random: &mut R, origin: BlockPos) -> bool {
    if origin.y > level.get_sea_level() - 1 {
        return false;
    }
    if !level.get_block_state(origin).is_block(blocks::WATER) && !level.get_block_state(origin.below()).is_block(blocks::WATER) {
        return false;
    }
    let found_packed_ice = Direction::VALUES.into_iter().any(|direction| direction != Direction::Down && level.get_block_state(origin.relative(direction)).is_block(blocks::PACKED_ICE));
    if !found_packed_ice {
        return false;
    }
    level.set_block(origin, blocks::BLUE_ICE.default_state(), 2);
    for _ in 0..200 {
        let y_off = random.next_int_bounded(5) - random.next_int_bounded(6);
        let mut xz_diff = 3;
        if y_off < 2 {
            xz_diff += y_off / 2;
        }
        if xz_diff >= 1 {
            let dx = random.next_int_bounded(xz_diff) - random.next_int_bounded(xz_diff);
            let dz = random.next_int_bounded(xz_diff) - random.next_int_bounded(xz_diff);
            let place_pos = origin.offset(dx, y_off, dz);
            let place_state = level.get_block_state(place_pos);
            if place_state.is_air() || place_state.is_block(blocks::WATER) || place_state.is_block(blocks::PACKED_ICE) || place_state.is_block(blocks::ICE) {
                for direction in Direction::VALUES {
                    if level.get_block_state(place_pos.relative(direction)).is_block(blocks::BLUE_ICE) {
                        level.set_block(place_pos, blocks::BLUE_ICE.default_state(), 2);
                        break;
                    }
                }
            }
        }
    }
    true
}
