use crisp_core::{BlockPos, Direction};
use crisp_data::properties::POWER;
use crisp_data::{Block, BlockState};

use super::redstone_wire_evaluator::{get_block_signal, get_incoming_wire_signal};
use crate::server::level::ServerLevel;
use crate::world::level::BlockGetter;
use crate::world::level::update_flags::UPDATE_CLIENTS;

pub struct DefaultRedstoneWireEvaluator {
    pub wire_block: Block,
}

impl DefaultRedstoneWireEvaluator {
    pub fn update_power_strength(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState) {
        let target_strength = self.calculate_target_strength(level, pos);
        if state.get(POWER) != target_strength {
            if level.block_state(pos) == state {
                level.set_block(pos, state.set(POWER, target_strength), UPDATE_CLIENTS);
            }
            let mut to_update = [pos; 7];
            for (i, direction) in Direction::VALUES.into_iter().enumerate() {
                to_update[i + 1] = pos.relative(direction);
            }
            to_update.sort_by_key(|p| hash_set_bucket(*p));
            for block_pos in to_update {
                level.update_neighbors_at(block_pos, self.wire_block);
            }
        }
    }

    fn calculate_target_strength(&self, level: &mut ServerLevel, pos: BlockPos) -> i32 {
        let block_signal = get_block_signal(level, pos);
        if block_signal == 15 { block_signal } else { block_signal.max(get_incoming_wire_signal(self.wire_block, level, pos)) }
    }
}

fn hash_set_bucket(pos: BlockPos) -> i32 {
    let h = pos.java_hash();
    (h ^ ((h as u32) >> 16) as i32) & 15
}

#[cfg(test)]
mod tests {
    use crisp_core::{BlockPos, Direction};

    #[test]
    fn neighbours_follow_java_hash_set_order() {
        let order = |x, y, z| {
            let pos = BlockPos::new(x, y, z);
            let mut to_update = [pos; 7];
            for (i, direction) in Direction::VALUES.into_iter().enumerate() {
                to_update[i + 1] = pos.relative(direction);
            }
            to_update.sort_by_key(|p| super::hash_set_bucket(*p));
            to_update.map(|p| (p.x, p.y, p.z))
        };
        assert_eq!(order(0, -60, 0), [(0, -61, 0), (0, -60, 1), (1, -60, 0), (0, -60, 0), (0, -59, 0), (0, -60, -1), (-1, -60, 0)]);
        assert_eq!(order(5, 64, -3), [(5, 64, -2), (5, 63, -3), (6, 64, -3), (5, 64, -3), (5, 65, -3), (5, 64, -4), (4, 64, -3)]);
        assert_eq!(
            order(-1000, 12, 77),
            [(-1000, 12, 77), (-1000, 13, 77), (-1000, 12, 76), (-1001, 12, 77), (-1000, 11, 77), (-1000, 12, 78), (-999, 12, 77)]
        );
    }
}
