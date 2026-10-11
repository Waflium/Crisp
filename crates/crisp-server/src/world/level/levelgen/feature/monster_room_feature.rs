use crisp_core::random::RandomSource;
use crisp_core::{BlockPos, Direction};
use crisp_data::{BlockState, block_tags, blocks};

use crate::server::level::world_gen_region::WorldGenRegion;
use crate::world::level::block::entity::BlockEntityKind;
use crate::world::level::levelgen::structure::structure_piece::reorient;

const MOBS: [&str; 4] = ["minecraft:skeleton", "minecraft:zombie", "minecraft:zombie", "minecraft:spider"];
const SIMPLE_DUNGEON: &str = "minecraft:chests/simple_dungeon";

fn safe_set_block(level: &mut WorldGenRegion, pos: BlockPos, state: BlockState) {
    if !level.get_block_state(pos).is(&block_tags::FEATURES_CANNOT_REPLACE) {
        level.set_block(pos, state, 2);
    }
}

pub fn place<R: RandomSource>(level: &mut WorldGenRegion, random: &mut R, origin: BlockPos) -> bool {
    let air = blocks::CAVE_AIR.default_state();
    let xr = random.next_int_bounded(2) + 2;
    let min_x = -xr - 1;
    let max_x = xr + 1;
    let zr = random.next_int_bounded(2) + 2;
    let min_z = -zr - 1;
    let max_z = zr + 1;
    let mut hole_count = 0;
    for dx in min_x..=max_x {
        for dy in -1..=4 {
            for dz in min_z..=max_z {
                let hole_pos = origin.offset(dx, dy, dz);
                let solid = level.get_block_state(hole_pos).is_solid();
                if dy == -1 && !solid {
                    return false;
                }
                if dy == 4 && !solid {
                    return false;
                }
                if (dx == min_x || dx == max_x || dz == min_z || dz == max_z)
                    && dy == 0
                    && level.get_block_state(hole_pos).is_air()
                    && level.get_block_state(hole_pos.above()).is_air()
                {
                    hole_count += 1;
                }
            }
        }
    }
    if !(1..=5).contains(&hole_count) {
        return false;
    }
    for dx in min_x..=max_x {
        let mut dy = 3;
        while dy >= -1 {
            for dz in min_z..=max_z {
                let wall_block = origin.offset(dx, dy, dz);
                let wall_state = level.get_block_state(wall_block);
                if dx == min_x || dy == -1 || dz == min_z || dx == max_x || dy == 4 || dz == max_z {
                    if wall_block.y >= level.get_min_y() && !level.get_block_state(wall_block.below()).is_solid() {
                        level.set_block(wall_block, air, 2);
                    } else if wall_state.is_solid() && !wall_state.is_block(blocks::CHEST) {
                        if dy == -1 && random.next_int_bounded(4) != 0 {
                            safe_set_block(level, wall_block, blocks::MOSSY_COBBLESTONE.default_state());
                        } else {
                            safe_set_block(level, wall_block, blocks::COBBLESTONE.default_state());
                        }
                    }
                } else if !wall_state.is_block(blocks::CHEST) && !wall_state.is_block(blocks::SPAWNER) {
                    safe_set_block(level, wall_block, air);
                }
            }
            dy -= 1;
        }
    }
    for _ in 0..2 {
        for _ in 0..3 {
            let xc = origin.x + random.next_int_bounded(xr * 2 + 1) - xr;
            let yc = origin.y;
            let zc = origin.z + random.next_int_bounded(zr * 2 + 1) - zr;
            let chest_pos = BlockPos::new(xc, yc, zc);
            if level.get_block_state(chest_pos).is_air() {
                let wall_count = Direction::PLANE_HORIZONTAL.into_iter().filter(|&direction| level.get_block_state(chest_pos.relative(direction)).is_solid()).count();
                if wall_count == 1 {
                    let chest = reorient(level, chest_pos, blocks::CHEST.default_state());
                    safe_set_block(level, chest_pos, chest);
                    set_block_entity_loot_table(level, random, chest_pos, SIMPLE_DUNGEON);
                    break;
                }
            }
        }
    }
    safe_set_block(level, origin, blocks::SPAWNER.default_state());
    match level.get_block_entity(origin).map(|entity| &mut entity.kind) {
        Some(BlockEntityKind::Spawner(spawner)) => {
            let entity_type = MOBS[random.next_int_bounded(MOBS.len() as i32) as usize];
            spawner.spawner.set_entity_id(entity_type, random);
        }
        _ => error!("Failed to fetch mob spawner entity at ({}, {}, {})", origin.x, origin.y, origin.z),
    }
    true
}

fn set_block_entity_loot_table(level: &mut WorldGenRegion, random: &mut impl RandomSource, pos: BlockPos, loot_table: &str) {
    if let Some(entity) = level.get_block_entity(pos)
        && entity.is_randomizable()
        && let Some(base) = entity.container_base_mut()
    {
        base.loot_table = Some(loot_table.to_owned());
        base.loot_table_seed = random.next_long();
    }
}
