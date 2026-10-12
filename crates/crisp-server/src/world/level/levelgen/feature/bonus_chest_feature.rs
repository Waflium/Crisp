use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, Direction};
use crisp_data::blocks;

use super::monster_room_feature::set_block_entity_loot_table;
use crate::world::level::block;
use crate::world::level::chunk::HeightmapType;
use crate::world::level::world_gen_level::WorldGenLevel;

const SPAWN_BONUS_CHEST: &str = "minecraft:chests/spawn_bonus_chest";

pub fn place<R: RandomSource>(level: &mut impl WorldGenLevel, random: &mut R, origin: BlockPos) -> bool {
    let chunk_pos = origin.chunk();
    let mut x_poses: Vec<i32> = (chunk_pos.min_block_x()..=chunk_pos.min_block_x() + 15).collect();
    crate::util::shuffle(&mut x_poses, random);
    let mut z_poses: Vec<i32> = (chunk_pos.min_block_z()..=chunk_pos.min_block_z() + 15).collect();
    crate::util::shuffle(&mut z_poses, random);
    for &x in &x_poses {
        for &z in &z_poses {
            let chest_pos = BlockPos::new(x, level.get_height(HeightmapType::MotionBlockingNoLeaves, x, z), z);
            let state = level.get_block_state(chest_pos);
            if state.is_air() || state.collision_shape().is_empty() {
                level.set_block(chest_pos, blocks::CHEST.default_state(), 2);
                set_block_entity_loot_table(level, random, chest_pos, SPAWN_BONUS_CHEST);
                let torch = blocks::TORCH.default_state();
                for direction in Direction::PLANE_HORIZONTAL {
                    let torch_pos = chest_pos.relative(direction);
                    if block::of(torch).can_survive(torch, level, torch_pos) {
                        level.set_block(torch_pos, torch, 2);
                    }
                }
                return true;
            }
        }
    }
    false
}
